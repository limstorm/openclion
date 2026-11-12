//! Picks the WebView rendering path the host can actually carry.
//!
//! WebKitGTK composites the page through GL unless `WEBKIT_DISABLE_COMPOSITING_MODE`
//! is set. That is the right choice on a GPU, but without one — VMs, containers,
//! remote desktops, sessions whose driver fell back to llvmpipe — every composited
//! frame is rasterized on the CPU, and the compositor then costs more than the
//! plain cairo path it wraps. Measured on a VM with llvmpipe, a 12k-line C++ file
//! open (see `docs/perf/webkit-wayland.md`):
//!
//! | Interaction                | compositing on      | compositing off     |
//! | -------------------------- | ------------------- | ------------------- |
//! | window resize, 1.4 s       | 23 frames, max 668 ms | 86 frames, max 49 ms |
//! | wheel scroll, 1.2 s        | 65 frames, p95 130 ms | 103 frames, p95 75 ms |
//!
//! Wayland makes this worse than X11: the compositor drives resizes frame by
//! frame, so a 600 ms frame is a window that lags the pointer instead of a
//! one-off hitch. So when there is no DRM render node, hand WebKit the cairo path.
//!
//! An explicit `WEBKIT_DISABLE_COMPOSITING_MODE` always wins (that is WebKit's own
//! switch, documented by the user's environment), and `OPENCLION_WEBKIT_COMPOSITING`
//! overrides the heuristic either way.

use std::env;

/// WebKit's own switch: `1` disables accelerated compositing.
pub const WEBKIT_SWITCH: &str = "WEBKIT_DISABLE_COMPOSITING_MODE";
/// OpenCLion's override: `on`/`1` keeps compositing, `off`/`0` disables it.
pub const OVERRIDE: &str = "OPENCLION_WEBKIT_COMPOSITING";

/// What [`decide`] says about this host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Accelerated compositing stays on (the default, and what a GPU wants).
    Keep,
    /// Turn it off; the reason is logged once at start-up.
    Disable(&'static str),
}

/// Everything [`decide`] looks at, so the heuristic is a pure function of the host.
#[derive(Debug, Clone, Default)]
pub struct Host {
    /// `OPENCLION_WEBKIT_COMPOSITING`.
    pub override_value: Option<String>,
    /// Whether `WEBKIT_DISABLE_COMPOSITING_MODE` is already set.
    pub webkit_switch_set: bool,
    /// `LIBGL_ALWAYS_SOFTWARE` set to something other than `0`/`false`.
    pub libgl_always_software: bool,
    /// `GALLIUM_DRIVER` naming a software rasterizer (`llvmpipe`, `softpipe`, …).
    pub gallium_software: bool,
    /// A DRM render node (`/dev/dri/renderD*`) exists.
    pub has_render_node: bool,
}

/// Chooses the rendering path for `host`.
pub fn decide(host: &Host) -> Decision {
    match host.override_value.as_deref().map(str::trim) {
        Some("0" | "off" | "false" | "no") => return Decision::Disable("OPENCLION_WEBKIT_COMPOSITING turned it off"),
        Some("1" | "on" | "true" | "yes") => return Decision::Keep,
        _ => {}
    }
    // Whoever set WebKit's own switch made this decision already.
    if host.webkit_switch_set {
        return Decision::Keep;
    }
    if host.libgl_always_software {
        return Decision::Disable("LIBGL_ALWAYS_SOFTWARE is set");
    }
    if host.gallium_software {
        return Decision::Disable("GALLIUM_DRIVER names a software rasterizer");
    }
    if !host.has_render_node {
        return Decision::Disable("no /dev/dri/renderD* render node (software GL)");
    }
    Decision::Keep
}

/// Applies [`decide`] to this process. Call before GTK/WebKit start.
pub fn tune() {
    let host = Host {
        override_value: env::var(OVERRIDE).ok(),
        webkit_switch_set: env::var_os(WEBKIT_SWITCH).is_some_and(|v| !v.is_empty()),
        libgl_always_software: env::var("LIBGL_ALWAYS_SOFTWARE").is_ok_and(|v| !is_falsey(&v)),
        gallium_software: env::var("GALLIUM_DRIVER").is_ok_and(|v| is_software_rasterizer(&v)),
        has_render_node: has_render_node(),
    };
    if let Decision::Disable(reason) = decide(&host) {
        // SAFETY: called from `main` before any GTK/WebKit thread exists, so no
        // other thread can be reading the environment.
        unsafe { env::set_var(WEBKIT_SWITCH, "1") };
        eprintln!("openclion: {reason}: WebKit accelerated compositing disabled for this session");
    }
}

fn is_falsey(value: &str) -> bool {
    matches!(value.trim(), "0" | "false" | "no")
}

fn is_software_rasterizer(driver: &str) -> bool {
    let driver = driver.trim().to_ascii_lowercase();
    ["llvmpipe", "softpipe", "swrast", "lavapipe"].iter().any(|sw| driver.contains(sw))
}

/// True when the kernel exposes a DRM render node, i.e. a GPU that can render.
fn has_render_node() -> bool {
    std::fs::read_dir("/dev/dri")
        .map(|entries| {
            entries.flatten().any(|entry| entry.file_name().to_str().is_some_and(|name| name.starts_with("renderD")))
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> Host {
        Host { has_render_node: true, ..Host::default() }
    }

    #[test]
    fn a_gpu_keeps_compositing() {
        assert_eq!(decide(&host()), Decision::Keep);
    }

    #[test]
    fn no_render_node_turns_it_off() {
        let h = Host { has_render_node: false, ..host() };
        assert_eq!(decide(&h), Decision::Disable("no /dev/dri/renderD* render node (software GL)"));
    }

    #[test]
    fn a_software_gl_environment_turns_it_off_even_with_a_node() {
        let h = Host { libgl_always_software: true, ..host() };
        assert!(matches!(decide(&h), Decision::Disable(_)));
        let h = Host { gallium_software: true, ..host() };
        assert!(matches!(decide(&h), Decision::Disable(_)));
        assert_eq!(
            decide(&Host { gallium_software: true, has_render_node: false, ..host() }),
            Decision::Disable("GALLIUM_DRIVER names a software rasterizer")
        );
    }

    #[test]
    fn webkits_own_switch_is_left_alone() {
        let h = Host { webkit_switch_set: true, has_render_node: false, ..host() };
        assert_eq!(decide(&h), Decision::Keep);
    }

    #[test]
    fn the_override_wins_either_way() {
        let off = Host { override_value: Some("off".into()), has_render_node: true, ..host() };
        assert!(matches!(decide(&off), Decision::Disable(_)));
        let on = Host { override_value: Some("on".into()), has_render_node: false, ..host() };
        assert_eq!(decide(&on), Decision::Keep);
        let junk = Host { override_value: Some("maybe".into()), has_render_node: false, ..host() };
        assert!(matches!(decide(&junk), Decision::Disable(_)));
    }

    #[test]
    fn software_driver_names_are_recognised() {
        for name in ["llvmpipe", "LLVMpipe", "softpipe", "swrast", "lavapipe"] {
            assert!(is_software_rasterizer(name), "{name}");
        }
        for name in ["iris", "radeonsi", "zink", "virgl"] {
            assert!(!is_software_rasterizer(name), "{name}");
        }
    }

    #[test]
    fn falsey_values_do_not_count_as_forced_software() {
        assert!(is_falsey("0"));
        assert!(is_falsey("false"));
        assert!(!is_falsey("1"));
        assert!(!is_falsey("true"));
    }
}
