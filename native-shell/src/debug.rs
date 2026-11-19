//! lldb-dap through `ide-dap`: launch the CMake executable, stop at entry, and
//! show stack frames and locals in the Debug tool window.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

use ide_core::{DebugSessionId, EventSink, IdeEvent, SharedSink, event::DebugEvent};
use ide_dap::{DebugManager, LaunchConfig, SessionOwner, StepKind};
use ide_language::LanguageRegistry;
use nsh_base::editor::{EditorApi, EditorDecoration, EditorGutterMark, EditorPosition, EditorRange, EditorSelection};
use nsh_core::prelude::*;

use crate::model::State;

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
        for (path, lines) in breakpoints {
            let list: Vec<ide_dap::Breakpoint> = lines
                .into_iter()
                .map(|line| ide_dap::Breakpoint {
                    line,
                    condition: None,
                    hit_condition: None,
                    log_message: None,
                    enabled: true,
                })
                .collect();
            let _ = manager.set_breakpoints(path.clone(), list).await;
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
                });
            }
        }
        state.debug_vars.set(vars);
    });
}

/// Toggles a breakpoint on `line` (0-based) of the active editor.
pub fn toggle_breakpoint(state: State, line: u32) {
    let path = state.active_path.get();
    if path.as_os_str().is_empty() {
        return;
    }
    let mut lines = state.breakpoints.with(|map| map.get(&path).cloned().unwrap_or_default());
    if let Some(index) = lines.iter().position(|value| *value == line) {
        lines.remove(index);
    } else {
        lines.push(line);
    }
    lines.sort_unstable();
    lines.dedup();
    state.breakpoints.with(|map| {
        if lines.is_empty() {
            map.remove(&path);
        } else {
            map.insert(path.clone(), lines.clone());
        }
    });
    state.breakpoints_rev.update(|rev| *rev += 1);
    if let Some(bridge) = state.debug_bridge.get() {
        let list: Vec<ide_dap::Breakpoint> = lines
            .iter()
            .map(|line| ide_dap::Breakpoint {
                line: *line,
                condition: None,
                hit_condition: None,
                log_message: None,
                enabled: true,
            })
            .collect();
        spawn(async move {
            let _ = bridge.manager.set_breakpoints(path, list).await;
        });
    }
}

/// Breakpoints and the debugger's current line for the active editor.
pub fn decorations(state: State) -> Vec<EditorDecoration> {
    state.breakpoints_rev.get();
    let path = state.active_path.get();
    let mut out = Vec::new();
    if let Some(lines) = state.breakpoints.with(|map| map.get(&path).cloned()) {
        for line in lines {
            let mut decoration = EditorDecoration::default();
            decoration.range = EditorRange::new(EditorPosition::new(line, 0), EditorPosition::new(line, 0));
            let mut gutter = EditorGutterMark::default();
            gutter.class = Some("bg-[#e5534b]".into());
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
