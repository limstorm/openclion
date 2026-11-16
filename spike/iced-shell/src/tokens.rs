//! Theme tokens, mirroring the CSS variables the web workbench uses (`--panel`, `--border`,
//! `--text`, `--accent`, …) so a native build can be compared side by side with the web one.

use iced::{Border, Color};

#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    pub background: Color,
    pub panel: Color,
    pub panel_hover: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub accent_muted: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
}

impl Tokens {
    pub const fn dark() -> Self {
        Self {
            background: Color::from_rgb(0.118, 0.121, 0.133),
            panel: Color::from_rgb(0.169, 0.176, 0.188),
            panel_hover: Color::from_rgb(0.208, 0.216, 0.235),
            border: Color::from_rgb(0.224, 0.239, 0.263),
            text: Color::from_rgb(0.875, 0.882, 0.910),
            muted: Color::from_rgb(0.549, 0.565, 0.608),
            accent: Color::from_rgb(0.208, 0.455, 0.941),
            accent_muted: Color::from_rgba(0.208, 0.455, 0.941, 0.22),
            success: Color::from_rgb(0.380, 0.749, 0.482),
            warning: Color::from_rgb(0.937, 0.718, 0.337),
            error: Color::from_rgb(0.925, 0.443, 0.447),
        }
    }

    pub const fn light() -> Self {
        Self {
            background: Color::from_rgb(1.0, 1.0, 1.0),
            panel: Color::from_rgb(0.953, 0.957, 0.969),
            panel_hover: Color::from_rgb(0.910, 0.918, 0.933),
            border: Color::from_rgb(0.859, 0.871, 0.894),
            text: Color::from_rgb(0.169, 0.176, 0.188),
            muted: Color::from_rgb(0.424, 0.439, 0.478),
            accent: Color::from_rgb(0.208, 0.455, 0.941),
            accent_muted: Color::from_rgba(0.208, 0.455, 0.941, 0.15),
            success: Color::from_rgb(0.263, 0.631, 0.376),
            warning: Color::from_rgb(0.792, 0.573, 0.157),
            error: Color::from_rgb(0.804, 0.298, 0.302),
        }
    }

    pub fn border_style(&self) -> Border {
        Border { color: self.border, width: 1.0, ..Border::default() }
    }
}
