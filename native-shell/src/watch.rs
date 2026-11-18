//! Filesystem watching: changes on disk rebuild the project tree and refresh
//! git status, so the editor and backend stay in step with external tools.

use std::path::Path;
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, new_debouncer};
use nsh_core::prelude::*;

use crate::model::State;

pub fn start(state: State) {
    let root = state.root.get();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut debouncer = match new_debouncer(Duration::from_millis(400), None, move |result: DebounceEventResult| {
        let Ok(events) = result else {
            return;
        };
        let relevant = events.iter().flat_map(|event| event.paths.iter()).any(|path| !ignored(path));
        if relevant {
            let _ = tx.send(());
        }
    }) {
        Ok(debouncer) => debouncer,
        Err(_) => return,
    };
    if debouncer.watch(&root, RecursiveMode::Recursive).is_err() {
        return;
    }
    // The debouncer runs for the lifetime of the app; leaking it keeps its
    // thread and inotify watch alive after this function returns.
    std::mem::forget(debouncer);
    spawn(async move {
        while rx.recv().await.is_some() {
            state.reload_tree();
            state.refresh_vcs();
        }
    });
}

/// Generated output does not change the project tree.
fn ignored(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some("build")
                | Some("cmake-build-debug")
                | Some("cmake-build-release")
                | Some("target")
                | Some(".git")
                | Some("node_modules")
                | Some(".nsh")
                | Some(".cache")
        )
    })
}
