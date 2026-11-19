//! The integrated terminal: a real PTY from `ide-terminal`, drawn by nsh's
//! native Terminal component (VTE emulation, ANSI colours, input methods).

use std::sync::Arc;

use ide_core::{NullSink, SharedSink, TerminalId};
use ide_terminal::{TerminalManager, TerminalOptions};
use nsh_base::TerminalApi;
use nsh_core::prelude::*;

use crate::model::State;

#[derive(Clone)]
pub struct Session {
    manager: Arc<TerminalManager>,
    id: TerminalId,
}

/// Starts the login shell once, the first time the Terminal tool window opens.
pub fn ensure_started(state: State) {
    if state.terminal_started.get() || std::env::var_os("OPENCLION_NO_TERMINAL").is_some() {
        return;
    }
    state.terminal_started.set(true);
    let root = state.root.get();
    let manager = Arc::new(TerminalManager::new());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let options = TerminalOptions {
        cwd: root.clone().into(),
        shell: None,
        args: Vec::new(),
        env: Default::default(),
        cols: 120,
        rows: 30,
        shell_integration: false,
    };
    let sink: SharedSink = Arc::new(NullSink);
    match manager.spawn(
        options,
        sink,
        Box::new(move |data| {
            let _ = tx.send(data);
        }),
    ) {
        Ok(info) => {
            let id = info.id;
            let manager_for_prompt = manager.clone();
            state.terminal_session.replace(Some(Session { manager, id }));
            // The user's theme prints a right-hand Node/time prompt on every
            // line; it adds no information here and misplaces the cursor.
            spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(450)).await;
                let _ = manager_for_prompt
                    .write(id, " PS1='\\w $'; PROMPT='%~ $'; RPROMPT=''; precmd_functions=(); clear\n");
            });
            spawn(async move {
                while let Some(data) = rx.recv().await {
                    write_output(state, &data);
                }
            });
        }
        Err(error) => {
            state.terminal_buffer.with(|buffer| buffer.push_str(&format!("terminal: {error}\r\n")));
            state.terminal_rev.update(|rev| *rev += 1);
        }
    }
}

/// Writes `data` into the terminal's emulator (build output, run output, or
/// the shell's bytes) — never into the shell's input.
pub fn write_output(state: State, data: &str) {
    if let Some(api) = state.terminal_ref.api::<TerminalApi>() {
        api.write(data.to_owned());
        state.terminal_rev.update(|rev| *rev += 1);
    } else {
        state.terminal_buffer.with(|buffer| buffer.push_str(data));
        state.terminal_rev.update(|rev| *rev += 1);
    }
}

/// Flushes output that arrived before the Terminal component was mounted.
pub fn flush(state: State) {
    let api = state.terminal_ref.api::<TerminalApi>();
    let pending = state.terminal_buffer.with(|buffer| std::mem::take(buffer));
    if let Some(api) = api
        && !pending.is_empty()
    {
        api.write(pending);
    } else if !pending.is_empty() {
        state.terminal_buffer.with(|buffer| buffer.push_str(&pending));
    }
}

/// Gives the Terminal component keyboard focus.
pub fn focus(state: State) {
    if let Some(api) = state.terminal_ref.api::<TerminalApi>() {
        api.focus();
    }
}

pub fn send(state: State, data: &str) {
    if let Some(session) = state.terminal_session.get() {
        let _ = session.manager.write(session.id, data);
    }
}

pub fn resize(state: State, cols: u32, rows: u32) {
    if let Some(session) = state.terminal_session.get() {
        let _ = session.manager.resize(session.id, cols as u16, rows as u16);
    }
}
