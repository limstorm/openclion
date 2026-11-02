//! IPC commands of `ide_host::ops::build` (the shared implementation).

use crate::state::AppState;
use ide_build::compile::{BuildEvent, BuildKind, BuildResult};
use ide_core::{IdeResult, RequestId};
#[allow(unused_imports)]
pub use ide_host::ops::build::*;
use tauri::ipc::Channel;
use tauri::{State, Window};

/// Build › Build Project / Rebuild Project / Build Module (`file`: a file or folder of the
/// module) / Build Artifacts, streaming the console and messages on `on_event`; cancellable
/// with `cancel_request(request)`. Diagnostics of `clear` (the files of the previous build)
/// are cleared first; the new messages are published as `build` diagnostics.
#[tauri::command]
#[specta::specta]
pub async fn build_run(
    window: Window,
    state: State<'_, AppState>,
    kind: BuildKind,
    file: Option<std::path::PathBuf>,
    request: RequestId,
    clear: Vec<std::path::PathBuf>,
    on_event: Channel<BuildEvent>,
) -> IdeResult<BuildResult> {
    ide_host::ops::build::build_run(
        &state.host,
        window.label(),
        kind,
        file,
        request,
        clear,
        crate::commands::sender(on_event),
    )
    .await
}
