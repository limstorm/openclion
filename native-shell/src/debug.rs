//! lldb-dap through `ide-dap`: launch the CMake executable, stop at entry, and
//! show stack frames and locals in the Debug tool window.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

use ide_core::{DebugSessionId, EventSink, IdeEvent, SharedSink, event::DebugEvent};
use ide_dap::{DebugManager, LaunchConfig, SessionOwner, StepKind};
use ide_language::LanguageRegistry;
use nsh_base::editor::{EditorApi, EditorDecoration, EditorGutterMark, EditorPosition, EditorRange, EditorSelection};
use nsh_core::prelude::*;

use crate::model::{BreakpointSpec, State};

#[derive(Clone, PartialEq)]
pub struct DebugFrame {
    pub id: i64,
    pub name: String,
    pub path: Option<PathBuf>,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, PartialEq)]
pub struct DebugVar {
    pub scope: String,
    pub name: String,
    pub value: String,
    pub type_name: Option<String>,
    pub reference: i64,
    pub depth: u32,
    pub expanded: bool,
}

#[derive(Clone, PartialEq)]
pub struct WatchValue {
    pub expression: String,
    pub value: String,
}

#[derive(Clone)]
pub struct DebugBridge {
    manager: Arc<DebugManager>,
    sink: SharedSink,
}

struct DebugSink {
    tx: tokio::sync::mpsc::UnboundedSender<IdeEvent>,
}

impl EventSink for DebugSink {
    fn emit(&self, event: IdeEvent) {
        let _ = self.tx.send(event);
    }
}

/// Creates the manager and drains its events on the engine thread.
pub fn init(state: State) {
    if std::env::var_os("OPENCLION_NO_DEBUG").is_some() {
        return;
    }
    let Ok(languages) = LanguageRegistry::load(None) else {
        return;
    };
    let manager = DebugManager::new(Arc::new(languages));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let sink: SharedSink = Arc::new(DebugSink { tx });
    state.debug_bridge.replace(Some(DebugBridge { manager, sink }));
    spawn(async move {
        while let Some(event) = rx.recv().await {
            handle(state, event);
        }
    });
}

fn handle(state: State, event: IdeEvent) {
    let IdeEvent::Debug { session, event } = event else {
        return;
    };
    match event {
        DebugEvent::Stopped { thread_id, reason, .. } => {
            state.debug_active.set(true);
            state.debug_stopped.set(true);
            state.debug_thread.set(thread_id);
            state.debug_status.set(format!("Paused ({reason})"));
            refresh_stack(state, session, thread_id);
        }
        DebugEvent::Continued { .. } => {
            state.debug_stopped.set(false);
            state.debug_line.set(None);
            state.debug_status.set("Running".to_owned());
        }
        DebugEvent::Output { output, .. } => {
            crate::terminal::write_output(state, &output);
        }
        DebugEvent::Started { name, .. } => {
            state.debug_status.set(format!("Starting {name}"));
        }
        DebugEvent::Initialized => {
            state.debug_status.set("Configuring…".to_owned());
        }
        DebugEvent::Terminated | DebugEvent::Exited { .. } => {
            state.debug_active.set(false);
            state.debug_stopped.set(false);
            state.debug_session.set(None);
            state.debug_frames.set(Vec::new());
            state.debug_vars.set(Vec::new());
            state.debug_watch_values.set(Vec::new());
            state.debug_line.set(None);
            state.debug_status.set("Session ended".to_owned());
        }
        DebugEvent::BreakpointsChanged { .. } | DebugEvent::Thread { .. } => {}
    }
}

/// Starts the first executable of the CMake model under lldb-dap.
pub fn start(state: State) {
    if state.debug_active.get() {
        state.status.set("Debug session already running".to_owned());
        return;
    }
    let Some(bridge) = state.debug_bridge.get() else {
        state.status.set("Debug adapter unavailable".to_owned());
        return;
    };
    let Some(model) = state.cmake.get() else {
        state.status.set("No CMake model to debug".to_owned());
        return;
    };
    let Some((build, target)) = model.executables().next() else {
        state.status.set("No executable target to debug".to_owned());
        return;
    };
    let root = state.root.get();
    let program = target.artifact.clone().map(PathBuf::from).filter(|path| path.is_file()).or_else(|| {
        let candidate = root.join(&build.dir).join(&target.name);
        candidate.is_file().then_some(candidate)
    });
    let Some(program) = program else {
        state.status.set(format!("Build the project first: no artifact for {}", target.name));
        return;
    };
    let name = target.name.clone();
    let config = LaunchConfig {
        name: format!("{name} (Debug)"),
        adapter: "lldb-dap".to_owned(),
        request: "launch".to_owned(),
        program: Some(program.to_string_lossy().to_string()),
        args: Vec::new(),
        cwd: Some(root.into()),
        env: BTreeMap::new(),
        extra_json: Some(r#"{"stopOnEntry":false}"#.to_owned()),
    };
    state.debug_status.set("Starting lldb-dap…".to_owned());
    state.show_bottom.set(true);
    state.bottom_tool.set(Some(crate::model::BottomTool::Debug));
    crate::terminal::ensure_started(state);
    let manager = bridge.manager;
    let breakpoints = state.breakpoints.with(|map| map.clone());
    spawn(async move {
        for (path, specs) in breakpoints {
            let list = dap_breakpoints(&specs);
            let _ = manager.set_breakpoints(path, list).await;
        }
        match manager.start(config, SessionOwner::new("openclion-native", bridge.sink)).await {
            Ok(session) => {
                state.debug_session.set(Some(session));
                state.debug_active.set(true);
                state.debug_status.set("Debugger attached".to_owned());
            }
            Err(error) => {
                state.debug_status.set(format!("Debug failed: {error}"));
                state.status.set(format!("Debug failed: {error}"));
            }
        }
    });
}

fn refresh_stack(state: State, session: DebugSessionId, thread: i64) {
    let Some(bridge) = state.debug_bridge.get() else {
        return;
    };
    let manager = bridge.manager;
    spawn(async move {
        let frames = manager.stack_trace(session, thread).await.unwrap_or_default();
        let first = frames.first().map(|frame| (frame.id, frame.path.clone(), frame.line, frame.column));
        let mapped: Vec<DebugFrame> = frames
            .iter()
            .map(|frame| DebugFrame {
                id: frame.id,
                name: frame.name.clone(),
                path: frame.path.clone().map(|path| path.into_path_buf()),
                line: frame.line,
                column: frame.column,
            })
            .collect();
        state.debug_frames.set(mapped);
        state.debug_selected.set(0);
        let Some((frame_id, path, line, column)) = first else {
            return;
        };
        if let Some(path) = path {
            let path = path.into_path_buf();
            state.debug_line.set(Some((path.clone(), line)));
            state.open_path(path);
            set_timeout(Duration::from_millis(120), move || {
                if let Some(api) = state.editor.api::<EditorApi>() {
                    api.set_selections(vec![EditorSelection::caret(EditorPosition::new(line, column))]);
                    api.scroll_to_line(line);
                }
            });
        }
        let scopes = manager.scopes(session, frame_id).await.unwrap_or_default();
        let mut vars = Vec::new();
        for scope in scopes.iter().take(2) {
            let values = manager.variables(session, scope.variables_reference).await.unwrap_or_default();
            for value in values {
                vars.push(DebugVar {
                    scope: scope.name.clone(),
                    name: value.name,
                    value: value.value,
                    type_name: value.type_name,
                    reference: value.variables_reference,
                    depth: 0,
                    expanded: false,
                });
            }
        }
        state.debug_vars.set(vars);
    });
}

/// Re-reads the variables of frame `index` in the Debug window.
pub fn select_frame(state: State, index: usize) {
    let frames = state.debug_frames.get();
    let Some(frame) = frames.get(index) else {
        return;
    };
    let Some(session) = state.debug_session.get() else {
        return;
    };
    let frame_id = frame.id;
    state.debug_selected.set(index);
    let Some(bridge) = state.debug_bridge.get() else {
        return;
    };
    spawn(async move {
        let scopes = bridge.manager.scopes(session, frame_id).await.unwrap_or_default();
        let mut vars = Vec::new();
        for scope in scopes.iter().take(2) {
            let values = bridge.manager.variables(session, scope.variables_reference).await.unwrap_or_default();
            for value in values {
                vars.push(DebugVar {
                    scope: scope.name.clone(),
                    name: value.name,
                    value: value.value,
                    type_name: value.type_name,
                    reference: value.variables_reference,
                    depth: 0,
                    expanded: false,
                });
            }
        }
        state.debug_vars.set(vars);
        refresh_watch(state);
    });
}

/// Adds a watch expression and evaluates it at the selected frame.
pub fn add_watch(state: State, expression: String) {
    let expression = expression.trim().to_owned();
    if expression.is_empty() {
        return;
    }
    state.debug_watch.update(|watch| {
        if !watch.contains(&expression) {
            watch.push(expression);
        }
    });
    state.debug_watch_input.set(String::new());
    refresh_watch(state);
}

pub fn remove_watch(state: State, index: usize) {
    state.debug_watch.update(|watch| {
        if index < watch.len() {
            watch.remove(index);
        }
    });
    refresh_watch(state);
}

/// Re-evaluates every watch expression at the selected frame.
pub fn refresh_watch(state: State) {
    let (Some(session), Some(bridge)) = (state.debug_session.get(), state.debug_bridge.get()) else {
        return;
    };
    let Some(frame) = state.debug_frames.get().get(state.debug_selected.get()).cloned() else {
        return;
    };
    let manager = bridge.manager;
    spawn(async move {
        let mut values = Vec::new();
        for expression in state.debug_watch.get() {
            let value = manager
                .evaluate(session, Some(frame.id), expression.clone())
                .await
                .map(|variable| variable.value)
                .unwrap_or_else(|error| format!("<{error}>"));
            values.push(WatchValue { expression, value });
        }
        state.debug_watch_values.set(values);
    });
}

/// Toggles a breakpoint on `line` (0-based) of the active editor.
pub fn toggle_breakpoint(state: State, line: u32) {
    let path = state.active_path.get();
    if path.as_os_str().is_empty() {
        return;
    }
    let mut specs = state.breakpoints.with(|map| map.get(&path).cloned().unwrap_or_default());
    if let Some(index) = specs.iter().position(|spec| spec.line == line) {
        specs.remove(index);
    } else {
        specs.push(BreakpointSpec::at(line));
    }
    specs.sort_by_key(|spec| spec.line);
    specs.dedup_by_key(|spec| spec.line);
    store_breakpoints(state, &path, specs);
}

/// Sets or clears the condition of the breakpoint at `line`.
/// Opens the condition dialog for the breakpoint on the caret's line.
pub fn edit_condition_at_caret(state: State) {
    let path = state.active_path.get();
    if path.as_os_str().is_empty() {
        return;
    }
    let line = state.cursor.get().0.saturating_sub(1);
    let exists =
        state.breakpoints.with(|map| map.get(&path).is_some_and(|specs| specs.iter().any(|spec| spec.line == line)));
    if exists {
        state.open_breakpoint_condition(path, line);
    } else {
        state.status.set("No breakpoint on this line".to_owned());
    }
}

pub fn set_breakpoint_condition(state: State, path: std::path::PathBuf, line: u32, condition: String) {
    let mut specs = state.breakpoints.with(|map| map.get(&path).cloned().unwrap_or_default());
    let condition = (!condition.trim().is_empty()).then(|| condition.trim().to_owned());
    if let Some(spec) = specs.iter_mut().find(|spec| spec.line == line) {
        spec.condition = condition;
    } else {
        let mut spec = BreakpointSpec::at(line);
        spec.condition = condition;
        specs.push(spec);
        specs.sort_by_key(|spec| spec.line);
    }
    store_breakpoints(state, &path, specs);
}

fn store_breakpoints(state: State, path: &std::path::Path, specs: Vec<BreakpointSpec>) {
    state.breakpoints.with(|map| {
        if specs.is_empty() {
            map.remove(path);
        } else {
            map.insert(path.to_path_buf(), specs.clone());
        }
    });
    state.breakpoints_rev.update(|rev| *rev += 1);
    if let Some(bridge) = state.debug_bridge.get() {
        let manager = bridge.manager;
        let list = dap_breakpoints(&specs);
        let path = path.to_path_buf();
        background(async move {
            let _ = manager.set_breakpoints(path, list).await;
        });
    }
}

fn dap_breakpoints(specs: &[BreakpointSpec]) -> Vec<ide_dap::Breakpoint> {
    specs
        .iter()
        .map(|spec| ide_dap::Breakpoint {
            line: spec.line,
            condition: spec.condition.clone(),
            hit_condition: None,
            log_message: spec.log_message.clone(),
            enabled: spec.enabled,
        })
        .collect()
}

/// Breakpoints and the debugger's current line for the active editor.
pub fn decorations(state: State) -> Vec<EditorDecoration> {
    state.breakpoints_rev.get();
    let path = state.active_path.get();
    let mut out = Vec::new();
    if let Some(specs) = state.breakpoints.with(|map| map.get(&path).cloned()) {
        for spec in specs {
            let line = spec.line;
            let mut decoration = EditorDecoration::default();
            decoration.range = EditorRange::new(EditorPosition::new(line, 0), EditorPosition::new(line, 0));
            let mut gutter = EditorGutterMark::default();
            gutter.class = Some(
                if spec.condition.is_some() || spec.log_message.is_some() { "bg-[#e5b34b]" } else { "bg-[#e5534b]" }
                    .into(),
            );
            decoration.gutter = Some(gutter);
            out.push(decoration);
        }
    }
    if let Some((line_path, line)) = state.debug_line.get()
        && line_path == path
    {
        let mut decoration = EditorDecoration::default();
        decoration.range = EditorRange::new(EditorPosition::new(line, 0), EditorPosition::new(line, 0));
        decoration.line_class = Some("bg-[#d4e2ff]".into());
        out.push(decoration);
    }
    out
}

/// Expands or collapses a variable in the Locals tree.
pub fn toggle_variable(state: State, index: usize) {
    let vars = state.debug_vars.get();
    let Some(variable) = vars.get(index) else {
        return;
    };
    if variable.expanded {
        let depth = variable.depth;
        let mut next = vars;
        next[index].expanded = false;
        let mut end = index + 1;
        while end < next.len() && next[end].depth > depth {
            end += 1;
        }
        next.drain(index + 1..end);
        state.debug_vars.set(next);
        return;
    }
    if variable.reference == 0 {
        return;
    }
    let reference = variable.reference;
    let depth = variable.depth;
    let scope = variable.scope.clone();
    let Some(session) = state.debug_session.get() else {
        return;
    };
    let Some(bridge) = state.debug_bridge.get() else {
        return;
    };
    spawn(async move {
        let Ok(values) = bridge.manager.variables(session, reference).await else {
            return;
        };
        let mut vars = state.debug_vars.get();
        let Some(parent) = vars.get_mut(index) else {
            return;
        };
        parent.expanded = true;
        let mut children: Vec<DebugVar> = values
            .into_iter()
            .map(|value| DebugVar {
                scope: scope.clone(),
                name: value.name,
                value: value.value,
                type_name: value.type_name,
                reference: value.variables_reference,
                depth: depth + 1,
                expanded: false,
            })
            .collect();
        if children.is_empty() {
            return;
        }
        let at = (index + 1).min(vars.len());
        vars.splice(at..at, children.drain(..));
        state.debug_vars.set(vars);
    });
}

pub fn resume(state: State) {
    let (Some(session), thread) = (state.debug_session.get(), state.debug_thread.get()) else {
        return;
    };
    let Some(bridge) = state.debug_bridge.get() else {
        return;
    };
    spawn(async move {
        let _ = bridge.manager.resume(session, thread).await;
    });
}

pub fn step(state: State, kind: StepKind) {
    let (Some(session), thread) = (state.debug_session.get(), state.debug_thread.get()) else {
        return;
    };
    let Some(bridge) = state.debug_bridge.get() else {
        return;
    };
    spawn(async move {
        let _ = bridge.manager.step(session, thread, kind).await;
    });
}

pub fn stop(state: State) {
    let Some(session) = state.debug_session.get() else {
        return;
    };
    let Some(bridge) = state.debug_bridge.get() else {
        return;
    };
    spawn(async move {
        let _ = bridge.manager.stop(session).await;
        state.debug_active.set(false);
        state.debug_status.set("Stopped".to_owned());
    });
}
