//! The right tool window (CMake, Notifications, AI, Database) and the
//! bottom tool window (Terminal, Problems, Run, Services).

use nsh_core::element::FontWeight;
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
            state.refresh_cmake();
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
    scroll().flex_1().w_full().min_h(px(0.0)).child(dynamic(move || {
        let model = state.cmake.get();
        let mut rows: Vec<View> = Vec::new();
        match &model {
            None => {
                rows.push(panel_row(pal, 0, false, icons::CMAKE, "Loading CMake model…".to_owned(), None, false));
            }
            Some(model) if !model.has_project => {
                rows.push(panel_row(
                    pal,
                    0,
                    false,
                    icons::CMAKE,
                    "No CMakeLists.txt".to_owned(),
                    Some("open a CMake project".to_owned()),
                    false,
                ));
            }
            Some(model) => {
                let root = state.root_name();
                rows.push(panel_row(pal, 0, true, icons::CMAKE, root, None, true));
                if model.builds.is_empty() {
                    rows.push(panel_row(
                        pal,
                        1,
                        false,
                        icons::FOLDER,
                        "No configured build directory".to_owned(),
                        Some("run CMake configure".to_owned()),
                        false,
                    ));
                }
                for build in &model.builds {
                    rows.push(panel_row(
                        pal,
                        1,
                        true,
                        icons::CMAKE,
                        build.configuration(),
                        Some(build.dir.clone()),
                        false,
                    ));
                    if !build.targets.is_empty() {
                        rows.push(panel_row(pal, 2, true, icons::FOLDER, "Targets".to_owned(), None, false));
                    }
                    for target in &build.targets {
                        let kind = match target.kind {
                            ide_cmake::CmakeTargetKind::Executable => "executable",
                            ide_cmake::CmakeTargetKind::StaticLibrary => "static library",
                            ide_cmake::CmakeTargetKind::SharedLibrary => "shared library",
                            ide_cmake::CmakeTargetKind::ModuleLibrary => "module",
                            ide_cmake::CmakeTargetKind::ObjectLibrary => "object library",
                            ide_cmake::CmakeTargetKind::InterfaceLibrary => "interface",
                            ide_cmake::CmakeTargetKind::Utility => "utility",
                            ide_cmake::CmakeTargetKind::Unknown => "target",
                        };
                        rows.push(panel_row(
                            pal,
                            3,
                            false,
                            icons::CMAKE,
                            target.name.clone(),
                            Some(kind.to_owned()),
                            false,
                        ));
                    }
                    if let Some(db) = &build.compile_commands {
                        rows.push(panel_row(
                            pal,
                            1,
                            false,
                            crate::icons::file_icon(db),
                            "compile_commands.json".to_owned(),
                            None,
                            false,
                        ));
                    }
                }
                for preset in &model.presets {
                    rows.push(panel_row(
                        pal,
                        1,
                        false,
                        icons::CMAKE,
                        preset.name.clone(),
                        Some(preset.kind.clone()),
                        false,
                    ));
                }
            }
        }
        view().flex_col().w_full().py(px(4.0)).children(rows)
    }))
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
            Some(BottomTool::Debug) => debug_body(state, pal).into_element(),
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
            .child(dynamic(move || {
                (state.debug_active.get() || !state.debug_frames.get().is_empty())
                    .then(|| bottom_tab(state, pal, "Debug", BottomTool::Debug, current).into_element())
            }))
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

fn debug_body(state: State, pal: Palette) -> impl IntoElement {
    view()
        .flex_1()
        .flex_col()
        .min_h(px(0.0))
        .w_full()
        .child(
            view()
                .h(px(30.0))
                .flex_none()
                .flex_row()
                .items_center()
                .gap(px(2.0))
                .px(px(6.0))
                .border_b_1()
                .border_color(pal.border)
                .child(tool_button(pal, icons::RUN, "Continue", move || crate::debug::resume(state)))
                .child(tool_button(pal, icons::CHEVRON_DOWN, "Step Over", move || {
                    crate::debug::step(state, ide_dap::StepKind::Over)
                }))
                .child(tool_button(pal, icons::CHEVRON_DOWN, "Step In", move || {
                    crate::debug::step(state, ide_dap::StepKind::Into)
                }))
                .child(tool_button(pal, icons::CHEVRON_RIGHT, "Step Out", move || {
                    crate::debug::step(state, ide_dap::StepKind::Out)
                }))
                .child(tool_button(pal, icons::CLOSE, "Stop", move || crate::debug::stop(state)))
                .child(text_ui(move || state.debug_status.get(), 11.0, pal.fg_muted).ml(px(8.0))),
        )
        .child(
            view()
                .flex_1()
                .flex_row()
                .min_h(px(0.0))
                .w_full()
                .child(scroll().w(px(420.0)).flex_none().child(dynamic(move || {
                    let frames = state.debug_frames.get();
                    if frames.is_empty() {
                        return view().p(px(10.0)).child(text_muted("Not debugging", 12.0, pal)).into_element();
                    }
                    view()
                        .flex_col()
                        .w_full()
                        .py(px(4.0))
                        .children(frames.into_iter().enumerate().map(move |(index, frame)| {
                            let label = format!(
                                "{}  {}:{}",
                                frame.name,
                                frame
                                    .path
                                    .as_ref()
                                    .and_then(|p| p.file_name())
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_default(),
                                frame.line + 1
                            );
                            view()
                                .h(px(22.0))
                                .flex_none()
                                .mx(px(4.0))
                                .flex_row()
                                .items_center()
                                .gap(px(4.0))
                                .px(px(6.0))
                                .rounded(px(4.0))
                                .bg(move || {
                                    if state.debug_selected.get() == index {
                                        pal.tree_selected
                                    } else {
                                        transparent_black().into()
                                    }
                                })
                                .hover(move |s| s.bg(pal.tree_hover))
                                .on_press(move || crate::debug::select_frame(state, index))
                                .child(icon(icons::RUN, 12.0, pal.fg_muted))
                                .child(text_ui(label, 11.0, pal.fg).truncate())
                        }))
                        .into_element()
                })))
                .child(view().w(px(1.0)).flex_none().h_full().bg(pal.border))
                .child(scroll().flex_1().child(dynamic(move || {
                    let vars = state.debug_vars.get();
                    if vars.is_empty() {
                        return view().p(px(10.0)).child(text_muted("No variables", 12.0, pal)).into_element();
                    }
                    view()
                        .flex_col()
                        .w_full()
                        .py(px(4.0))
                        .children(vars.into_iter().map(move |var| {
                            view()
                                .h(px(20.0))
                                .flex_row()
                                .items_center()
                                .gap(px(6.0))
                                .px(px(8.0))
                                .child(text_ui(var.name, 11.0, pal.fg))
                                .child(text_ui("=", 11.0, pal.fg_faint))
                                .child(text_ui(var.value, 11.0, pal.link).truncate())
                                .child(view().flex_1())
                                .child(text_muted(var.scope, 10.0, pal))
                        }))
                        .into_element()
                }))),
        )
}

fn tool_button(pal: Palette, markup: &'static str, label: &'static str, on_press: impl Fn() + 'static) -> View {
    view()
        .h(px(22.0))
        .px(px(6.0))
        .flex_row()
        .items_center()
        .gap(px(3.0))
        .rounded(px(4.0))
        .hover(move |s| s.bg(pal.toolbar_hover))
        .on_press(on_press)
        .child(icon(markup, 12.0, pal.fg_secondary))
        .child(text_ui(label, 11.0, pal.fg_secondary))
}

fn terminal_body(state: State, pal: Palette) -> impl IntoElement {
    effect(move || {
        crate::terminal::ensure_started(state);
    });
    effect(move || {
        state.terminal_rev.get();
        crate::terminal::flush(state);
        // The component's ref is registered after its first commit.
        set_timeout(std::time::Duration::from_millis(150), move || {
            crate::terminal::flush(state);
        });
    });
    view().flex_1().w_full().min_h(px(0.0)).p(px(4.0)).bg(pal.bg_island).child(
        nsh_base::terminal()
            .component_ref(&state.terminal_ref)
            .flex_1()
            .w_full()
            .font_family(MONO_FONT.family)
            .font_size(px(12.0))
            .line_height(px(18.0))
            .on_data(move |data: &SharedString| crate::terminal::send(state, data))
            .on_resize(move |event: &nsh_base::TerminalEvent| {
                if let nsh_base::TerminalEvent::Resize(cols, rows) = event {
                    crate::terminal::resize(state, *cols, *rows);
                }
            }),
    )
}

fn problems_body(state: State, pal: Palette) -> impl IntoElement {
    view().flex_1().w_full().child(dynamic(move || {
        state.diagnostics_rev.get();
        let entries = state.diagnostics_cache.with(|cache| {
            let mut rows: Vec<(std::path::PathBuf, String, u32, ide_core::Diagnostic)> = Vec::new();
            for (path, items) in cache {
                for item in items {
                    rows.push((path.clone(), item.message.clone(), item.range.start.line, item.clone()));
                }
            }
            rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.2.cmp(&b.2)));
            rows
        });
        if entries.is_empty() {
            return view()
                .flex_1()
                .w_full()
                .items_center()
                .justify_center()
                .gap(px(8.0))
                .child(icon(icons::PROBLEMS, 20.0, pal.success))
                .child(text_ui("No problems in the workspace", 13.0, pal.fg_muted))
                .into_element();
        }
        let count = entries.len();
        view()
            .flex_col()
            .w_full()
            .py(px(4.0))
            .child(view().px(px(10.0)).pb(px(4.0)).child(text_muted(format!("{count} problems"), 11.0, pal)))
            .children(entries.into_iter().map(move |(path, message, line, item)| {
                let color = match item.severity {
                    ide_core::Severity::Error => pal.danger,
                    ide_core::Severity::Warning => pal.warning,
                    ide_core::Severity::Info => pal.link,
                    ide_core::Severity::Hint => pal.fg_muted,
                };
                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let target = path.clone();
                view()
                    .h(px(24.0))
                    .flex_none()
                    .mx(px(4.0))
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(6.0))
                    .rounded(px(5.0))
                    .hover(move |s| s.bg(pal.tree_hover))
                    .on_press(move || state.open_path(target.clone()))
                    .child(icon(icons::PROBLEMS, 14.0, color))
                    .child(text_ui(message, 12.0, pal.fg).truncate())
                    .child(view().flex_1())
                    .child(text_muted(format!("{name}:{}", line + 1), 11.0, pal))
            }))
            .into_element()
    }))
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
