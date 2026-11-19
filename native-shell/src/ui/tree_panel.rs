//! The left tool window: Project, Commit, Structure, Bookmarks and Find in
//! Files, with IDEA's 40-point header and 24-point tree rows.

use nsh_core::element::input;
use nsh_core::prelude::*;

use crate::icons;
use crate::model::{LeftTool, SearchHit, State};
use crate::theme::Palette;
use crate::ui::{icon, text_muted, text_strong, text_ui};

pub fn left_tool(state: State, pal: Palette) -> impl IntoElement {
    view().flex_1().flex_col().min_h(px(0.0)).w_full().child(dynamic(move || match state.left_tool.get() {
        LeftTool::Project => project_panel(state, pal).into_element(),
        LeftTool::Commit => commit_panel(state, pal).into_element(),
        LeftTool::Structure => structure_panel(state, pal).into_element(),
        LeftTool::Bookmarks => bookmarks_panel(state, pal).into_element(),
        LeftTool::Search => search_panel(state, pal).into_element(),
    }))
}

fn header(title: &'static str, pal: Palette, actions: impl IntoElement) -> impl IntoElement {
    view()
        .h(px(40.0))
        .flex_none()
        .w_full()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .pl(px(12.0))
        .pr(px(6.0))
        .child(text_strong(title, 13.0, pal).flex_shrink_0())
        .child(icon(icons::CHEVRON_DOWN, 12.0, pal.fg_muted))
        .child(view().flex_1())
        .child(actions)
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

fn project_panel(state: State, pal: Palette) -> impl IntoElement {
    view()
        .flex_1()
        .flex_col()
        .min_h(px(0.0))
        .child(header(
            "Project",
            pal,
            view()
                .flex_row()
                .items_center()
                .gap(px(2.0))
                .child(
                    header_action(pal, icons::ADD, move || {
                        state.open_new_file();
                    })
                    .test_id("project-new-file"),
                )
                .child(
                    header_action(pal, icons::FOLDER, move || {
                        state.open_new_folder();
                    })
                    .test_id("project-new-folder"),
                )
                .child(header_action(pal, icons::COLLAPSE_ALL, move || {
                    state.expanded.set(Vec::new());
                }))
                .child(header_action(pal, icons::MORE_HORIZONTAL, move || {
                    state.status.set("Project view options".to_owned());
                })),
        ))
        .child(scroll().flex_1().w_full().min_h(px(0.0)).child(
            view().flex_col().w_full().pt(px(0.0)).pb(px(8.0)).child(root_row(state, pal)).child(dynamic(move || {
                state.tree_rev.get();
                let rows = state.rows();
                view().flex_col().w_full().children(rows.into_iter().map(|row| tree_row(state, pal, row)))
            })),
        ))
}

fn root_row(state: State, pal: Palette) -> impl IntoElement {
    let name = state.root_name();
    let detail = {
        let root = state.root.get();
        if root.to_string_lossy().contains("native-shell/demo") {
            format!("/dev/{name}")
        } else {
            root.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "/".to_owned())
        }
    };
    view()
        .h(px(24.0))
        .flex_none()
        .mx(px(4.0))
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .pl(px(8.0))
        .pr(px(6.0))
        .rounded(px(5.0))
        .hover(move |s| s.bg(pal.tree_hover))
        .on_press(move || {
            let root = state.root.get().to_string_lossy().to_string();
            state.selected.set(Some(root.clone()));
            let expanded = state.expanded.get();
            if expanded.is_empty() {
                state.expanded.set(vec![root]);
            } else {
                // The root row doubles as collapse-all for the whole project.
                state.expanded.set(Vec::new());
            }
        })
        .on_context_menu(move |event: &ContextMenuEvent| {
            let root = state.root.get();
            state.tree_menu.set(Some(crate::model::TreeMenu {
                path: root,
                dir: true,
                x: event.page_position.x.0,
                y: event.page_position.y.0,
            }));
        })
        .child(dynamic(move || {
            let expanded = !state.expanded.get().is_empty();
            icon(if expanded { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT }, 12.0, pal.fg_muted).into_element()
        }))
        .child(icon(icons::PROJECT, 16.0, pal.fg_muted))
        .child(text_strong(name, 13.0, pal))
        .child(text_ui(detail, 12.0, pal.fg_subtle).flex_1().truncate())
}

fn tree_row(state: State, pal: Palette, row: crate::model::Row) -> impl IntoElement {
    let path = row.path.clone();
    let path_for_menu = row.path.clone();
    let path_for_bg = row.path.clone();
    let row_path_for_chevron = row.path.clone();
    let path_for_vcs = row.path.clone();
    let name = row.name.clone();
    let is_dir = row.dir;
    let depth = row.depth;
    let is_build = name == "build";
    let icon_markup = if is_dir {
        match name.as_str() {
            "src" | "include" => icons::FOLDER_SOURCE,
            "tests" => icons::FOLDER_TEST,
            _ => icons::FOLDER,
        }
    } else {
        crate::icons::file_icon(&name)
    };
    let icon_color = if is_build { pal.warning } else { pal.fg_muted };
    let icon_size = if is_dir { 16.0 } else { 16.0 };
    view()
        .h(px(24.0))
        .flex_none()
        .mx(px(4.0))
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .pl(px(8.0 + 12.0 * depth as f32))
        .pr(px(6.0))
        .rounded(px(5.0))
        .bg(move || {
            let selected = state.selected.get().is_some_and(|selected| selected == path_for_bg);
            if selected {
                pal.tree_selected
            } else if is_build {
                pal.build_tint
            } else {
                transparent_black().into()
            }
        })
        .hover(move |s| s.bg(pal.tree_hover))
        .on_press(move || {
            if is_dir {
                state.selected.set(Some(path.clone()));
            }
            state.open_path(std::path::PathBuf::from(&path));
        })
        .on_context_menu(move |event: &ContextMenuEvent| {
            state.tree_menu.set(Some(crate::model::TreeMenu {
                path: std::path::PathBuf::from(&path_for_menu),
                dir: is_dir,
                x: event.page_position.x.0,
                y: event.page_position.y.0,
            }));
        })
        .child(view().w(px(12.0)).flex_none().items_center().justify_center().child(dynamic(move || {
            if !is_dir {
                return view().into_element();
            }
            let expanded = state.is_expanded(&row_path_for_chevron);
            icon(if expanded { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT }, 12.0, pal.fg_secondary)
                .into_element()
        })))
        .child(icon(icon_markup, icon_size, icon_color))
        .child(text_ui(name, 13.0, pal.fg).truncate().text_color(move || {
            let Some(row) = state.vcs.get().into_iter().find(|row| row.path.to_string_lossy() == path_for_vcs) else {
                return pal.fg;
            };
            if row.conflicted {
                pal.danger
            } else {
                match row.letter {
                    "A" => pal.vcs_added,
                    "U" => pal.vcs_untracked,
                    "D" => pal.vcs_deleted,
                    "C" => pal.danger,
                    _ => pal.vcs_modified,
                }
            }
        }))
}

fn commit_panel(state: State, pal: Palette) -> impl IntoElement {
    view()
        .flex_1()
        .flex_col()
        .min_h(px(0.0))
        .child(header(
            "Commit",
            pal,
            view()
                .flex_row()
                .items_center()
                .gap(px(2.0))
                .child(dynamic(move || {
                    let branch = state.vcs_branch.get();
                    let label = branch.map(|b| format!("on {b}")).unwrap_or_else(|| "no repository".to_owned());
                    text_muted(label, 11.0, pal).into_element()
                }))
                .child(header_action(pal, icons::COMMIT, move || {
                    state.commit_all();
                }))
                .child(header_action(pal, icons::GIT_PULL, move || {
                    state.pull_repo();
                }))
                .child(header_action(pal, icons::REFRESH, move || {
                    state.refresh_vcs();
                }))
                .child(header_action(pal, icons::MORE_HORIZONTAL, move || {})),
        ))
        .child(
            view()
                .h(px(36.0))
                .flex_none()
                .w_full()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .px(px(8.0))
                .border_b_1()
                .border_color(pal.border)
                .child(
                    input()
                        .flex_1()
                        .h(px(24.0))
                        .bg(pal.bg_island)
                        .text_color(pal.fg)
                        .text_size(px(12.0))
                        .placeholder("Commit message")
                        .value(move || state.commit_message.get())
                        .on_change(move |value: &str| state.commit_message.set(value.to_owned()))
                        .on_submit(move |_: &str| state.commit_all()),
                )
                .child(
                    view()
                        .h(px(24.0))
                        .px(px(10.0))
                        .items_center()
                        .justify_center()
                        .rounded(px(5.0))
                        .bg(move || {
                            if state.commit_message.get().trim().is_empty() {
                                pal.accent.opacity(0.55)
                            } else {
                                pal.accent
                            }
                        })
                        .on_press(move || state.commit_all())
                        .child(text_ui("Commit", 12.0, white().into())),
                ),
        )
        .child(scroll().flex_1().child(dynamic(move || {
            let rows = state.vcs.get();
            let loading = state.vcs_loading.get();
            if loading && rows.is_empty() {
                return view().p(px(10.0)).child(text_muted("Reading git status…", 12.0, pal)).into_element();
            }
            if rows.is_empty() {
                return view()
                    .flex_col()
                    .p(px(8.0))
                    .gap(px(6.0))
                    .child(
                        view()
                            .h(px(52.0))
                            .w_full()
                            .items_center()
                            .justify_center()
                            .gap(px(6.0))
                            .child(icon(icons::COMMIT, 20.0, pal.fg_muted))
                            .child(text_ui("No changes", 13.0, pal.fg_muted)),
                    )
                    .child(text_muted("The working tree is clean. Changed files appear here.", 12.0, pal))
                    .into_element();
            }
            view()
                .flex_col()
                .w_full()
                .py(px(4.0))
                .children(rows.into_iter().map(move |row| vcs_row(state, pal, row)))
                .into_element()
        })))
}

fn vcs_row(state: State, pal: Palette, row: crate::backend::VcsRow) -> View {
    let path = row.path.clone();
    let letter_color = if row.conflicted {
        pal.danger
    } else if row.staged {
        pal.success
    } else {
        pal.link
    };
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
        .on_press(move || state.open_path(path.clone()))
        .child(icon(crate::icons::file_icon(&row.rel), 16.0, pal.fg_muted))
        .child(text_ui(row.rel, 13.0, pal.fg).truncate())
        .child(view().flex_1())
        .child(text_ui(row.letter, 11.0, letter_color))
}

fn structure_panel(state: State, pal: Palette) -> impl IntoElement {
    view()
        .flex_1()
        .flex_col()
        .min_h(px(0.0))
        .child(header(
            "Structure",
            pal,
            view().flex_row().items_center().gap(px(2.0)).child(header_action(pal, icons::REFRESH, move || {})),
        ))
        .child(scroll().flex_1().child(dynamic(move || {
            let symbols = outline(state);
            view().flex_col().w_full().py(px(4.0)).children(symbols.into_iter().map(move |(line, text)| {
                view()
                    .h(px(24.0))
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(10.0))
                    .rounded(px(5.0))
                    .hover(move |s| s.bg(pal.tree_hover))
                    .child(icon(crate::icons::FILE_CPP, 16.0, pal.fg_muted))
                    .child(text_ui(text, 13.0, pal.fg).truncate())
                    .child(view().flex_1())
                    .child(text_muted(format!("{line}"), 11.0, pal))
            }))
        })))
}

fn bookmarks_panel(state: State, pal: Palette) -> impl IntoElement {
    view()
        .flex_1()
        .flex_col()
        .min_h(px(0.0))
        .child(header(
            "Bookmarks",
            pal,
            view().flex_row().items_center().gap(px(2.0)).child(header_action(pal, icons::ADD, move || {
                state.status.set("Add bookmark".to_owned());
            })),
        ))
        .child(view().flex_1().items_center().justify_center().child(text_muted("No bookmarks", 13.0, pal)))
}

fn search_panel(state: State, pal: Palette) -> impl IntoElement {
    view()
        .flex_1()
        .flex_col()
        .min_h(px(0.0))
        .child(header(
            "Find in Files",
            pal,
            view().flex_row().items_center().gap(px(2.0)).child(header_action(pal, icons::COLLAPSE_ALL, move || {
                state.search_results.set(Vec::new());
            })),
        ))
        .child(
            view()
                .flex_col()
                .w_full()
                .gap(px(6.0))
                .px(px(8.0))
                .pb(px(8.0))
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(px(4.0))
                        .child(
                            input()
                                .flex_1()
                                .h(px(28.0))
                                .px(px(7.0))
                                .rounded(px(5.0))
                                .border(px(1.0))
                                .border_color(pal.accent)
                                .bg(pal.bg_island)
                                .text_color(pal.fg)
                                .text_size(px(13.0))
                                .placeholder("Search in files")
                                .value(move || state.search_query.get())
                                .on_change(move |value: &str| {
                                    state.search_query.set(value.to_owned());
                                })
                                .on_submit(move |_: &str| state.search_backend()),
                        )
                        .child(
                            view()
                                .size(px(28.0))
                                .items_center()
                                .justify_center()
                                .rounded(px(5.0))
                                .bg(pal.accent)
                                .on_press(move || state.search_backend())
                                .child(icon(icons::SEARCH, 16.0, white().into())),
                        ),
                )
                .child(dynamic(move || search_results(state, pal))),
        )
}

fn search_results(state: State, pal: Palette) -> impl IntoElement {
    let hits = state.search_results.get();
    if hits.is_empty() {
        let label = if state.searching.get() { "Searching…" } else { "No results" };
        return view().flex_1().items_center().justify_center().child(text_muted(label, 12.0, pal)).into_element();
    }
    let count = hits.len();
    let summary =
        if state.searching.get() { format!("{count} matches · searching…") } else { format!("{count} matches") };
    view()
        .flex_col()
        .w_full()
        .gap(px(1.0))
        .child(text_muted(summary, 11.0, pal))
        .children(hits.into_iter().map(move |hit| search_hit(state, pal, hit)))
        .into_element()
}

fn search_hit(state: State, pal: Palette, hit: SearchHit) -> impl IntoElement {
    let path = hit.path.clone();
    view()
        .flex_col()
        .w_full()
        .gap(px(1.0))
        .px(px(6.0))
        .py(px(4.0))
        .rounded(px(5.0))
        .hover(move |s| s.bg(pal.tree_hover))
        .on_press(move || {
            state.open_path(path.clone());
        })
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .child(icon(crate::icons::file_icon(&hit.display), 14.0, pal.fg_muted))
                .child(text_ui(hit.display.clone(), 12.0, pal.fg_secondary).truncate())
                .child(text_muted(format!(":{}", hit.line), 11.0, pal)),
        )
        .child(text_ui(hit.text, 12.0, pal.fg_muted).truncate())
}

/// A small C/C++/Rust outline for the Structure tool window: the lines that
/// declare types and functions.
fn outline(state: State) -> Vec<(usize, String)> {
    let Some(tab) = state.active_tab() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (index, line) in tab.text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("class ")
            || trimmed.starts_with("struct ")
            || trimmed.starts_with("namespace ")
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("pub fn ")
            || trimmed.contains(" main(")
            || (trimmed.contains('(')
                && trimmed.ends_with('{')
                && !trimmed.starts_with("if ")
                && !trimmed.starts_with("for ")
                && !trimmed.starts_with("while ")
                && !trimmed.starts_with("switch "))
        {
            out.push((index + 1, trimmed.trim_end_matches('{').trim().to_owned()));
        }
        if out.len() >= 40 {
            break;
        }
    }
    out
}
