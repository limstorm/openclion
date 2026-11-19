//! The workbench: title bar, tool window stripes, project tree, editor tabs
//! and status bar, drawn entirely in Rust with nsh's native views.

use nsh_core::element::{AdaptiveColor, FontWeight, Svg};
use nsh_core::prelude::*;

use crate::icons;
use crate::model::State;
use crate::theme::{Palette, UI_FONT};

pub mod editor_panel;
pub mod overlay;
pub mod panels;
pub mod rail;
pub mod status;
pub mod titlebar;
pub mod tree_panel;

pub fn text_ui(label: impl IntoReactive<SharedString>, size: f32, color: AdaptiveColor) -> Text {
    text(label).font_family(UI_FONT.family).text_size(px(size)).text_color(color)
}

pub fn text_strong(label: impl IntoReactive<SharedString>, size: f32, pal: Palette) -> Text {
    text_ui(label, size, pal.fg_strong).font_weight(FontWeight::MEDIUM)
}

pub fn text_muted(label: impl IntoReactive<SharedString>, size: f32, pal: Palette) -> Text {
    text_ui(label, size, pal.fg_muted)
}

pub fn icon(markup: &'static str, size: f32, color: AdaptiveColor) -> Svg {
    let icon: Svg = icons::icon(markup, size);
    icon.text_color(color)
}

pub fn tool_button(
    markup: &'static str,
    size: f32,
    icon_size: f32,
    color: AdaptiveColor,
    pal: Palette,
    on_press: impl Fn() + 'static,
) -> View {
    view()
        .size(px(size))
        .items_center()
        .justify_center()
        .rounded(px(8.0))
        .hover(move |s| s.bg(pal.toolbar_hover))
        .active(move |s| s.bg(pal.toolbar_pressed))
        .on_press(on_press)
        .child(icon(markup, icon_size, color))
}

pub fn chrome(state: State, pal: Palette) -> impl IntoElement {
    view()
        .key(if pal.dark { "workbench-dark" } else { "workbench-light" })
        .flex_1()
        .flex_col()
        .size_full()
        .bg(pal.bg_frame)
        .text_color(pal.fg)
        .font_family(UI_FONT.family)
        .child(titlebar::bar(state, pal))
        .child(
            view().flex_1().flex_row().min_h(px(0.0)).w_full().gap(px(6.0)).child(rail::left_rail(state, pal)).child(
                view()
                    .flex_1()
                    .flex_col()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .gap(px(6.0))
                    .child(
                        view()
                            .flex_1()
                            .flex_row()
                            .min_h(px(0.0))
                            .w_full()
                            .gap(px(6.0))
                            .child(dynamic(move || {
                                state.show_left.get().then(|| left_panel(state, pal).into_element())
                            }))
                            .child(
                                view()
                                    .flex_1()
                                    .flex_col()
                                    .min_w(px(200.0))
                                    .min_h(px(0.0))
                                    .child(editor_panel::editor_area(state, pal)),
                            )
                            .child(dynamic(move || {
                                state.show_right.get().then(|| panels::right_panel(state, pal).into_element())
                            }))
                            .child(rail::right_rail(state, pal)),
                    )
                    .child(dynamic(move || {
                        state.show_bottom.get().then(|| panels::bottom_panel(state, pal).into_element())
                    })),
            ),
        )
        .child(status::status_bar(state, pal))
        .child(overlay::overlays(state, pal))
}

fn left_panel(state: State, pal: Palette) -> impl IntoElement {
    view()
        .w(px(state.left_width.get()))
        .flex_none()
        .min_h(px(0.0))
        .flex_col()
        .overflow_hidden()
        .rounded(px(10.0))
        .bg(pal.bg_island)
        .child(tree_panel::left_tool(state, pal))
}
