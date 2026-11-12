//! "Create Shortcut…" (Help menu): a launcher for this application that does not go
//! through the package manager.
//!
//! Linux writes freedesktop `.desktop` entries — one in the applications menu
//! (`~/.local/share/applications`) and one on the desktop when there is one. Windows
//! writes a `.lnk` on the desktop through WScript.Shell. macOS has no equivalent to
//! add (the bundle lives in /Applications and the Dock is the launcher), so the menu
//! item is not shown there.

use ide_core::{IdeError, IdeResult};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Type, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Shortcut {
    /// Launchers written, menu entry first.
    pub paths: Vec<ide_core::IpcPath>,
    /// The executable the launchers start.
    pub target: ide_core::IpcPath,
}

#[tauri::command]
#[specta::specta]
pub fn create_shortcut() -> IdeResult<Shortcut> {
    create()
}

fn executable() -> IdeResult<PathBuf> {
    std::env::current_exe().map_err(|e| IdeError::internal(format!("Cannot locate the running application: {e}")))
}

#[cfg(target_os = "linux")]
fn create() -> IdeResult<Shortcut> {
    let exe = executable()?;
    let applications =
        dirs::data_dir().ok_or_else(|| IdeError::unavailable("No data directory for this user"))?.join("applications");
    let body = desktop_entry(&exe);
    let mut paths = Vec::new();
    let menu = applications.join("openclion.desktop");
    write_launcher(&menu, &body, &mut paths)?;
    // The desktop copy is separate from the menu entry (GNOME needs both), but not when
    // the desktop directory *is* the applications directory.
    if let Some(desktop) = dirs::desktop_dir() {
        let file = desktop.join("openclion.desktop");
        if file != menu {
            write_launcher(&file, &body, &mut paths)?;
        }
    }
    Ok(Shortcut { paths: paths.into_iter().map(Into::into).collect(), target: exe.into() })
}

#[cfg(not(target_os = "linux"))]
fn create() -> IdeResult<Shortcut> {
    windows_lnk()
}

#[cfg(target_os = "linux")]
fn desktop_entry(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.5\n\
         Name=OpenCLion\n\
         GenericName=IDE\n\
         Comment=An open-source C/C++ IDE\n\
         Exec={}\n\
         Icon={}\n\
         Terminal=false\n\
         Categories=Development;IDE;\n\
         StartupWMClass=openclion\n",
        exec_field(exe),
        icon_field()
    )
}

/// The `Exec` value: absolute, quoted when the path has characters `.desktop` treats specially.
#[cfg(target_os = "linux")]
fn exec_field(exe: &Path) -> String {
    let path = exe.to_string_lossy();
    if path.contains(' ') || path.contains('\t') {
        format!("\"{}\"", path.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        path.into_owned()
    }
}

/// An icon the launcher can resolve: the installed hicolor entries first, else the theme name
/// (which is what the bundled package installs).
#[cfg(target_os = "linux")]
fn icon_field() -> String {
    let mut candidates: Vec<PathBuf> = Vec::new();
    for root in ["/usr/share", "/usr/local/share"] {
        candidates.push(PathBuf::from(root).join("icons/hicolor/128x128/apps/openclion.png"));
    }
    if let Some(data) = dirs::data_dir() {
        candidates.push(data.join("icons/hicolor/128x128/apps/openclion.png"));
    }
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "openclion".to_owned())
}

#[cfg(target_os = "linux")]
fn write_launcher(path: &PathBuf, body: &str, paths: &mut Vec<PathBuf>) -> IdeResult<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| IdeError::internal(format!("Cannot create {}: {e}", dir.display())))?;
    }
    std::fs::write(path, body).map_err(|e| IdeError::internal(format!("Cannot write {}: {e}", path.display())))?;
    // Desktop environments only run launchers that are executable.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
    }
    // GNOME additionally wants the file marked trusted; `gio` is absent on other desktops.
    let _ = std::process::Command::new("gio")
        .args(["set", &path.to_string_lossy(), "metadata::trusted", "true"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    paths.push(path.clone());
    Ok(())
}

/// Windows: a `.lnk` on the desktop, written by WScript.Shell.
#[cfg(not(target_os = "linux"))]
fn windows_lnk() -> IdeResult<Shortcut> {
    #[cfg(not(windows))]
    {
        return Err(IdeError::unavailable("Create Shortcut is not available on this platform"));
    }
    #[cfg(windows)]
    {
        let exe = executable()?;
        let desktop = dirs::desktop_dir().ok_or_else(|| IdeError::unavailable("No desktop directory for this user"))?;
        std::fs::create_dir_all(&desktop)
            .map_err(|e| IdeError::internal(format!("Cannot create {}: {e}", desktop.display())))?;
        let lnk = desktop.join("OpenCLion.lnk");
        let working_dir = exe.parent().unwrap_or(&desktop);
        let script = format!(
            "$s = (New-Object -ComObject WScript.Shell).CreateShortcut('{}'); \
             $s.TargetPath = '{}'; $s.WorkingDirectory = '{}'; $s.IconLocation = '{}'; $s.Save()",
            ps_quote(&lnk),
            ps_quote(&exe),
            ps_quote(working_dir),
            ps_quote(&exe),
        );
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
            .map_err(|e| IdeError::unavailable(format!("Cannot run PowerShell: {e}")))?;
        if !output.status.success() {
            return Err(IdeError::internal(format!(
                "PowerShell could not create the shortcut: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(Shortcut { paths: vec![lnk.into()], target: exe.into() })
    }
}

#[cfg(windows)]
fn ps_quote(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\'', "''")
}
