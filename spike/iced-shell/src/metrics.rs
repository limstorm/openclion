//! The reference table of layout metrics copied from the web theme (`src/styles/tokens.css`), so the native shell
//! sits on the same pixel grid as the webview one instead of approximating it.
//!
//! Every value below is quoted in the comment next to it, which is the only way to keep the
//! two versions from drifting apart. Several are consumed as the corresponding component is
//! ported, so the module keeps the whole table rather than only what today's code reads.
#![allow(dead_code)]

/// `--h-titlebar: 40px`
pub const TITLEBAR: f32 = 40.0;
/// `--h-tabbar: 38px`
pub const TABBAR: f32 = 38.0;
/// `--h-tab: 28px`
pub const TAB: f32 = 28.0;
/// `--h-row: 24px` (tree and list rows)
pub const ROW: f32 = 24.0;
/// `--h-statusbar: 30px`, and `StatusBar.module.css` adds 4px: `calc(var(--h-statusbar) + 4px)`
pub const STATUSBAR: f32 = 34.0;
/// The activity stripe is 44px wide in the web layout.
pub const STRIPE: f32 = 44.0;
/// `--h-stripe-button: 48px`
pub const STRIPE_BUTTON: f32 = 48.0;
/// `--h-header: 40px` — tool window headers.
pub const HEADER: f32 = 40.0;
/// `--radius-tab: 6px`, `--radius-small: 4px`, `--radius-row: 4px`
pub const RADIUS_TAB: f32 = 6.0;
pub const RADIUS_ROW: f32 = 4.0;
pub const RADIUS_SMALL: f32 = 4.0;
/// `--fs-ui: 13px`, `--fs-small: 12px`, `--fs-tiny: 11px`, `--fs-code: 13px`
pub const FONT_UI: f32 = 13.0;
pub const FONT_SMALL: f32 = 12.0;
pub const FONT_TINY: f32 = 11.0;
pub const FONT_CODE: f32 = 13.0;
/// `line-height: 1.4` on `body`.
pub const LINE_HEIGHT: f32 = 1.4;
/// Editor line height from the web editor scheme (13px font, 1.4 ratio).
pub const EDITOR_LINE: f32 = 18.2;
/// Tree indentation per level (`ProjectView.module.css` pads each level by 12px).
pub const TREE_INDENT: f32 = 12.0;
/// Gap between a row's icon and its label (`--space` in `ProjectView.module.css`: 5px).
pub const TREE_GAP: f32 = 5.0;
/// Tree row horizontal padding.
pub const TREE_PADDING: f32 = 8.0;
/// `--island-gap: 6px` — the frame shows through between islands.
pub const ISLAND_GAP: f32 = 6.0;
/// `--island-radius: 10px`
pub const ISLAND_RADIUS: f32 = 10.0;
/// `--h-field: 28px`, `--h-button: 28px`
pub const FIELD: f32 = 28.0;
pub const BUTTON: f32 = 28.0;

/// Line height for a font size, using the web's `1.4` ratio.
pub const fn line_for(size: f32) -> f32 {
    size * LINE_HEIGHT
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 13px × 1.4 must be the editor line height the scheme uses (18.2px).
    #[test]
    fn editor_line_height_follows_the_ratio() {
        assert!((line_for(FONT_CODE) - EDITOR_LINE).abs() < 0.01);
    }

    /// The numbers the web actually paints, measured from a screenshot of the webview build:
    /// title bar 40, tab bar 38/39, status bar 34, tree row pitch 24.
    #[test]
    fn matches_the_measured_web_geometry() {
        assert_eq!(TITLEBAR, 40.0);
        assert_eq!(TABBAR, 38.0);
        assert_eq!(STATUSBAR, 34.0);
        assert_eq!(ROW, 24.0);
        assert_eq!(TAB, 28.0);
        assert!(TAB + 10.0 <= TABBAR, "a tab fits inside the tab bar");
    }
}
