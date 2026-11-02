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

/// The base-class virtual members the C/C++ class at `pos` can override, for
/// the chooser: name, signature, and where the base class is declared.
#[tauri::command]
#[specta::specta]
pub async fn cpp_members(
    window: Window,
    state: State<'_, AppState>,
    doc: DocId,
    pos: Pos,
) -> IdeResult<Vec<ide_host::ops::cpp::GeneratableMember>> {
    ide_host::ops::cpp::members_to_generate(&state.host, window.label(), doc, pos).await
}

/// Inserts a stub for every base-class virtual member the C/C++ class at `pos`
/// does not override yet, and returns the file and line of the first stub.
#[tauri::command]
#[specta::specta]
pub async fn cpp_generate_members(
    window: Window,
    state: State<'_, AppState>,
    doc: DocId,
    pos: Pos,
) -> IdeResult<ide_host::ops::cpp::GeneratedDefinition> {
    ide_host::ops::cpp::generate_members(&state.host, window.label(), doc, pos).await
}
