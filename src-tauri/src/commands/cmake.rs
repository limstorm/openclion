//! IPC commands of `ide_host::ops::cmake` (the shared implementation).

use crate::state::AppState;
use ide_cmake::CmakeModel;
use ide_core::IdeResult;
use tauri::{State, Window};

/// Build directories, codemodel targets and presets of the window's CMake project.
#[tauri::command]
#[specta::specta]
pub async fn cmake_model(window: Window, state: State<'_, AppState>) -> IdeResult<CmakeModel> {
    ide_host::ops::cmake::model(&state.host, window.label()).await
}
