//! Floating layers: Quick Open, the project menu, Settings and the toast.

use std::path::PathBuf;

use nsh_core::element::FontWeight;
use nsh_core::element::input;
use nsh_core::file_dialog::OpenDialog;
use nsh_core::prelude::*;

use crate::icons;
use crate::model::{State, file_name};
use crate::theme::{Palette, UI_FONT};
use crate::ui::{icon, text_muted, text_strong, text_ui};

pub fn overlays(state: State, pal: Palette) -> impl IntoElement {
    view()
        .absolute()
        .inset(px(0.0))
        .pointer_events(PointerEvents::BoxNone)
        .child(dynamic(move || quick_open(state, pal).into_element()))
        .child(dynamic(move || project_menu(state, pal).into_element()))
        .child(dynamic(move || settings_dialog(state, pal).into_element()))
        .child(dynamic(move || toast(state, pal).into_element()))
}

fn quick_open(state: State, pal: Palette) -> impl IntoElement {
    if !state.quick_open.get() {
        return view().into_element();
    }
    let query = state.quick_query.get();
    let root = state.root.get();
    let files = state.files();
    let needle = query.to_lowercase();
    let matches: Vec<PathBuf> = files
        .iter()
        .filter(|path| needle.is_empty() || path.to_string_lossy().to_lowercase().contains(&needle))
        .take(14)
        .cloned()
        .collect();
    view()
        .absolute()
        .inset(px(0.0))
        .child(view().absolute().inset(px(0.0)).bg(c(0x00000029)).on_press(move || state.quick_open.set(false)))
        .child(
            view().absolute().left(px(0.0)).right(px(0.0)).top(px(92.0)).items_center().child(
                view()
                    .w(px(640.0))
                    .h(px(420.0))
                    .flex_col()
                    .overflow_hidden()
                    .rounded(px(10.0))
                    .border(px(1.0))
                    .border_color(pal.border_strong)
                    .bg(pal.bg_island)
                    .child(
                        view()
                            .h(px(44.0))
                            .flex_none()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .px(px(12.0))
                            .border_b_1()
                            .border_color(pal.border)
                            .child(icon(icons::SEARCH, 16.0, pal.fg_muted))
                            .child(
                                input()
                                    .flex_1()
                                    .h(px(32.0))
                                    .bg(pal.bg_island)
                                    .text_color(pal.fg)
                                    .text_size(px(14.0))
                                    .placeholder("Go to file")
                                    .auto_focus(true)
                                    .value(move || state.quick_query.get())
                                    .on_change(move |value: &str| state.quick_query.set(value.to_owned()))
                                    .on_submit(move |_: &str| {
                                        let files = state.files();
                                        let needle = state.quick_query.get().to_lowercase();
                                        if let Some(path) = files
                                            .into_iter()
                                            .find(|path| path.to_string_lossy().to_lowercase().contains(&needle))
                                        {
                                            state.open_path(path);
                                            state.quick_open.set(false);
                                        }
                                    }),
                            )
                            .child(
                                view()
                                    .size(px(24.0))
                                    .flex_none()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(4.0))
                                    .hover(move |s| s.bg(pal.toolbar_hover))
                                    .on_press(move || state.quick_open.set(false))
                                    .child(icon(icons::CLOSE, 12.0, pal.fg_muted)),
                            ),
                    )
                    .child(scroll().flex_1().child(view().flex_col().py(px(4.0)).children(
                        matches.into_iter().enumerate().map(move |(index, path)| {
                            let display = path.strip_prefix(&root).unwrap_or(&path).to_string_lossy().to_string();
                            let title = file_name(&path);
                            let selected = index == 0;
                            view()
                                .h(px(28.0))
                                .flex_none()
                                .mx(px(4.0))
                                .flex_row()
                                .items_center()
                                .gap(px(8.0))
                                .px(px(8.0))
                                .rounded(px(5.0))
                                .bg(if selected { pal.tree_selected } else { transparent_black().into() })
                                .hover(move |s| s.bg(pal.tree_hover))
                                .on_press(move || {
                                    state.open_path(path.clone());
                                    state.quick_open.set(false);
                                })
                                .child(icon(crate::icons::file_icon(&title), 16.0, pal.fg_muted))
                                .child(text_ui(title.clone(), 13.0, pal.fg_strong))
                                .child(view().flex_1())
                                .child(if display == title {
                                    view().into_element()
                                } else {
                                    text_ui(display, 12.0, pal.fg_subtle).truncate().into_element()
                                })
                        }),
                    ))),
            ),
        )
        .into_element()
}

fn project_menu(state: State, pal: Palette) -> impl IntoElement {
    if !state.project_menu.get() {
        return view().into_element();
    }
    view()
        .absolute()
        .inset(px(0.0))
        .child(view().absolute().inset(px(0.0)).on_press(move || state.project_menu.set(false)))
        .child(
            view()
                .absolute()
                .top(px(38.0))
                .left(px(12.0))
                .w(px(250.0))
                .flex_col()
                .py(px(4.0))
                .rounded(px(8.0))
                .border(px(1.0))
                .border_color(pal.border_strong)
                .bg(pal.bg_island)
                .child(menu_item(pal, "Open…", true, move || {
                    state.project_menu.set(false);
                    choose_folder(state);
                }))
                .child(menu_item(pal, "Recent Projects", false, move || {
                    state.status.set("Recent projects are kept by the desktop shell".to_owned());
                }))
                .child(view().h(px(1.0)).my(px(4.0)).mx(px(8.0)).bg(pal.border))
                .child(menu_item(pal, "Close Project", false, move || {
                    state.project_menu.set(false);
                    state.tabs.set(Vec::new());
                    state.active.set(0);
                    state.active_path.set(PathBuf::new());
                    state.editor_seed.set(String::new());
                    state.status.set("Project closed".to_owned());
                })),
        )
        .into_element()
}

fn menu_item(pal: Palette, label: &'static str, enabled: bool, on_press: impl Fn() + 'static) -> View {
    view()
        .h(px(28.0))
        .flex_none()
        .mx(px(4.0))
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .px(px(8.0))
        .rounded(px(5.0))
        .hover(move |s| s.bg(pal.toolbar_hover))
        .on_press(on_press)
        .child(text_ui(label, 13.0, if enabled { pal.fg } else { pal.fg_faint }))
}

fn choose_folder(state: State) {
    let picked = pick_paths(OpenDialog::folders().multiple(false).button_label("Open"));
    spawn(async move {
        match picked.await {
            Ok(Some(paths)) => {
                if let Some(path) = paths.into_iter().next() {
                    state.reload_workspace(path);
                }
            }
            Ok(None) => {}
            Err(error) => state.status.set(format!("No folder dialog: {error}")),
        }
    });
}

fn settings_dialog(state: State, pal: Palette) -> impl IntoElement {
    if !state.settings.get() {
        return view().into_element();
    }
    view()
        .absolute()
        .inset(px(0.0))
        .items_center()
        .justify_center()
        .bg(c(0x00000040))
        .child(
            view()
                .w(px(900.0))
                .h(px(640.0))
                .flex_col()
                .overflow_hidden()
                .rounded(px(10.0))
                .border(px(1.0))
                .border_color(pal.border_strong)
                .bg(pal.bg_elev)
                .child(
                    view()
                        .h(px(46.0))
                        .flex_none()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .px(px(12.0))
                        .border_b_1()
                        .border_color(pal.border)
                        .child(icon(icons::SETTINGS, 16.0, pal.fg_muted))
                        .child(text_strong("Settings", 13.0, pal))
                        .child(text_muted("OpenCLion · native shell", 12.0, pal))
                        .child(view().flex_1())
                        .child(
                            view()
                                .size(px(24.0))
                                .items_center()
                                .justify_center()
                                .rounded(px(4.0))
                                .hover(move |s| s.bg(pal.toolbar_hover))
                                .on_press(move || state.settings.set(false))
                                .child(icon(icons::CLOSE, 14.0, pal.fg_muted)),
                        ),
                )
                .child(
                    view()
                        .flex_1()
                        .flex_row()
                        .min_h(px(0.0))
                        .child(settings_sidebar(state, pal))
                        .child(settings_content(state, pal)),
                ),
        )
        .into_element()
}

fn settings_sidebar(state: State, pal: Palette) -> impl IntoElement {
    view()
        .w(px(240.0))
        .flex_none()
        .h_full()
        .flex_col()
        .gap(px(1.0))
        .p(px(8.0))
        .border_r_1()
        .border_color(pal.border)
        .child(side_row(pal, "Appearance", true))
        .child(side_row(pal, "Editor", false))
        .child(side_row(pal, "Keymap", false))
        .child(side_row(pal, "Plugins", false))
        .child(side_row(pal, "Build, Execution, Deployment", false))
        .child(view().flex_1())
        .child(view().h(px(24.0)).items_center().px(px(8.0)).child(text_muted(
            format!("nsh 0.6 · Rust · {}", if state.dark.get() { "Dark" } else { "Light" }),
            11.0,
            pal,
        )))
}

fn side_row(pal: Palette, label: &'static str, active: bool) -> View {
    view()
        .h(px(26.0))
        .flex_none()
        .items_center()
        .px(px(8.0))
        .rounded(px(5.0))
        .bg(if active { pal.tree_selected } else { transparent_black().into() })
        .hover(move |s| s.bg(pal.tree_hover))
        .child(text_ui(label, 13.0, if active { pal.fg_strong } else { pal.fg }))
}

fn settings_content(state: State, pal: Palette) -> impl IntoElement {
    scroll()
        .flex_1()
        .h_full()
        .child(
            view()
                .flex_col()
                .p(px(20.0))
                .gap(px(16.0))
                .child(
                    text_ui("Appearance", 18.0, pal.fg_strong)
                        .font_weight(FontWeight::SEMIBOLD)
                        .font_family(UI_FONT.family),
                )
                .child(setting_row(
                    pal,
                    "Theme",
                    "Islands Light, the native OpenCLion theme",
                    view()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            view()
                                .h(px(26.0))
                                .px(px(12.0))
                                .items_center()
                                .rounded(px(5.0))
                                .bg(pal.accent)
                                .child(text_ui("Light", 12.0, white().into())),
                        )
                        .child(text_muted("Dark mode is on the roadmap (GTK scroll views)…", 11.0, pal)),
                ))
                .child(setting_row(
                    pal,
                    "Editor font",
                    "JetBrains Mono 13, line height 22 — drawn by the native NSH editor",
                    text_ui("JetBrains Mono 13", 12.0, pal.fg_secondary),
                ))
                .child(setting_row(
                    pal,
                    "Tool windows",
                    "Project, Commit, Structure, Bookmarks, CMake, Terminal",
                    text_muted("6 registered", 12.0, pal),
                ))
                .child(
                    view()
                        .flex_col()
                        .gap(px(6.0))
                        .child(text_ui("About this build", 13.0, pal.fg_strong))
                        .child(text_muted(
                            "The UI is Retained-mode Rust: nsh views, GTK on Linux, no WebView and no JavaScript engine. The Tauri frontend is not loaded.",
                            12.0,
                            pal,
                        )),
                )
                .child(view().h(px(12.0)))
                .child(
                    view()
                        .flex_row()
                        .justify_end()
                        .gap(px(6.0))
                        .child(
                            view()
                                .h(px(28.0))
                                .px(px(14.0))
                                .items_center()
                                .rounded(px(5.0))
                                .border(px(1.0))
                                .border_color(pal.border_strong)
                                .hover(move |s| s.bg(pal.toolbar_hover))
                                .on_press(move || state.settings.set(false))
                                .child(text_ui("Cancel", 13.0, pal.fg)),
                        )
                        .child(
                            view()
                                .h(px(28.0))
                                .px(px(18.0))
                                .items_center()
                                .rounded(px(5.0))
                                .bg(pal.accent)
                                .hover(move |s| s.opacity(0.92))
                                .on_press(move || state.settings.set(false))
                                .child(text_ui("OK", 13.0, white().into())),
                        ),
                ),
        )
}

fn setting_row(pal: Palette, title: &'static str, detail: &'static str, control: impl IntoElement) -> impl IntoElement {
    view()
        .flex_col()
        .w_full()
        .gap(px(6.0))
        .pb(px(14.0))
        .border_b_1()
        .border_color(pal.border)
        .child(text_ui(title, 13.0, pal.fg_strong))
        .child(text_muted(detail, 12.0, pal))
        .child(view().h(px(4.0)))
        .child(control)
}

fn toast(state: State, _pal: Palette) -> impl IntoElement {
    let Some(message) = state.toast.get() else {
        return view().into_element();
    };
    view()
        .absolute()
        .right(px(16.0))
        .bottom(px(48.0))
        .w(px(320.0))
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .p(px(10.0))
        .rounded(px(8.0))
        .bg(c(0x26282cff))
        .child(icon(icons::NOTIFICATIONS, 16.0, white().into()))
        .child(text_ui(message, 12.0, white().into()).flex_1())
        .child(
            view()
                .size(px(20.0))
                .items_center()
                .justify_center()
                .rounded(px(4.0))
                .hover(move |s| s.bg(c(0xffffff1f)))
                .on_press(move || state.toast.set(None))
                .child(icon(icons::CLOSE, 12.0, white().into())),
        )
        .into_element()
}

fn c(hex: u32) -> nsh_core::element::AdaptiveColor {
    rgba(hex).into()
}
