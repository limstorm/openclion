//! The custom 40-point title bar: project widget, run configuration and the
//! IDE-wide actions, matching `TitleBar.module.css`.

use nsh_core::element::FontWeight;
use nsh_core::prelude::*;

use crate::icons;
use crate::model::State;
use crate::theme::{Palette, UI_FONT};
use crate::ui::{icon, text_ui, tool_button};
use nsh_core::desktop::WindowControl;

pub fn bar(state: State, pal: Palette) -> impl IntoElement {
    view()
        .h(px(40.0))
        .flex_none()
        .w_full()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .pl(px(8.0))
        .pr(px(0.0))
        .window_drag(true)
        .child(menu_button(state, pal))
        .child(project_button(state, pal))
        .child(view().flex_1().h_full())
        .child(config_widget(state, pal))
        .child(
            tool_button(icons::RUN_COLORED, 30.0, 16.0, pal.fg_secondary, pal, move || {
                state.run_active_configuration();
            })
            .window_drag(false),
        )
        .child(
            tool_button(icons::DEBUG_COLORED, 30.0, 16.0, pal.fg_secondary, pal, move || {
                crate::debug::start(state);
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
        .child(window_button(pal, WindowControl::Minimize))
        .child(window_button(pal, WindowControl::Maximize))
        .child(window_button(pal, WindowControl::Close))
}

fn menu_button(state: State, pal: Palette) -> impl IntoElement {
    view()
        .size(px(30.0))
        .flex_none()
        .items_center()
        .justify_center()
        .rounded(px(6.0))
        .hover(move |s| s.bg(pal.toolbar_hover))
        .window_drag(false)
        .test_id("app-menu-button")
        .on_press(move || state.app_menu.update(|open| *open = !*open))
        .child(
            view()
                .flex_col()
                .gap(px(3.0))
                .child(view().w(px(13.0)).h(px(1.5)).rounded(px(1.0)).bg(pal.fg_muted))
                .child(view().w(px(13.0)).h(px(1.5)).rounded(px(1.0)).bg(pal.fg_muted))
                .child(view().w(px(13.0)).h(px(1.5)).rounded(px(1.0)).bg(pal.fg_muted)),
        )
}

fn window_button(pal: Palette, control: WindowControl) -> impl IntoElement {
    let close = matches!(control, WindowControl::Close);
    view()
        .size(px(46.0))
        .h_full()
        .flex_none()
        .items_center()
        .justify_center()
        .window_drag(false)
        .window_control(control)
        .hover(move |s| if close { s.bg(pal.danger) } else { s.bg(pal.toolbar_hover) })
        .child(match control {
            WindowControl::Minimize => view().w(px(10.0)).h(px(1.0)).bg(pal.fg_secondary).into_element(),
            WindowControl::Maximize => {
                view().size(px(9.0)).border(px(1.0)).border_color(pal.fg_secondary).into_element()
            }
            WindowControl::Close => icon(icons::CLOSE, 11.0, pal.fg_secondary).into_element(),
        })
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
        .test_id("run-config-widget")
        .on_press(move || state.run_chooser.set(true))
        .child(text_ui(
            move || {
                state
                    .run_target
                    .get()
                    .map(|target| format!("{target} (Debug)"))
                    .unwrap_or_else(|| "Select Configuration".to_owned())
            },
            13.0,
            pal.fg_secondary,
        ))
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
