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
//! The render node is only half the answer: a machine can expose DRM devices that
//! cannot render (a virtual display adapter, a kernel modesetting stub). So the
//! probe also reads the DRM driver behind every `/sys/class/drm/renderD*` node and
//! treats an all-display-only set as software rendering. Real accelerators — Intel
//! i915/xe (iGPU), amdgpu/radeon, nouveau, NVIDIA's own node, the ARM/Mali stacks —
//! keep compositing, as does `virtio_gpu` (its Mesa driver is virgl when the host
//! offers it, and llvmpipe otherwise; the environment checks below catch the
//! latter when the session says so).
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
    /// The DRM drivers behind those render nodes, from `/sys/class/drm`.
    pub gpus: Vec<Gpu>,
}

/// One DRM render node and the driver that owns it, e.g. `renderD128` → `i915`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gpu {
    pub node: String,
    pub driver: String,
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
    if !host.gpus.is_empty() && host.gpus.iter().all(|gpu| is_display_only(&gpu.driver)) {
        return Decision::Disable("only display-only DRM adapters (no GPU renderer)");
    }
    Decision::Keep
}

/// Drivers that drive a display but render nothing: firmware framebuffers, virtual
/// KMS and the paravirtual consoles of VM hypervisors.
fn is_display_only(driver: &str) -> bool {
    let driver = driver.trim().to_ascii_lowercase();
    [
        "ast",
        "aspeed",
        "vkms",
        "vboxvideo",
        "vmwgfx",
        "bochs",
        "cirrus",
        "qxl",
        "mgag200",
        "simpledrm",
        "virtio_gpu_vkms",
    ]
    .iter()
    .any(|virtual_driver| driver == *virtual_driver || driver.starts_with("simpledrm"))
}

/// Applies [`decide`] to this process. Call before GTK/WebKit start.
pub fn tune() {
    let gpus = gpus();
    let host = Host {
        override_value: env::var(OVERRIDE).ok(),
        webkit_switch_set: env::var_os(WEBKIT_SWITCH).is_some_and(|v| !v.is_empty()),
        libgl_always_software: env::var("LIBGL_ALWAYS_SOFTWARE").is_ok_and(|v| !is_falsey(&v)),
        gallium_software: env::var("GALLIUM_DRIVER").is_ok_and(|v| is_software_rasterizer(&v)),
        has_render_node: !gpus.is_empty() || dev_dri_render_node(),
        gpus,
    };
    match decide(&host) {
        Decision::Disable(reason) => {
            // SAFETY: called from `main` before any GTK/WebKit thread exists, so no
            // other thread can be reading the environment.
            unsafe { env::set_var(WEBKIT_SWITCH, "1") };
            eprintln!("openclion: {reason}: WebKit accelerated compositing disabled for this session");
        }
        Decision::Keep => match host.override_value.as_deref().map(str::trim) {
            Some(value) => {
                eprintln!("openclion: {OVERRIDE}={value}: WebKit accelerated compositing on");
            }
            None => {
                eprintln!("openclion: GPU {}: WebKit accelerated compositing on", describe(&host.gpus));
            }
        },
    }
}

/// `i915 (renderD128)`, `none` — the one-line GPU summary the start-up log carries.
fn describe(gpus: &[Gpu]) -> String {
    if gpus.is_empty() {
        return "none".to_owned();
    }
    gpus.iter()
        .map(|gpu| {
            let driver = if gpu.driver.is_empty() { "unknown driver" } else { gpu.driver.as_str() };
            format!("{driver} ({})", gpu.node)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The DRM render nodes and their drivers, from `/sys/class/drm`.
pub fn gpus() -> Vec<Gpu> {
    let Ok(entries) = std::fs::read_dir("/sys/class/drm") else {
        return Vec::new();
    };
    let mut gpus: Vec<Gpu> = entries
        .flatten()
        .filter_map(|entry| {
            let node = entry.file_name().to_str()?.to_owned();
            if !node.starts_with("render") {
                return None;
            }
            let driver = std::fs::read_link(entry.path().join("device/driver"))
                .ok()
                .and_then(|path| path.file_name().map(|name| name.to_string_lossy().into_owned()))
                .unwrap_or_default();
            Some(Gpu { node, driver })
        })
        .collect();
    gpus.sort_by(|a, b| a.node.cmp(&b.node));
    gpus
}

fn is_falsey(value: &str) -> bool {
    matches!(value.trim(), "0" | "false" | "no")
}

fn is_software_rasterizer(driver: &str) -> bool {
    let driver = driver.trim().to_ascii_lowercase();
    ["llvmpipe", "softpipe", "swrast", "lavapipe"].iter().any(|sw| driver.contains(sw))
}

/// True when `/dev/dri` holds a render node; the fallback when `/sys` is unreadable.
fn dev_dri_render_node() -> bool {
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

    fn gpu(node: &str, driver: &str) -> Gpu {
        Gpu { node: node.to_owned(), driver: driver.to_owned() }
    }

    #[test]
    fn a_real_gpu_keeps_compositing_whatever_else_is_attached() {
        let h = Host { gpus: vec![gpu("renderD128", "i915"), gpu("renderD129", "ast")], ..host() };
        assert_eq!(decide(&h), Decision::Keep);
        let amd = Host { gpus: vec![gpu("renderD128", "amdgpu")], ..host() };
        assert_eq!(decide(&amd), Decision::Keep);
        let virgl = Host { gpus: vec![gpu("renderD128", "virtio_gpu")], ..host() };
        assert_eq!(decide(&virgl), Decision::Keep);
    }

    #[test]
    fn display_only_adapters_are_software() {
        let h = Host { gpus: vec![gpu("renderD128", "ast"), gpu("renderD129", "vkms")], ..host() };
        assert_eq!(decide(&h), Decision::Disable("only display-only DRM adapters (no GPU renderer)"));
        for driver in ["ast", "aspeed", "vkms", "vboxvideo", "bochs", "cirrus", "qxl", "mgag200", "simpledrm"] {
            assert!(is_display_only(driver), "{driver}");
        }
        for driver in ["i915", "xe", "amdgpu", "radeon", "nouveau", "nvidia", "virtio_gpu", "panfrost", ""] {
            assert!(!is_display_only(driver), "{driver}");
        }
    }

    #[test]
    fn the_gpu_summary_names_the_driver_and_node() {
        assert_eq!(
            describe(&[gpu("renderD128", "i915"), gpu("renderD129", "amdgpu")]),
            "i915 (renderD128), amdgpu (renderD129)"
        );
        assert_eq!(describe(&[gpu("renderD128", "")]), "unknown driver (renderD128)");
        assert_eq!(describe(&[]), "none");
    }

    #[test]
    fn falsey_values_do_not_count_as_forced_software() {
        assert!(is_falsey("0"));
        assert!(is_falsey("false"));
        assert!(!is_falsey("1"));
        assert!(!is_falsey("true"));
    }
}
