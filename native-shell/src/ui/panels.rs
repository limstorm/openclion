//! The right tool window (CMake, Notifications, AI, Database) and the
//! bottom tool window (Terminal, Problems, Run, Services).

use nsh_core::element::FontWeight;
use nsh_core::element::input;
use nsh_core::prelude::*;

use crate::icons;
use crate::model::{BottomTool, RightTool, State};
use crate::theme::{MONO_FONT, Palette};
use crate::ui::{icon, text_muted, text_strong, text_ui};

pub fn right_panel(state: State, pal: Palette) -> impl IntoElement {
    view()
        .w(px(state.right_width.get()))
        .flex_none()
        .min_h(px(0.0))
        .flex_col()
        .overflow_hidden()
        .rounded(px(10.0))
        .bg(pal.bg_island)
        .child(right_header(state, pal))
        .child(view().flex_1().min_h(px(0.0)).child(dynamic(move || match state.right_tool.get() {
            RightTool::Cmake => cmake_body(state, pal).into_element(),
            RightTool::Notifications => notifications_body(state, pal).into_element(),
            RightTool::Agents => agents_body(state, pal).into_element(),
            RightTool::Database => database_body(state, pal).into_element(),
        })))
}

fn right_header(state: State, pal: Palette) -> impl IntoElement {
    let title = match state.right_tool.get() {
        RightTool::Cmake => "CMake",
        RightTool::Notifications => "Notifications",
        RightTool::Agents => "AI Assistant",
        RightTool::Database => "Database",
    };
    view()
        .h(px(40.0))
        .flex_none()
        .w_full()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .pl(px(12.0))
        .pr(px(6.0))
        .child(text_strong(title, 13.0, pal))
        .child(view().flex_1())
        .child(header_action(pal, icons::REFRESH, move || {
            state.status.set("CMake project reloaded".to_owned());
        }))
        .child(header_action(pal, icons::MORE_HORIZONTAL, move || {
            state.status.set("CMake tool window actions".to_owned());
        }))
        .child(header_action(pal, icons::CLOSE, move || {
            state.show_right.set(false);
        }))
}

fn header_action(pal: Palette, markup: &'static str, on_press: impl Fn() + 'static) -> View {
    view()
        .size(px(24.0))
        .flex_none()
        .items_center()
        .justify_center()
        .rounded(px(5.0))
        .hover(move |s| s.bg(pal.toolbar_hover))
        .on_press(on_press)
        .child(icon(markup, 16.0, pal.fg_muted))
}

fn cmake_body(state: State, pal: Palette) -> impl IntoElement {
    let root = state.root_name();
    scroll().flex_1().w_full().min_h(px(0.0)).child(
        view()
            .flex_col()
            .w_full()
            .py(px(6.0))
            .child(panel_row(pal, 0, true, icons::CMAKE, root, None, true))
            .child(panel_row(pal, 1, true, icons::CMAKE, "Debug".to_owned(), Some("build".to_owned()), false))
            .child(panel_row(pal, 2, true, icons::FOLDER, "Targets".to_owned(), None, false))
            .child(panel_row(pal, 3, false, icons::CMAKE, "acme".to_owned(), Some("01".to_owned()), false))
            .child(panel_row(pal, 3, false, icons::CMAKE, "acme_tests".to_owned(), Some("01".to_owned()), false))
            .child(panel_row(
                pal,
                4,
                false,
                icons::CMAKE,
                "greeter".to_owned(),
                Some("static library".to_owned()),
                false,
            ))
            .child(panel_row(
                pal,
                1,
                false,
                crate::icons::file_icon("compile_commands.json"),
                "compile_commands.json".to_owned(),
                None,
                false,
            )),
    )
}

fn panel_row(
    pal: Palette,
    depth: usize,
    chevron: bool,
    icon_markup: &'static str,
    label: String,
    detail: Option<String>,
    bold: bool,
) -> View {
    view()
        .h(px(24.0))
        .flex_none()
        .mx(px(4.0))
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .pl(px(6.0 + 14.0 * depth as f32))
        .pr(px(6.0))
        .rounded(px(5.0))
        .hover(move |s| s.bg(pal.tree_hover))
        .child(view().w(px(12.0)).flex_none().items_center().justify_center().child(icon(
            if chevron { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT },
            12.0,
            if chevron { pal.fg_secondary } else { transparent_black().into() },
        )))
        .child(icon(icon_markup, 16.0, pal.fg_muted))
        .child(if bold {
            text_strong(label, 13.0, pal).into_element()
        } else {
            text_ui(label, 13.0, pal.fg).into_element()
        })
        .child(if let Some(detail) = detail {
            if detail.len() <= 2 {
                view()
                    .ml(px(2.0))
                    .px(px(4.0))
                    .rounded(px(3.0))
                    .bg(pal.bg_elev)
                    .child(text_ui(detail, 10.0, pal.fg_muted))
                    .into_element()
            } else {
                text_ui(detail, 12.0, pal.fg_subtle).ml(px(2.0)).flex_none().into_element()
            }
        } else {
            view().into_element()
        })
        .child(view().flex_1())
}

fn notifications_body(state: State, pal: Palette) -> impl IntoElement {
    view()
        .flex_col()
        .p(px(10.0))
        .gap(px(8.0))
        .child(
            view()
                .rounded(px(8.0))
                .border(px(1.0))
                .border_color(pal.border_strong)
                .bg(pal.bg_elev)
                .p(px(10.0))
                .gap(px(4.0))
                .child(text_ui("Native shell ready", 13.0, pal.fg_strong))
                .child(text_muted(
                    "OpenCLion is running on nsh: Rust views, no web view and no JS runtime.",
                    12.0,
                    pal,
                )),
        )
        .child(
            view()
                .rounded(px(8.0))
                .border(px(1.0))
                .border_color(pal.border)
                .p(px(10.0))
                .gap(px(4.0))
                .child(text_ui("CMake model reloaded", 13.0, pal.fg_strong))
                .child(text_muted("2 targets, 1 static library", 12.0, pal)),
        )
        .child(
            view()
                .h(px(28.0))
                .items_center()
                .justify_center()
                .rounded(px(6.0))
                .hover(move |s| s.bg(pal.toolbar_hover))
                .on_press(move || {
                    state.status.set("Notifications cleared".to_owned());
                })
                .child(text_ui("Clear all", 12.0, pal.fg_secondary)),
        )
}

fn agents_body(state: State, pal: Palette) -> impl IntoElement {
    view()
        .flex_col()
        .w_full()
        .p(px(10.0))
        .gap(px(8.0))
        .child(text_ui("AI Assistant", 13.0, pal.fg_strong).font_weight(FontWeight::SEMIBOLD))
        .child(text_muted(
            "Ask about the workspace, generate a patch, or explain the selection. This native build keeps the action surface local: no script runtime is embedded.",
            12.0,
            pal,
        ))
        .child(
            view()
                .rounded(px(8.0))
                .border(px(1.0))
                .border_color(pal.border_strong)
                .bg(pal.bg_elev)
                .p(px(8.0))
                .gap(px(6.0))
                .child(text_muted("Try:", 11.0, pal))
                .child(chip(state, pal, "Explain main.cpp"))
                .child(chip(state, pal, "Build acme (Debug)"))
                .child(chip(state, pal, "Find TODOs in the project")),
        )
}

fn chip(state: State, pal: Palette, label: &'static str) -> View {
    view()
        .px(px(8.0))
        .py(px(5.0))
        .rounded(px(5.0))
        .bg(pal.bg_island)
        .border(px(1.0))
        .border_color(pal.border)
        .hover(move |s| s.bg(pal.toolbar_hover))
        .on_press(move || state.status.set(format!("AI: {label}")))
        .child(text_ui(label, 12.0, pal.fg_secondary))
}

fn database_body(state: State, pal: Palette) -> impl IntoElement {
    view()
        .flex_col()
        .w_full()
        .p(px(10.0))
        .gap(px(8.0))
        .child(text_ui("Database", 13.0, pal.fg_strong).font_weight(FontWeight::SEMIBOLD))
        .child(text_muted("No data sources are attached to this workspace.", 12.0, pal))
        .child(
            view()
                .h(px(28.0))
                .items_center()
                .justify_center()
                .rounded(px(6.0))
                .border(px(1.0))
                .border_color(pal.border_strong)
                .hover(move |s| s.bg(pal.toolbar_hover))
                .on_press(move || {
                    state.status.set("Add data source".to_owned());
                })
                .child(text_ui("Add data source", 12.0, pal.fg_secondary)),
        )
}

// ---------------------------------------------------------------------------
// Bottom tool window

pub fn bottom_panel(state: State, pal: Palette) -> impl IntoElement {
    view()
        .h(px(state.bottom_height.get()))
        .flex_none()
        .w_full()
        .flex_col()
        .overflow_hidden()
        .rounded(px(10.0))
        .bg(pal.bg_island)
        .child(bottom_tabs(state, pal))
        .child(view().flex_1().min_h(px(0.0)).w_full().child(dynamic(move || match state.bottom_tool.get() {
            Some(BottomTool::Terminal) | None => terminal_body(state, pal).into_element(),
            Some(BottomTool::Problems) => problems_body(state, pal).into_element(),
            Some(BottomTool::Run) => run_body(state, pal).into_element(),
            Some(BottomTool::Services) => services_body(state, pal).into_element(),
        })))
}

fn bottom_tabs(state: State, pal: Palette) -> impl IntoElement {
    view().h(px(36.0)).flex_none().w_full().child(dynamic(move || {
        let current = state.bottom_tool.get();
        view()
            .h(px(36.0))
            .w_full()
            .flex_row()
            .items_center()
            .gap(px(2.0))
            .px(px(6.0))
            .border_b_1()
            .border_color(pal.border)
            .child(bottom_tab(state, pal, "Terminal", BottomTool::Terminal, current))
            .child(bottom_tab(state, pal, "Problems", BottomTool::Problems, current))
            .child(bottom_tab(state, pal, "Run", BottomTool::Run, current))
            .child(bottom_tab(state, pal, "Services", BottomTool::Services, current))
            .child(view().flex_1())
            .child(header_action(pal, icons::CLOSE, move || {
                state.show_bottom.set(false);
            }))
    }))
}

fn bottom_tab(state: State, pal: Palette, label: &'static str, tool: BottomTool, current: Option<BottomTool>) -> View {
    let active = current == Some(tool);
    view()
        .h(px(26.0))
        .flex_none()
        .items_center()
        .justify_center()
        .px(px(10.0))
        .rounded(px(5.0))
        .bg(if active { pal.tab_selected } else { transparent_black().into() })
        .hover(move |s| if active { s.bg(pal.tab_selected) } else { s.bg(pal.tab_hover) })
        .on_press(move || {
            state.bottom_tool.set(Some(tool));
            state.show_bottom.set(true);
        })
        .child(text_ui(label, 12.0, if active { pal.fg_strong } else { pal.fg_secondary }))
}

fn terminal_body(state: State, pal: Palette) -> impl IntoElement {
    view()
        .flex_1()
        .flex_col()
        .min_h(px(0.0))
        .w_full()
        .child(
            scroll().flex_1().min_h(px(0.0)).w_full().child(
                view().flex_col().w_full().p(px(8.0)).gap(px(1.0)).children(
                    state
                        .terminal_lines
                        .get()
                        .into_iter()
                        .map(|line| text_ui(line, 12.0, pal.fg).font_family(MONO_FONT.family)),
                ),
            ),
        )
        .child(
            view()
                .h(px(30.0))
                .flex_none()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .px(px(8.0))
                .border_t_1()
                .border_color(pal.border)
                .child(text_ui("$", 12.0, pal.success).font_family(MONO_FONT.family))
                .child(
                    input()
                        .flex_1()
                        .h(px(24.0))
                        .bg(pal.bg_island)
                        .text_color(pal.fg)
                        .text_size(px(12.0))
                        .font_family(MONO_FONT.family)
                        .placeholder("Run a shell command")
                        .value(move || state.terminal_input.get())
                        .on_change(move |value: &str| state.terminal_input.set(value.to_owned()))
                        .on_submit(move |_: &str| state.terminal_submit()),
                ),
        )
}

fn problems_body(state: State, pal: Palette) -> impl IntoElement {
    view()
        .flex_1()
        .w_full()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .child(icon(icons::PROBLEMS, 20.0, pal.success))
        .child(text_ui("No problems in the workspace", 13.0, pal.fg_muted))
        .child(
            view()
                .h(px(26.0))
                .px(px(10.0))
                .items_center()
                .rounded(px(6.0))
                .hover(move |s| s.bg(pal.toolbar_hover))
                .on_press(move || state.status.set("Re-analyzed the project".to_owned()))
                .child(text_ui("Re-run inspection", 12.0, pal.fg_secondary)),
        )
}

fn run_body(state: State, pal: Palette) -> impl IntoElement {
    view()
        .flex_col()
        .p(px(10.0))
        .gap(px(4.0))
        .child(text_muted("Run configurations", 11.0, pal))
        .child(run_row(state, pal, "acme (Debug)", "CMake application"))
        .child(run_row(state, pal, "acme_tests (Debug)", "CMake test"))
}

fn run_row(state: State, pal: Palette, name: &'static str, detail: &'static str) -> View {
    view()
        .h(px(30.0))
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .px(px(8.0))
        .rounded(px(5.0))
        .hover(move |s| s.bg(pal.tree_hover))
        .on_press(move || state.status.set(format!("Run {name}")))
        .child(icon(icons::RUN, 16.0, pal.success))
        .child(text_ui(name, 13.0, pal.fg))
        .child(text_muted(detail, 12.0, pal))
}

fn services_body(state: State, pal: Palette) -> impl IntoElement {
    view()
        .flex_1()
        .w_full()
        .items_center()
        .justify_center()
        .child(text_muted("No services are running", 13.0, pal))
        .child(
            view()
                .h(px(26.0))
                .px(px(10.0))
                .items_center()
                .rounded(px(6.0))
                .hover(move |s| s.bg(pal.toolbar_hover))
                .on_press(move || state.status.set("Add a service".to_owned()))
                .child(text_ui("Add service", 12.0, pal.fg_secondary)),
        )
}
