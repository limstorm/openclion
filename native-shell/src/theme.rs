//! the design tokens of OpenCLion's Islands themes, ported from
//! `src/styles/tokens.css` so the native shell draws the same colours as the
//! Tauri frontend did.

use nsh_core::element::{AdaptiveColor, FontAsset, asset};
use nsh_core::prelude::*;

pub const UI_FONT: FontAsset = asset!("Inter.ttf");
pub const MONO_FONT: FontAsset = asset!("JetBrainsMono.ttf");

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub dark: bool,
    pub bg_frame: AdaptiveColor,
    pub bg_island: AdaptiveColor,
    pub bg_elev: AdaptiveColor,
    pub bg_hover: AdaptiveColor,
    pub bg_selected: AdaptiveColor,
    pub bg_status: AdaptiveColor,
    pub border: AdaptiveColor,
    pub border_strong: AdaptiveColor,
    pub fg: AdaptiveColor,
    pub fg_strong: AdaptiveColor,
    pub fg_secondary: AdaptiveColor,
    pub fg_muted: AdaptiveColor,
    pub fg_subtle: AdaptiveColor,
    pub fg_faint: AdaptiveColor,
    pub fg_gutter: AdaptiveColor,
    pub accent: AdaptiveColor,
    pub accent_soft: AdaptiveColor,
    pub link: AdaptiveColor,
    pub danger: AdaptiveColor,
    pub warning: AdaptiveColor,
    pub success: AdaptiveColor,
    pub tab_selected: AdaptiveColor,
    pub tab_border: AdaptiveColor,
    pub tab_hover: AdaptiveColor,
    pub tree_selected: AdaptiveColor,
    pub tree_hover: AdaptiveColor,
    pub stripe_selected: AdaptiveColor,
    pub toolbar_hover: AdaptiveColor,
    pub toolbar_pressed: AdaptiveColor,
    pub project_color: AdaptiveColor,
    pub project_color_end: AdaptiveColor,
    pub build_tint: AdaptiveColor,
    pub editor_fg: AdaptiveColor,
    pub line_number: AdaptiveColor,
    pub line_number_active: AdaptiveColor,
    pub gutter_border: AdaptiveColor,
    pub selection: AdaptiveColor,
    pub caret: AdaptiveColor,
    pub search_match: AdaptiveColor,
    pub popup_shadow: AdaptiveColor,
}

impl Palette {
    pub fn light() -> Self {
        Self {
            dark: false,
            bg_frame: c(0xe9eaeeff),
            bg_island: c(0xffffffff),
            bg_elev: c(0xf7f8f9ff),
            bg_hover: c(0xedededff),
            bg_selected: c(0xd0dffeff),
            bg_status: c(0xe9eaeeff),
            border: c(0xe9eaeeff),
            border_strong: c(0xd1d3d9ff),
            fg: c(0x000000ff),
            fg_strong: c(0x000000ff),
            fg_secondary: c(0x5f6269ff),
            fg_muted: c(0x6c707eff),
            fg_subtle: c(0x73767cff),
            fg_faint: c(0x9fa2a8ff),
            fg_gutter: c(0xaeb3c2ff),
            accent: c(0x3871e1ff),
            accent_soft: c(0x3871e11f),
            link: c(0x2f5eb9ff),
            danger: c(0xc54e58ff),
            warning: c(0xa56906ff),
            success: c(0x338555ff),
            tab_selected: c(0xe3ebfeff),
            tab_border: c(0xa7c5ffff),
            tab_hover: c(0x00000012),
            tree_selected: c(0xd0dffeff),
            tree_hover: c(0x00000008),
            stripe_selected: c(0x00000020),
            toolbar_hover: c(0x00000012),
            toolbar_pressed: c(0x00000020),
            project_color: c(0xe08855ff),
            project_color_end: c(0xe9806fff),
            build_tint: c(0xf7efe6ff),
            editor_fg: c(0x080808ff),
            line_number: c(0xaeb3c2ff),
            line_number_active: c(0x767a8aff),
            gutter_border: c(0xebecf0ff),
            selection: c(0xa6d2ffff),
            caret: c(0x000000ff),
            search_match: c(0xfcd47eff),
            popup_shadow: c(0x0000002e),
        }
    }

    pub fn dark() -> Self {
        Self {
            dark: true,
            bg_frame: c(0x26282cff),
            bg_island: c(0x191a1cff),
            bg_elev: c(0x212326ff),
            bg_hover: c(0x2e2f30ff),
            bg_selected: c(0x2a4371ff),
            bg_status: c(0x26282cff),
            border: c(0x26282cff),
            border_strong: c(0x40434aff),
            fg: c(0xd1d3d9ff),
            fg_strong: c(0xdfe1e5ff),
            fg_secondary: c(0x9fa2a8ff),
            fg_muted: c(0x8c8f95ff),
            fg_subtle: c(0x73767cff),
            fg_faint: c(0x5f6269ff),
            fg_gutter: c(0x4b5059ff),
            accent: c(0x3871e1ff),
            accent_soft: c(0x3871e133),
            link: c(0x71a1feff),
            danger: c(0xf57e84ff),
            warning: c(0xd59637ff),
            success: c(0x6db083ff),
            tab_selected: c(0x233558ff),
            tab_border: c(0x2e4d89ff),
            tab_hover: c(0xffffff17),
            tree_selected: c(0x2a4371ff),
            tree_hover: c(0xffffff0f),
            stripe_selected: c(0xffffff29),
            toolbar_hover: c(0xffffff17),
            toolbar_pressed: c(0xffffff29),
            project_color: c(0xe08855ff),
            project_color_end: c(0xe9806fff),
            build_tint: c(0x3b3129ff),
            editor_fg: c(0xbcbec4ff),
            line_number: c(0x4b5059ff),
            line_number_active: c(0xa1a3abff),
            gutter_border: c(0x26282cff),
            selection: c(0x214283ff),
            caret: c(0xced0d6ff),
            search_match: c(0x114957ff),
            popup_shadow: c(0x00000073),
        }
    }

    pub fn of(dark: bool) -> Self {
        if dark { Self::dark() } else { Self::light() }
    }
}

/// An opaque AdaptiveColor.
fn c(hex: u32) -> AdaptiveColor {
    rgba(hex).into()
}
