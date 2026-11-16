//! The web theme's colours and surfaces, copied from `src/styles/tokens.css` (Islands Light is
//! the default there, so it is the default here too). Names follow the CSS variables so a
//! mismatch is easy to spot: `bg_app` is `--bg-app`, `fg_muted` is `--fg-muted`, and so on.

use iced::{Border, Color};

#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    /// `--bg-frame` — the window chrome between islands.
    pub frame: Color,
    /// `--bg-app` — the editor surface.
    pub app: Color,
    /// `--bg-elev` — panels.
    pub elev: Color,
    /// `--bg-hover`
    pub hover: Color,
    /// `--border`
    pub border: Color,
    /// `--border-strong`
    pub border_strong: Color,
    /// `--fg`
    pub fg: Color,
    /// `--fg-secondary`
    pub fg_secondary: Color,
    /// `--fg-muted`
    pub fg_muted: Color,
    /// `--accent`
    pub accent: Color,
    /// `--bg-selected`
    pub selected: Color,
    /// `--bg-tree-selected`
    pub tree_selected: Color,
    /// `--bg-tab-selected`, `--border-tab-selected`
    pub tab_selected: Color,
    pub tab_selected_border: Color,
    /// `--caret`
    pub caret: Color,
    /// `--danger`, `--warning`, `--success`
    pub danger: Color,
    pub warning: Color,
    pub success: Color,
}

impl Tokens {
    /// `:root` — Islands Dark.
    pub const fn dark() -> Self {
        Self {
            frame: Color::from_rgb(0.149, 0.157, 0.173),               // #26282c
            app: Color::from_rgb(0.098, 0.102, 0.110),                 // #191a1c
            elev: Color::from_rgb(0.129, 0.137, 0.149),                // #212326
            hover: Color::from_rgb(0.180, 0.184, 0.188),               // #2e2f30
            border: Color::from_rgb(0.149, 0.157, 0.173),              // #26282c
            border_strong: Color::from_rgb(0.251, 0.263, 0.290),       // #40434a
            fg: Color::from_rgb(0.820, 0.827, 0.839),                  // #d1d3d9
            fg_secondary: Color::from_rgb(0.624, 0.635, 0.659),        // #9fa2a8
            fg_muted: Color::from_rgb(0.549, 0.561, 0.584),            // #8c8f95
            accent: Color::from_rgb(0.220, 0.443, 0.882),              // #3871e1
            selected: Color::from_rgb(0.165, 0.263, 0.443),            // #2a4371
            tree_selected: Color::from_rgb(0.165, 0.263, 0.443),       // #2a4371
            tab_selected: Color::from_rgb(0.137, 0.208, 0.345),        // #233558
            tab_selected_border: Color::from_rgb(0.180, 0.302, 0.537), // #2e4d89
            caret: Color::from_rgb(0.808, 0.816, 0.839),               // #ced0d6
            danger: Color::from_rgb(0.961, 0.494, 0.518),              // #f57e84
            warning: Color::from_rgb(0.835, 0.588, 0.216),             // #d59637
            success: Color::from_rgb(0.427, 0.690, 0.514),             // #6db083
        }
    }

    /// `[data-theme="light"]` — Islands Light, the web default.
    pub const fn light() -> Self {
        Self {
            frame: Color::from_rgb(0.914, 0.918, 0.933),             // #e9eaee
            app: Color::from_rgb(1.0, 1.0, 1.0),                     // #ffffff
            elev: Color::from_rgb(0.969, 0.973, 0.976),              // #f7f8f9
            hover: Color::from_rgb(0.929, 0.929, 0.929),             // #ededed
            border: Color::from_rgb(0.914, 0.918, 0.933),            // #e9eaee
            border_strong: Color::from_rgb(0.820, 0.827, 0.851),     // #d1d3d9
            fg: Color::from_rgb(0.0, 0.0, 0.0),                      // #000000
            fg_secondary: Color::from_rgb(0.373, 0.384, 0.408),      // #5f6269
            fg_muted: Color::from_rgb(0.451, 0.463, 0.486),          // #73767c
            accent: Color::from_rgb(0.220, 0.443, 0.882),            // #3871e1
            selected: Color::from_rgb(0.816, 0.875, 0.996),          // #d0dffe
            tree_selected: Color::from_rgb(0.816, 0.875, 0.996),     // #d0dffe
            tab_selected: Color::from_rgb(0.890, 0.922, 0.996),      // #e3ebfe
            tab_selected_border: Color::from_rgb(0.655, 0.773, 1.0), // #a7c5ff
            caret: Color::from_rgb(0.0, 0.0, 0.0),                   // #000000
            danger: Color::from_rgb(0.961, 0.494, 0.518),            // #f57e84
            warning: Color::from_rgb(0.835, 0.588, 0.216),           // #d59637
            success: Color::from_rgb(0.427, 0.690, 0.514),           // #6db083
        }
    }

    pub fn border_style(&self) -> Border {
        Border { color: self.border, width: 1.0, ..Border::default() }
    }
}
