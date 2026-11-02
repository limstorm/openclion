//! CMake tool window: the project's build directories, codemodel targets and presets.
//!
//! Reading is cheap (a `CMakeCache.txt` plus the last File API reply), so the
//! tool window reloads the model after every configure instead of caching it
//! here. Configuring and building go through the CMake run configurations of
//! `ide-run`, so both paths report into the same Run / Build tool windows.
use crate::Host;
use crate::rpc::Dispatcher;
use ide_cmake::CmakeModel;
use ide_core::{IdeError, IdeResult};

/// The CMake model of the window's project, read from disk.
pub async fn model(host: &Host, window: &str) -> IdeResult<CmakeModel> {
    host.require_trust(window)?;
    let project = host.project(window)?;
    let root = project.root.clone();
    // Profiles are project-scoped: a project may override the global CMake settings.
    let profiles = project.settings.effective().cmake.profiles;
    tokio::task::spawn_blocking(move || ide_cmake::load(&root, profiles.as_deref()))
        .await
        .map_err(|e| IdeError::internal(e.to_string()))
}

pub(crate) fn register(d: &mut Dispatcher) {
    d.add("cmake_model", |c| async move { model(&c.host, &c.window).await });
}
