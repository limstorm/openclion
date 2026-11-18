//! The custom 40-point title bar: project widget, run configuration and the
//! IDE-wide actions, matching `TitleBar.module.css`.

use nsh_core::element::FontWeight;
use nsh_core::prelude::*;

use crate::icons;
use crate::model::State;
use crate::theme::{Palette, UI_FONT};
use crate::ui::{icon, text_ui, tool_button};
use nsh_core::element::desktop::title_bar;

pub fn bar(state: State, pal: Palette) -> impl IntoElement {
    view()
        .h(px(40.0))
        .flex_none()
        .w_full()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .pl(px(10.0))
        .pr(titlebar_right())
        .window_drag(true)
        .child(project_button(state, pal))
        .child(view().flex_1().h_full())
        .child(config_widget(state, pal))
        .child(
            tool_button(icons::RUN_COLORED, 30.0, 16.0, pal.fg_secondary, pal, move || {
                state.status.set("No run configuration — select one first".to_owned());
            })
            .window_drag(false),
        )
        .child(
            tool_button(icons::DEBUG_COLORED, 30.0, 16.0, pal.fg_secondary, pal, move || {
                state.status.set("No run configuration — select one first".to_owned());
            })
            .window_drag(false),
        )
        .child(
            tool_button(icons::MORE_VERTICAL, 30.0, 16.0, pal.fg_secondary, pal, move || {
                state.status.set("Run Anything — Ctrl+Ctrl, or select a configuration".to_owned());
            })
            .window_drag(false),
        )
        .child(
            tool_button(icons::SEARCH, 30.0, 16.0, pal.fg_secondary, pal, move || {
                state.quick_query.set(String::new());
                state.quick_open.set(true);
            })
            .window_drag(false),
        )
        .child(
            tool_button(icons::SETTINGS, 30.0, 16.0, pal.fg_secondary, pal, move || {
                state.settings.set(true);
            })
            .window_drag(false),
        )
}

fn project_button(state: State, pal: Palette) -> impl IntoElement {
    let name = state.root_name();
    let initials = project_initials(&name);
    view()
        .h(px(30.0))
        .flex_none()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .px(px(8.0))
        .rounded(px(8.0))
        .hover(move |s| s.bg(pal.toolbar_hover))
        .active(move |s| s.bg(pal.toolbar_pressed))
        .window_drag(false)
        .on_press(move || state.project_menu.update(|open| *open = !*open))
        .child(
            view()
                .size(px(20.0))
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(px(4.0))
                .bg_linear_to_br()
                .gradient_from(pal.project_color)
                .gradient_to(pal.project_color_end)
                .child(text_ui(initials, 10.0, white().into()).font_weight(FontWeight::SEMIBOLD)),
        )
        .child(text_ui(name, 13.0, pal.fg_strong).font_family(UI_FONT.family).font_weight(FontWeight::MEDIUM))
        .child(icon(icons::CHEVRON_DOWN, 12.0, pal.fg_muted))
}

fn config_widget(state: State, pal: Palette) -> impl IntoElement {
    view()
        .h(px(30.0))
        .flex_none()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .px(px(8.0))
        .min_w(px(150.0))
        .rounded(px(8.0))
        .hover(move |s| s.bg(pal.toolbar_hover))
        .active(move |s| s.bg(pal.toolbar_pressed))
        .window_drag(false)
        .on_press(move || {
            state.status.set("Run configurations: acme (Debug), acme_tests".to_owned());
        })
        .child(text_ui("Select Configuration", 13.0, pal.fg_secondary))
        .child(icon(icons::CHEVRON_DOWN, 12.0, pal.fg_muted))
}

fn project_initials(name: &str) -> String {
    let mut out = String::new();
    for part in name.split(['-', '_', ' ']).filter(|p| !p.is_empty()) {
        if let Some(c) = part.chars().next() {
            out.extend(c.to_uppercase());
        }
        if out.chars().count() >= 2 {
            break;
        }
    }
    if out.is_empty() { name.chars().take(2).collect() } else { out }
}

/// Room the platform's own window buttons take on the right of the bar.
fn titlebar_right() -> Points {
    let measured = title_bar().right;
    #[cfg(target_os = "linux")]
    {
        // GTK places the window controls over the app; leave them their room.
        px(measured.0 + 8.0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        px(measured.0 + 8.0)
    }
}
