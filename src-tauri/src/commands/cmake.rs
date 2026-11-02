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

/// Configure and build presets of the project's `CMakePresets.json`.
#[tauri::command]
#[specta::specta]
pub async fn cmake_presets(window: Window, state: State<'_, AppState>) -> IdeResult<Vec<ide_cmake::CmakePreset>> {
    ide_host::ops::cmake::presets(&state.host, window.label()).await
}
