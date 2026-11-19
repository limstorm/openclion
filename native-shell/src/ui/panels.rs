//! The right tool window (Notifications) and the
//! bottom tool window (Terminal, Problems, Run, Git, CMake).

use nsh_core::element::input;
use nsh_core::prelude::*;

use crate::icons;
use crate::model::{BottomTool, RightTool, State};
use crate::theme::Palette;
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
            RightTool::Notifications => notifications_body(state, pal).into_element(),
        })))
}

fn right_header(state: State, pal: Palette) -> impl IntoElement {
    let title = match state.right_tool.get() {
        RightTool::Notifications => "Notifications",
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

fn cmake_tree(state: State, pal: Palette) -> impl IntoElement {
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
                        let target_name = target.name.clone();
                        let target_for_bg = target_name.clone();
                        let build_for_bg = build.dir.clone();
                        let target_for_press = target_name.clone();
                        let build_for_press = build.dir.clone();
                        let executable = matches!(target.kind, ide_cmake::CmakeTargetKind::Executable);
                        let row =
                            panel_row(pal, 3, false, icons::CMAKE, target_name.clone(), Some(kind.to_owned()), false)
                                .test_id(&format!("cmake-target-{target_name}"))
                                .bg(move || {
                                    let selected = state.cmake_selection.get().is_some_and(|selection| {
                                        selection.target == target_for_bg && selection.build_dir == build_for_bg
                                    });
                                    if selected { pal.tree_selected } else { transparent_black().into() }
                                })
                                .on_press(move || {
                                    state.select_cmake_target(
                                        build_for_press.clone(),
                                        target_for_press.clone(),
                                        executable,
                                    )
                                });
                        rows.push(row);
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
        .child(dynamic(move || bottom_content(state, pal).into_element()))
}

fn bottom_content(state: State, pal: Palette) -> impl IntoElement {
    match state.bottom_tool.get() {
        Some(BottomTool::Git) => view()
            .flex_col()
            .flex_1()
            .w_full()
            .min_h(px(0.0))
            .child(git_tabs(state, pal))
            .child(git_bottom(state, pal))
            .into_element(),
        Some(BottomTool::Cmake) => view()
            .flex_col()
            .flex_1()
            .w_full()
            .min_h(px(0.0))
            .child(cmake_bottom_tabs(state, pal))
            .child(cmake_tree(state, pal))
            .into_element(),
        Some(BottomTool::Problems) => view()
            .flex_col()
            .flex_1()
            .w_full()
            .min_h(px(0.0))
            .child(section_header(state, pal, "Problems"))
            .child(problems_body(state, pal))
            .into_element(),
        Some(BottomTool::Run) => view()
            .flex_col()
            .flex_1()
            .w_full()
            .min_h(px(0.0))
            .child(section_header(state, pal, "Run"))
            .child(run_body(state, pal))
            .into_element(),
        Some(BottomTool::Usages) => view()
            .flex_col()
            .flex_1()
            .w_full()
            .min_h(px(0.0))
            .child(section_header(state, pal, "Usages"))
            .child(usages_body(state, pal))
            .into_element(),
        Some(BottomTool::Hierarchy) => view()
            .flex_col()
            .flex_1()
            .w_full()
            .min_h(px(0.0))
            .child(section_header(state, pal, "Hierarchy"))
            .child(hierarchy_body(state, pal))
            .into_element(),
        Some(BottomTool::Debug) => debug_body(state, pal).into_element(),
        Some(BottomTool::Terminal) | None => terminal_body(state, pal).into_element(),
    }
}

fn section_header(state: State, pal: Palette, title: &'static str) -> impl IntoElement {
    view()
        .h(px(30.0))
        .flex_none()
        .w_full()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .px(px(8.0))
        .border_b_1()
        .border_color(pal.border)
        .child(text_strong(title, 12.0, pal))
        .child(view().flex_1())
        .child(header_action(pal, icons::CLOSE, move || {
            state.show_bottom.set(false);
        }))
}

fn git_tabs(state: State, pal: Palette) -> impl IntoElement {
    let tab = state.git_tab.get();
    let label = |text: &'static str, index: u8| {
        let active = tab == index;
        view()
            .h(px(26.0))
            .px(px(10.0))
            .items_center()
            .justify_center()
            .rounded(px(5.0))
            .bg(if active { pal.tab_selected } else { transparent_black().into() })
            .hover(move |s| s.bg(pal.tab_hover))
            .on_press(move || state.git_tab.set(index))
            .child(text_ui(text, 12.0, if active { pal.fg_strong } else { pal.fg_secondary }))
    };
    view()
        .h(px(36.0))
        .flex_none()
        .w_full()
        .flex_row()
        .items_center()
        .gap(px(2.0))
        .px(px(6.0))
        .border_b_1()
        .border_color(pal.border)
        .child(text_strong("Git", 12.0, pal).mr(px(6.0)))
        .child(label("Log", 0))
        .child(label("Console", 1))
        .child(label("Worktrees", 2))
        .child(view().flex_1())
        .child(debug_text_button(pal, "Refresh", move || state.refresh_git_log()))
        .child(header_action(pal, icons::CLOSE, move || {
            state.show_bottom.set(false);
        }))
}

fn cmake_bottom_tabs(state: State, pal: Palette) -> impl IntoElement {
    view()
        .h(px(36.0))
        .flex_none()
        .w_full()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .px(px(6.0))
        .border_b_1()
        .border_color(pal.border)
        .child(text_strong("CMake", 12.0, pal))
        .child(view().flex_1())
        .child(debug_text_button(pal, "Reload", move || state.configure_cmake()))
        .child(debug_text_button(pal, "Build", move || state.build_active_configuration()))
        .child(debug_text_button(pal, "Run", move || state.run_active_configuration()))
        .child(header_action(pal, icons::CLOSE, move || {
            state.show_bottom.set(false);
        }))
}

fn git_bottom(state: State, pal: Palette) -> impl IntoElement {
    effect(move || {
        if state.bottom_tool.get() == Some(BottomTool::Git) {
            state.refresh_git_log();
        }
    });
    let tab = state.git_tab.get();
    if tab == 1 {
        return git_console(state, pal).into_element();
    }
    if tab == 2 {
        return view()
            .flex_1()
            .items_center()
            .justify_center()
            .child(text_muted("No worktrees in this repository", 12.0, pal))
            .into_element();
    }
    view()
        .flex_col()
        .flex_1()
        .w_full()
        .min_h(px(0.0))
        .child(
            view()
                .h(px(24.0))
                .flex_none()
                .w_full()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .px(px(10.0))
                .border_b_1()
                .border_color(pal.border)
                .child(text_muted("Subject", 10.0, pal).flex_1())
                .child(text_muted("Author", 10.0, pal).w(px(110.0)))
                .child(text_muted("Date", 10.0, pal).w(px(86.0)))
                .child(text_muted("Hash", 10.0, pal).w(px(70.0))),
        )
        .child(scroll().flex_1().child(dynamic(move || {
            let rows = state.git_log.get();
            if state.git_log_loading.get() && rows.is_empty() {
                return view().p(px(10.0)).child(text_muted("Reading git log…", 12.0, pal)).into_element();
            }
            if rows.is_empty() {
                return view().p(px(10.0)).child(text_muted("No commits", 12.0, pal)).into_element();
            }
            view()
                .flex_col()
                .w_full()
                .children(rows.into_iter().take(50).map(move |row| {
                    view()
                        .h(px(24.0))
                        .flex_none()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .px(px(10.0))
                        .hover(move |s| s.bg(pal.tree_hover))
                        .child(
                            view()
                                .flex_row()
                                .items_center()
                                .gap(px(6.0))
                                .flex_1()
                                .min_w(px(0.0))
                                .overflow_hidden()
                                .child(icon(icons::COMMIT, 12.0, pal.link))
                                .child(text_ui(row.summary, 11.0, pal.fg).flex_1().min_w(px(0.0)).truncate()),
                        )
                        .child(text_muted(row.author, 10.0, pal).w(px(110.0)).truncate())
                        .child(text_muted(row.date, 10.0, pal).w(px(86.0)))
                        .child(text_ui(row.short_id, 10.0, pal.link).w(px(70.0)))
                }))
                .into_element()
        })))
        .into_element()
}

fn git_console(state: State, pal: Palette) -> impl IntoElement {
    let rows = state.vcs.get();
    view()
        .flex_col()
        .flex_1()
        .w_full()
        .min_h(px(0.0))
        .p(px(8.0))
        .gap(px(2.0))
        .child(text_muted(
            format!(
                "{}  ·  {} changed file(s)",
                state.vcs_branch.get().unwrap_or_else(|| "detached".to_owned()),
                rows.len()
            ),
            11.0,
            pal,
        ))
        .child(scroll().flex_1().children(rows.into_iter().map(move |row| {
            view()
                .h(px(20.0))
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(text_ui(row.letter, 10.0, pal.link).w(px(14.0)))
                .child(text_ui(row.rel, 11.0, pal.fg).truncate())
        })))
        .into_element()
}

fn hierarchy_body(state: State, pal: Palette) -> impl IntoElement {
    scroll().flex_1().w_full().min_h(px(0.0)).child(dynamic(move || {
        let rows = state.hierarchy.get();
        if rows.is_empty() {
            return view()
                .p(px(10.0))
                .child(text_muted("No type hierarchy at this position", 12.0, pal))
                .into_element();
        }
        view()
            .flex_col()
            .w_full()
            .py(px(4.0))
            .children(rows.into_iter().enumerate().map(move |(index, row)| {
                let name = row.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                view()
                    .h(px(24.0))
                    .flex_none()
                    .mx(px(4.0))
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(6.0))
                    .rounded(px(4.0))
                    .hover(move |s| s.bg(pal.tree_hover))
                    .on_press(move || crate::lsp::open_hierarchy(state, index))
                    .child(text_ui(row.relation, 9.0, pal.fg_faint).w(px(58.0)))
                    .child(icon(icons::STRUCTURE, 13.0, pal.fg_muted))
                    .child(text_ui(row.name, 12.0, pal.fg))
                    .child(text_muted(row.detail, 10.0, pal).truncate())
                    .child(view().flex_1())
                    .child(text_muted(format!("{name}:{}", row.line + 1), 10.0, pal))
            }))
            .into_element()
    }))
}

fn usages_body(state: State, pal: Palette) -> impl IntoElement {
    scroll().flex_1().w_full().min_h(px(0.0)).child(dynamic(move || {
        let rows = state.usages.get();
        if rows.is_empty() {
            return view()
                .flex_1()
                .items_center()
                .justify_center()
                .child(text_muted("No usages", 12.0, pal))
                .into_element();
        }
        let count = rows.len();
        view()
            .flex_col()
            .w_full()
            .py(px(4.0))
            .child(view().px(px(10.0)).pb(px(4.0)).child(text_muted(format!("{count} usages"), 11.0, pal)))
            .children(rows.into_iter().enumerate().map(move |(index, row)| {
                let name = row.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                view()
                    .h(px(24.0))
                    .flex_none()
                    .mx(px(4.0))
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(6.0))
                    .rounded(px(4.0))
                    .hover(move |s| s.bg(pal.tree_hover))
                    .on_press(move || crate::lsp::open_usage(state, index))
                    .child(icon(icons::SEARCH, 13.0, pal.fg_muted))
                    .child(text_ui(row.preview, 12.0, pal.fg).truncate())
                    .child(view().flex_1())
                    .child(text_muted(format!("{name}:{}", row.line + 1), 11.0, pal))
            }))
            .into_element()
    }))
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
                .child(debug_text_button(pal, "Condition", move || crate::debug::edit_condition_at_caret(state)))
                .child(debug_text_button(pal, "Logpoint", move || crate::debug::edit_log_at_caret(state)))
                .child(text_ui(move || state.debug_status.get(), 11.0, pal.fg_muted).ml(px(8.0))),
        )
        .child(watch_panel(state, pal))
        .child(
            view()
                .flex_1()
                .flex_row()
                .min_h(px(0.0))
                .w_full()
                .child(
                    view()
                        .w(px(420.0))
                        .flex_none()
                        .flex_col()
                        .min_h(px(0.0))
                        .child(breakpoints_panel(state, pal))
                        .child(scroll().flex_1().child(dynamic(move || {
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
                        }))),
                )
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
                        .children(vars.into_iter().enumerate().map(move |(index, var)| {
                            let depth = var.depth;
                            let expandable = var.reference > 0;
                            view()
                                .h(px(20.0))
                                .flex_row()
                                .items_center()
                                .gap(px(4.0))
                                .pl(px(8.0 + 12.0 * depth as f32))
                                .pr(px(8.0))
                                .rounded(px(3.0))
                                .hover(move |s| s.bg(pal.tree_hover))
                                .on_press(move || crate::debug::toggle_variable(state, index))
                                .child(dynamic(move || {
                                    if !expandable {
                                        return view().w(px(12.0)).into_element();
                                    }
                                    icon(
                                        if var.expanded { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT },
                                        11.0,
                                        pal.fg_muted,
                                    )
                                    .into_element()
                                }))
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

fn breakpoints_panel(state: State, pal: Palette) -> impl IntoElement {
    view()
        .h(px(96.0))
        .flex_none()
        .w_full()
        .flex_col()
        .border_b_1()
        .border_color(pal.border)
        .child(view().h(px(20.0)).flex_row().items_center().px(px(8.0)).child(text_muted("Breakpoints", 10.0, pal)))
        .child(scroll().flex_1().child(dynamic(move || {
            state.breakpoints_rev.get();
            let entries = state.breakpoints.with(|map| {
                let mut rows: Vec<(std::path::PathBuf, u32, Option<String>, Option<String>, bool)> = Vec::new();
                for (path, specs) in map {
                    for spec in specs {
                        rows.push((
                            path.clone(),
                            spec.line,
                            spec.condition.clone(),
                            spec.log_message.clone(),
                            spec.enabled,
                        ));
                    }
                }
                rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
                rows
            });
            if entries.is_empty() {
                return view().px(px(10.0)).child(text_muted("No breakpoints", 11.0, pal)).into_element();
            }
            view()
                .flex_col()
                .w_full()
                .children(entries.into_iter().take(8).map(move |(path, line, condition, log, enabled)| {
                    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    let label = if let Some(log) = log.filter(|log| !log.is_empty()) {
                        format!("{name}:{}  LOG {log}", line + 1)
                    } else if let Some(condition) = condition.filter(|condition| !condition.is_empty()) {
                        format!("{name}:{}  {condition}", line + 1)
                    } else {
                        format!("{name}:{}", line + 1)
                    };
                    let target = path.clone();
                    let path_for_toggle = path.clone();
                    view()
                        .h(px(18.0))
                        .flex_row()
                        .items_center()
                        .gap(px(4.0))
                        .px(px(8.0))
                        .rounded(px(3.0))
                        .hover(move |s| s.bg(pal.tree_hover))
                        .on_press(move || state.open_breakpoint_condition(target.clone(), line))
                        .child(
                            view()
                                .size(px(12.0))
                                .items_center()
                                .justify_center()
                                .on_press(move || {
                                    crate::debug::toggle_breakpoint_enabled(state, path_for_toggle.clone(), line)
                                })
                                .child(icon(icons::PROBLEMS, 10.0, if enabled { pal.danger } else { pal.fg_faint })),
                        )
                        .child(text_ui(label, 10.0, pal.fg).truncate().pointer_events(PointerEvents::None))
                }))
                .into_element()
        })))
}

fn watch_panel(state: State, pal: Palette) -> impl IntoElement {
    view()
        .h(px(30.0))
        .flex_none()
        .w_full()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .px(px(6.0))
        .border_b_1()
        .border_color(pal.border)
        .child(text_muted("Watch", 11.0, pal))
        .child(
            input()
                .w(px(220.0))
                .h(px(22.0))
                .bg(pal.bg_island)
                .text_color(pal.fg)
                .text_size(px(11.0))
                .placeholder("Expression")
                .value(move || state.debug_watch_input.get())
                .on_change(move |value: &str| state.debug_watch_input.set(value.to_owned()))
                .on_submit(move |_: &str| crate::debug::add_watch(state, state.debug_watch_input.get())),
        )
        .child(scroll().flex_1().child(dynamic(move || {
            let values = state.debug_watch_values.get();
            let expressions = state.debug_watch.get();
            if expressions.is_empty() {
                return view().into_element();
            }
            view()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .children(values.into_iter().take(4).enumerate().map(move |(index, value)| {
                    view()
                        .h(px(20.0))
                        .flex_row()
                        .items_center()
                        .gap(px(3.0))
                        .px(px(5.0))
                        .rounded(px(3.0))
                        .bg(pal.bg_elev)
                        .child(text_ui(format!("{} = {}", value.expression, value.value), 10.0, pal.fg_secondary))
                        .child(
                            view()
                                .size(px(14.0))
                                .items_center()
                                .justify_center()
                                .on_press(move || crate::debug::remove_watch(state, index))
                                .child(icon(icons::CLOSE, 9.0, pal.fg_muted)),
                        )
                }))
                .into_element()
        })))
}

fn debug_text_button(pal: Palette, label: &'static str, on_press: impl Fn() + 'static) -> View {
    view()
        .h(px(22.0))
        .px(px(6.0))
        .items_center()
        .rounded(px(4.0))
        .hover(move |s| s.bg(pal.toolbar_hover))
        .on_press(on_press)
        .child(text_ui(label, 11.0, pal.fg_secondary).pointer_events(PointerEvents::None))
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
    effect(move || {
        if state.show_bottom.get() && state.bottom_tool.get() == Some(BottomTool::Terminal) {
            set_timeout(std::time::Duration::from_millis(80), move || {
                crate::terminal::focus(state);
            });
        }
    });
    view().flex_1().w_full().min_h(px(0.0)).overflow_hidden().pl(px(6.0)).bg(pal.bg_island).child(
        nsh_base::terminal()
            .component_ref(&state.terminal_ref)
            .flex_1()
            .w_full()
            .cursor_class("bg-[#1f2329] -ml-[3px]")
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
    view().flex_col().w_full().p(px(10.0)).gap(px(4.0)).child(text_muted("Run configurations", 11.0, pal)).child(
        dynamic(move || {
            let model = state.cmake.get();
            let selected = state.run_target.get();
            let mut rows: Vec<View> = Vec::new();
            if let Some(model) = &model {
                for (build, target) in model.executables() {
                    let name = target.name.clone();
                    let name_for_press = name.clone();
                    let active = selected.as_deref() == Some(target.name.as_str());
                    let detail = format!("{}  ·  {}", build.configuration(), build.dir);
                    rows.push(
                        view()
                            .h(px(30.0))
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .px(px(8.0))
                            .rounded(px(5.0))
                            .bg(if active { pal.tree_selected } else { transparent_black().into() })
                            .hover(move |s| s.bg(pal.tree_hover))
                            .on_press(move || {
                                state.run_target.set(Some(name_for_press.clone()));
                                crate::settings::save(state);
                            })
                            .child(icon(icons::RUN, 16.0, pal.success))
                            .child(text_ui(name, 13.0, pal.fg))
                            .child(text_muted(detail, 12.0, pal)),
                    );
                }
            }
            if rows.is_empty() {
                return view().child(text_muted("No executable targets — configure CMake", 12.0, pal)).into_element();
            }
            view().flex_col().w_full().gap(px(2.0)).children(rows).into_element()
        }),
    )
}
