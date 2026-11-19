//! The integrated terminal: real PTYs from `ide-terminal`, drawn by nsh's
//! native Terminal component. Each tab owns one shell and a raw output log;
//! switching tabs replays that log into a fresh emulator.

use std::sync::Arc;

use ide_core::{NullSink, SharedSink, TerminalId};
use ide_terminal::{TerminalManager, TerminalOptions};
use nsh_base::TerminalApi;
use nsh_core::prelude::*;

use crate::model::{State, TerminalTab};

#[derive(Clone)]
pub struct Session {
    manager: Arc<TerminalManager>,
    id: TerminalId,
    key: u64,
}

const MAX_TABS: usize = 8;
const LOG_LIMIT: usize = 2_000_000;

fn shell_options(root: std::path::PathBuf) -> TerminalOptions {
    TerminalOptions {
        cwd: root.into(),
        shell: None,
        args: Vec::new(),
        env: Default::default(),
        cols: 120,
        rows: 30,
        shell_integration: false,
    }
}

fn prompt(manager: &Arc<TerminalManager>, id: TerminalId) {
    let manager = manager.clone();
    spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(450)).await;
        let _ = manager.write(id, " PS1=' \\w$'; PROMPT=' %~$'; RPROMPT=''; precmd_functions=(); clear\n");
    });
}

pub fn ensure_started(state: State) {
    if std::env::var_os("OPENCLION_NO_TERMINAL").is_some() {
        return;
    }
    if state.terminal_sessions.with(|sessions| sessions.is_empty()) {
        new_terminal(state);
    }
}

/// Opens another local shell tab.
pub fn new_terminal(state: State) {
    if std::env::var_os("OPENCLION_NO_TERMINAL").is_some() {
        return;
    }
    if state.terminal_last_new.with(|last| last.is_some_and(|last| last.elapsed().as_millis() < 350)) {
        return;
    }
    state.terminal_last_new.with(|last| *last = Some(std::time::Instant::now()));
    if state.terminal_sessions.with(|sessions| sessions.len()) >= MAX_TABS {
        state.status.set(format!("At most {MAX_TABS} terminal tabs"));
        return;
    }
    let key = state.terminal_next.get() + 1;
    state.terminal_next.set(key);
    let count = state.terminal_tabs.get().len() + 1;
    let title = if count == 1 { "Local".to_owned() } else { format!("Local {count}") };
    let manager = Arc::new(TerminalManager::new());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let sink: SharedSink = Arc::new(NullSink);
    match manager.spawn(
        shell_options(state.root.get()),
        sink,
        Box::new(move |data| {
            let _ = tx.send(data);
        }),
    ) {
        Ok(info) => {
            let id = info.id;
            prompt(&manager, id);
            state.terminal_sessions.with(|sessions| sessions.push(Session { manager, id, key }));
            state.terminal_tabs.update(|tabs| tabs.push(TerminalTab { key, title: title.clone() }));
            state.terminal_active.set(key);
            state.terminal_rev.update(|rev| *rev += 1);
            spawn(async move {
                while let Some(data) = rx.recv().await {
                    let live =
                        state.terminal_sessions.with(|sessions| sessions.iter().any(|session| session.key == key));
                    if !live {
                        break;
                    }
                    state.terminal_logs.with(|logs| {
                        let log = logs.entry(key).or_default();
                        log.push_str(&data);
                        if log.len() > LOG_LIMIT {
                            let cut = log.len().saturating_sub(LOG_LIMIT / 2);
                            let cut = log.char_indices().nth(cut).map(|(index, _)| index).unwrap_or(0);
                            log.drain(..cut);
                        }
                    });
                    if state.terminal_active.get() == key {
                        if let Some(api) = state.terminal_ref.api::<TerminalApi>() {
                            api.write(data);
                        }
                    }
                    state.terminal_rev.update(|rev| *rev += 1);
                }
            });
        }
        Err(error) => state.status.set(format!("Terminal: {error}")),
    }
}

pub fn select(state: State, key: u64) {
    if state.terminal_active.get() == key {
        focus(state);
        return;
    }
    state.terminal_active.set(key);
    state.terminal_rev.update(|rev| *rev += 1);
    set_timeout(std::time::Duration::from_millis(90), move || {
        sync(state);
        focus(state);
    });
}

pub fn close(state: State, key: u64) {
    state.terminal_sessions.with(|sessions| {
        if let Some(index) = sessions.iter().position(|session| session.key == key) {
            let session = sessions.remove(index);
            let _ = session.manager.kill(session.id);
        }
    });
    state.terminal_logs.with(|logs| {
        logs.remove(&key);
    });
    if state.terminal_shown.get() == key {
        state.terminal_shown.replace(0);
    }
    state.terminal_tabs.update(|tabs| tabs.retain(|tab| tab.key != key));
    if state.terminal_active.get() == key {
        let next = state.terminal_tabs.get().first().map(|tab| tab.key).unwrap_or(0);
        state.terminal_active.set(next);
    }
    state.terminal_rev.update(|rev| *rev += 1);
    if state.terminal_tabs.get().is_empty() {
        new_terminal(state);
    }
}

/// Writes `data` into the active terminal (build/run output or shell bytes).
pub fn write_output(state: State, data: &str) {
    let key = state.terminal_active.get();
    if key == 0 {
        return;
    }
    state.terminal_logs.with(|logs| logs.entry(key).or_default().push_str(data));
    if let Some(api) = state.terminal_ref.api::<TerminalApi>() {
        api.write(data.to_owned());
    }
    state.terminal_rev.update(|rev| *rev += 1);
}

/// Replays the active tab's raw output into a freshly mounted Terminal.
pub fn sync(state: State) {
    let key = state.terminal_active.get();
    if key == 0 || state.terminal_shown.get() == key {
        return;
    }
    let Some(api) = state.terminal_ref.api::<TerminalApi>() else {
        return;
    };
    let log = state.terminal_logs.with(|logs| logs.get(&key).cloned()).unwrap_or_default();
    api.reset();
    if !log.is_empty() {
        api.write(log);
    }
    state.terminal_shown.replace(key);
    api.focus();
}

/// Called when the Terminal view mounts, so the log is replayed into it.
pub fn invalidate(state: State) {
    state.terminal_shown.replace(0);
}

pub fn focus(state: State) {
    if let Some(api) = state.terminal_ref.api::<TerminalApi>() {
        api.focus();
    }
}

pub fn send(state: State, data: &str) {
    let key = state.terminal_active.get();
    let session = state.terminal_sessions.with(|sessions| {
        sessions.iter().find(|session| session.key == key).map(|session| (session.manager.clone(), session.id))
    });
    if let Some((manager, id)) = session {
        let _ = manager.write(id, data);
    }
}

pub fn resize(state: State, cols: u32, rows: u32) {
    let key = state.terminal_active.get();
    let session = state.terminal_sessions.with(|sessions| {
        sessions.iter().find(|session| session.key == key).map(|session| (session.manager.clone(), session.id))
    });
    if let Some((manager, id)) = session {
        let _ = manager.resize(id, cols as u16, rows as u16);
    }
}
