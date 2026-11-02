//! IPC commands of `ide_host::ops::cpp` (C/C++ source ↔ header pairing).

use crate::state::AppState;
use ide_core::{DocId, IdeResult, IpcPath, Pos, RequestId};
use std::path::PathBuf;
use tauri::{State, Window};

/// Headers related to a C/C++ source (or sources related to a header), best first.
#[tauri::command]
#[specta::specta]
pub async fn cpp_related_files(window: Window, state: State<'_, AppState>, path: PathBuf) -> IdeResult<Vec<IpcPath>> {
    ide_host::ops::cpp::related_files(&state.host, window.label(), path)
}

/// Appends the definition of the declaration at `pos` in the header `doc` to the
/// matching source file, and returns where it was written.
#[tauri::command]
#[specta::specta]
pub async fn cpp_generate_definition(
    window: Window,
    state: State<'_, AppState>,
    doc: DocId,
    pos: Pos,
    request: Option<RequestId>,
) -> IdeResult<ide_host::ops::cpp::GeneratedDefinition> {
    ide_host::ops::cpp::generate_definition(&state.host, window.label(), doc, pos, request).await
}
