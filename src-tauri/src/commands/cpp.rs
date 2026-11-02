//! IPC commands of `ide_host::ops::cpp` (C/C++ source ↔ header pairing).

use crate::state::AppState;
use ide_core::{IdeResult, IpcPath};
use std::path::PathBuf;
use tauri::{State, Window};

/// Headers related to a C/C++ source (or sources related to a header), best first.
#[tauri::command]
#[specta::specta]
pub async fn cpp_related_files(window: Window, state: State<'_, AppState>, path: PathBuf) -> IdeResult<Vec<IpcPath>> {
    ide_host::ops::cpp::related_files(&state.host, window.label(), path)
}
