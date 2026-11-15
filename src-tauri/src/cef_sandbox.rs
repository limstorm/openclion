//! Keeps a `cargo build` tree runnable by its owner.
//!
//! Chromium's setuid sandbox helper has to be `root:root` with mode `4755`; a build copies
//! a fresh `chrome-sandbox` next to the binary with the building user's ownership and mode
//! `0755`, and Chromium then aborts at start-up instead of falling back:
//!
//! ```text
//! The SUID sandbox helper binary was found, but is not configured correctly.
//! Rather than run without sandboxing I'm aborting now.
//! ```
//!
//! The CEF runtime's own policy handles a *missing* helper (it warns and continues), but a
//! present-and-broken one is fatal before that. So a tree that cannot provide a usable
//! helper asks for `SandboxPolicy::Disabled` explicitly, and says so once at start-up.
//! Packages are unaffected: `postinst` installs the helper `root:root 4755`.

use tauri_runtime_cef::SandboxPolicy;

/// The sandbox policy this executable needs, or `None` to let the runtime decide.
pub fn policy() -> Option<SandboxPolicy> {
    #[cfg(target_os = "linux")]
    {
        let helper = std::env::current_exe().ok()?.parent()?.join("chrome-sandbox");
        let Ok(metadata) = std::fs::metadata(&helper) else {
            // No helper at all: the runtime warns and runs without a sandbox if user
            // namespaces are unavailable, which is the same outcome as below.
            return None;
        };
        use std::os::unix::fs::MetadataExt;
        let setuid = metadata.mode() & 0o4000 != 0;
        let root_owned = metadata.uid() == 0;
        if setuid && root_owned {
            return None;
        }
        eprintln!(
            "openclion: {} is not root-owned 4755, so Chromium's sandbox cannot start: running without it \
             (an installed package sets this up, see scripts/cef-sandbox.sh)",
            helper.display()
        );
        Some(SandboxPolicy::Disabled)
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}
