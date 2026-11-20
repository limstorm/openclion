//! Floating layers: Quick Open, the project menu, Settings and the toast.

use std::path::PathBuf;

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
        .child(dynamic(move || code_action_popup(state, pal).into_element()))
        .child(dynamic(move || worktree_dialog(state, pal).into_element()))
        .child(dynamic(move || app_menu(state, pal).into_element()))
        .child(dynamic(move || tree_menu(state, pal).into_element()))
        .child(dynamic(move || run_chooser(state, pal).into_element()))
        .child(dynamic(move || file_dialog(state, pal).into_element()))
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
                .child(menu_item(pal, "Recent Projects", false, move || {}))
                .child(dynamic(move || {
                    let recent = state.recent.get();
                    view()
                        .flex_col()
                        .children(recent.into_iter().take(4).map(move |path| {
                            let target = PathBuf::from(&path);
                            let label = target
                                .file_name()
                                .map(|name| name.to_string_lossy().to_string())
                                .unwrap_or_else(|| path.clone());
                            menu_item_owned(pal, label, move || {
                                state.reload_workspace(target.clone());
                                state.project_menu.set(false);
                            })
                        }))
                        .into_element()
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

fn menu_item_owned(pal: Palette, label: String, on_press: impl Fn() + 'static) -> View {
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
        .child(text_ui(label, 13.0, pal.fg))
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

pub(crate) fn choose_folder(state: State) {
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
        .child(side_row(state, pal, "Appearance", 0))
        .child(side_row(state, pal, "Editor", 1))
        .child(side_row(state, pal, "Keymap", 2))
        .child(side_row(state, pal, "Plugins", 3))
        .child(side_row(state, pal, "Build, Execution, Deployment", 4))
        .child(view().flex_1())
        .child(view().h(px(24.0)).items_center().px(px(8.0)).child(text_muted(
            format!("nsh 0.6 · Rust · {}", if state.dark.get() { "Dark" } else { "Light" }),
            11.0,
            pal,
        )))
}

fn side_row(state: State, pal: Palette, label: &'static str, index: u32) -> View {
    view()
        .h(px(26.0))
        .flex_none()
        .items_center()
        .px(px(8.0))
        .rounded(px(5.0))
        .bg(move || if state.settings_page.get() == index { pal.tree_selected } else { transparent_black().into() })
        .hover(move |s| s.bg(pal.tree_hover))
        .on_press(move || state.settings_page.set(index))
        .child(text_ui(label, 13.0, pal.fg))
}

fn settings_content(state: State, pal: Palette) -> impl IntoElement {
    view().flex_1().h_full().min_w(px(0.0)).child(dynamic(move || match state.settings_page.get() {
        0 => appearance_page(state, pal).into_element(),
        1 => editor_page(state, pal).into_element(),
        2 => keymap_page(state, pal).into_element(),
        3 => plugins_page(state, pal).into_element(),
        _ => build_page(state, pal).into_element(),
    }))
}

fn page_shell(title: &'static str, pal: Palette, children: impl IntoElement) -> impl IntoElement {
    scroll().flex_1().h_full().child(
        view()
            .flex_col()
            .p(px(20.0))
            .gap(px(14.0))
            .child(text_ui(title, 18.0, pal.fg_strong).font_weight(FontWeight::SEMIBOLD).font_family(UI_FONT.family))
            .child(children),
    )
}

fn editor_page(state: State, pal: Palette) -> impl IntoElement {
    page_shell(
        "Editor",
        pal,
        view()
            .flex_col()
            .gap(px(14.0))
            .child(setting_row(
                pal,
                "Font size",
                "JetBrains Mono, drawn by the native NSH editor",
                view()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .child(font_button(pal, "-", -1.0, state))
                    .child(
                        text_ui(move || format!("{} px", state.editor_font.get()), 12.0, pal.fg_secondary).w(px(54.0)),
                    )
                    .child(font_button(pal, "+", 1.0, state)),
            ))
            .child(setting_row(
                pal,
                "Line numbers",
                "Shown in the gutter, with fold markers",
                text_muted("On", 12.0, pal),
            ))
            .child(setting_row(
                pal,
                "Folding and indent guides",
                "Native editor folding and indent guides",
                text_muted("On", 12.0, pal),
            ))
            .child(setting_row(
                pal,
                "Tab size",
                "Four spaces for C/C++ (from the language definition)",
                text_muted("4 spaces", 12.0, pal),
            )),
    )
}

fn keymap_page(state: State, pal: Palette) -> impl IntoElement {
    let shortcuts = [
        ("Ctrl+P", "Go to File"),
        ("Ctrl+F", "Find in File"),
        ("Ctrl+H", "Replace in File"),
        ("Ctrl+Shift+F", "Find in Files"),
        ("Ctrl+Alt+L", "Format Document"),
        ("Alt+F7", "Find Usages"),
        ("Alt+Enter", "Code Actions"),
        ("Ctrl+Alt+H", "Type Hierarchy"),
        ("F12", "Go to Definition"),
        ("Shift+F6", "Rename Symbol"),
        ("F9", "Toggle Breakpoint"),
        ("Ctrl+S", "Save"),
        ("Ctrl+W", "Close Tab"),
        ("Ctrl+B", "Toggle Project"),
        ("Ctrl+J", "Toggle Bottom Tool Window"),
        ("Ctrl+`", "Terminal"),
        ("Ctrl+Alt+S", "Settings"),
    ];
    let _ = state;
    page_shell(
        "Keymap",
        pal,
        view().flex_col().w_full().gap(px(1.0)).children(shortcuts.into_iter().map(move |(keys, action)| {
            view()
                .h(px(26.0))
                .w_full()
                .flex_row()
                .items_center()
                .gap(px(12.0))
                .px(px(6.0))
                .rounded(px(4.0))
                .hover(move |s| s.bg(pal.tree_hover))
                .child(text_ui(keys, 12.0, pal.link).w(px(150.0)))
                .child(text_ui(action, 12.0, pal.fg))
        })),
    )
}

fn plugins_page(_state: State, pal: Palette) -> impl IntoElement {
    let plugins = [
        ("clangd", "C/C++ language server", "Bundled via ide-lsp"),
        ("lldb-dap", "Debug adapter", "Bundled via ide-dap"),
        ("git", "Version control", "Bundled via ide-vcs"),
        ("CMake", "Build system", "Bundled via ide-cmake"),
        ("Terminal", "Login shell over a PTY", "Bundled via ide-terminal"),
        ("ripgrep engine", "Find in Files", "Bundled via ide-search"),
    ];
    page_shell(
        "Plugins",
        pal,
        view().flex_col().w_full().gap(px(6.0)).children(plugins.into_iter().map(move |(name, what, status)| {
            view()
                .w_full()
                .flex_row()
                .items_center()
                .gap(px(10.0))
                .p(px(8.0))
                .rounded(px(6.0))
                .border(px(1.0))
                .border_color(pal.border)
                .child(icon(icons::SETTINGS, 16.0, pal.fg_muted))
                .child(
                    view()
                        .flex_col()
                        .gap(px(2.0))
                        .child(text_ui(name, 13.0, pal.fg_strong))
                        .child(text_muted(what, 11.0, pal)),
                )
                .child(view().flex_1())
                .child(text_muted(status, 11.0, pal))
                .child(text_ui("Enabled", 11.0, pal.success))
        })),
    )
}

fn build_page(state: State, pal: Palette) -> impl IntoElement {
    page_shell(
        "Build, Execution, Deployment",
        pal,
        view()
            .flex_col()
            .gap(px(14.0))
            .child(dynamic(move || {
                let model = state.cmake.get();
                let (builds, targets) = model
                    .as_ref()
                    .map(|model| {
                        (model.builds.len(), model.builds.iter().map(|build| build.targets.len()).sum::<usize>())
                    })
                    .unwrap_or((0, 0));
                text_muted(format!("CMake: {builds} build directories, {targets} targets"), 12.0, pal).into_element()
            }))
            .child(setting_row(
                pal,
                "CMake",
                "Configure and reload the project's File API model",
                view()
                    .flex_row()
                    .gap(px(6.0))
                    .child(text_button(pal, "Reload", move || state.refresh_cmake()))
                    .child(text_button(pal, "Configure", move || state.configure_cmake())),
            ))
            .child(setting_row(
                pal,
                "Run",
                "Build and run the first executable target",
                text_button(pal, "Build & Run", move || state.run_active_configuration()),
            )),
    )
}

fn appearance_page(state: State, pal: Palette) -> impl IntoElement {
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
                    "JetBrains Mono, drawn by the native NSH editor",
                    view()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(font_button(pal, "-", -1.0, state))
                        .child(
                            text_ui(
                                move || format!("{} px", state.editor_font.get()),
                                12.0,
                                pal.fg_secondary,
                            )
                            .w(px(54.0)),
                        )
                        .child(font_button(pal, "+", 1.0, state)),
                ))
                .child(
                    view()
                        .flex_col()
                        .gap(px(4.0))
                        .w_full()
                        .child(text_ui("Recent projects", 13.0, pal.fg_strong))
                        .child(dynamic(move || {
                            let recent = state.recent.get();
                            if recent.is_empty() {
                                return text_muted("No recent projects yet", 12.0, pal).into_element();
                            }
                            view()
                                .flex_col()
                                .gap(px(1.0))
                                .children(recent.into_iter().take(5).map(move |path| {
                                    let target = std::path::PathBuf::from(&path);
                                    let label = target
                                        .file_name()
                                        .map(|name| name.to_string_lossy().to_string())
                                        .unwrap_or_else(|| path.clone());
                                    view()
                                        .h(px(24.0))
                                        .w_full()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(6.0))
                                        .px(px(6.0))
                                        .rounded(px(4.0))
                                        .hover(move |s| s.bg(pal.tree_hover))
                                        .on_press(move || {
                                            state.reload_workspace(target.clone());
                                            state.settings.set(false);
                                        })
                                        .child(text_ui(label, 12.0, pal.fg))
                                        .child(text_muted(path, 10.0, pal).truncate())
                                }))
                                .into_element()
                        })),
                )
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
                                .on_press(move || crate::settings::restore_defaults(state))
                                .child(text_ui("Restore Defaults", 13.0, pal.fg)),
                        )
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

fn font_button(pal: Palette, label: &'static str, delta: f32, state: State) -> View {
    view()
        .size(px(26.0))
        .items_center()
        .justify_center()
        .rounded(px(5.0))
        .border(px(1.0))
        .border_color(pal.border_strong)
        .hover(move |s| s.bg(pal.toolbar_hover))
        .on_press(move || crate::settings::set_editor_font(state, state.editor_font.get() + delta))
        .child(text_ui(label, 13.0, pal.fg_secondary))
}

fn text_button(pal: Palette, label: &'static str, on_press: impl Fn() + 'static) -> View {
    view()
        .h(px(26.0))
        .px(px(10.0))
        .items_center()
        .justify_center()
        .rounded(px(5.0))
        .border(px(1.0))
        .border_color(pal.border_strong)
        .hover(move |s| s.bg(pal.toolbar_hover))
        .on_press(on_press)
        .child(text_ui(label, 12.0, pal.fg_secondary))
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

fn code_action_popup(state: State, pal: Palette) -> impl IntoElement {
    if !state.code_action_popup.get() {
        return view().into_element();
    }
    view()
        .absolute()
        .inset(px(0.0))
        .child(view().absolute().inset(px(0.0)).on_press(move || state.code_action_popup.set(false)))
        .child(
            view().absolute().top(px(96.0)).left(px(0.0)).right(px(0.0)).items_center().child(
                view()
                    .w(px(480.0))
                    .max_h(px(320.0))
                    .flex_col()
                    .overflow_hidden()
                    .rounded(px(9.0))
                    .border(px(1.0))
                    .border_color(pal.border_strong)
                    .bg(pal.bg_island)
                    .child(
                        view()
                            .h(px(34.0))
                            .flex_none()
                            .flex_row()
                            .items_center()
                            .px(px(10.0))
                            .border_b_1()
                            .border_color(pal.border)
                            .child(text_strong("Code Actions", 12.0, pal)),
                    )
                    .child(scroll().flex_1().child(dynamic(move || {
                        let titles = state.code_action_titles.get();
                        view()
                            .flex_col()
                            .w_full()
                            .py(px(4.0))
                            .children(titles.into_iter().enumerate().map(move |(index, title)| {
                                view()
                                    .h(px(26.0))
                                    .flex_none()
                                    .mx(px(4.0))
                                    .flex_row()
                                    .items_center()
                                    .gap(px(6.0))
                                    .px(px(8.0))
                                    .rounded(px(5.0))
                                    .hover(move |s| s.bg(pal.toolbar_hover))
                                    .on_press(move || crate::lsp::apply_code_action(state, index))
                                    .child(icon(icons::SETTINGS, 13.0, pal.fg_muted))
                                    .child(text_ui(title, 12.0, pal.fg))
                            }))
                            .into_element()
                    }))),
            ),
        )
        .into_element()
}

fn worktree_dialog(state: State, pal: Palette) -> impl IntoElement {
    if !state.worktree_dialog.get() {
        return view().into_element();
    }
    let root = state.root.get();

    let branch = state.worktree_branch.get();
    let folder = if branch.trim().is_empty() {
        root.join(format!("../{}-wt", state.root_name()))
    } else {
        root.join(format!("../{}", branch.trim()))
    };
    view()
        .absolute()
        .inset(px(0.0))
        .items_center()
        .justify_center()
        .bg(c(0x00000040))
        .child(
            view()
                .w(px(520.0))
                .flex_col()
                .gap(px(10.0))
                .p(px(14.0))
                .rounded(px(8.0))
                .border(px(1.0))
                .border_color(pal.border_strong)
                .bg(pal.bg_island)
                .child(text_strong("New Worktree", 13.0, pal))
                .child(text_muted("Check out a branch or revision in another folder", 11.0, pal))
                .child(text_muted("Check out", 10.0, pal))
                .child(
                    input()
                        .h(px(28.0))
                        .px(px(8.0))
                        .rounded(px(5.0))
                        .border(px(1.0))
                        .border_color(pal.border_strong)
                        .bg(pal.bg_island)
                        .text_color(pal.fg)
                        .text_size(px(12.0))
                        .placeholder("branch or revision")
                        .auto_focus(true)
                        .value(move || state.worktree_revision.get())
                        .on_change(move |value: &str| state.worktree_revision.set(value.to_owned())),
                )
                .child(text_muted("New branch (optional)", 10.0, pal))
                .child(
                    input()
                        .h(px(28.0))
                        .px(px(8.0))
                        .rounded(px(5.0))
                        .border(px(1.0))
                        .border_color(pal.border_strong)
                        .bg(pal.bg_island)
                        .text_color(pal.fg)
                        .text_size(px(12.0))
                        .placeholder("leave empty to check out as is")
                        .value(move || state.worktree_branch.get())
                        .on_change(move |value: &str| state.worktree_branch.set(value.to_owned())),
                )
                .child(text_muted(format!("Folder: {}", folder.display()), 10.0, pal).truncate())
                .child(
                    view()
                        .flex_row()
                        .justify_end()
                        .gap(px(6.0))
                        .child(
                            view()
                                .h(px(26.0))
                                .px(px(12.0))
                                .items_center()
                                .rounded(px(5.0))
                                .border(px(1.0))
                                .border_color(pal.border_strong)
                                .hover(move |s| s.bg(pal.toolbar_hover))
                                .on_press(move || state.worktree_dialog.set(false))
                                .child(text_ui("Cancel", 12.0, pal.fg)),
                        )
                        .child(
                            view()
                                .h(px(26.0))
                                .px(px(14.0))
                                .items_center()
                                .rounded(px(5.0))
                                .bg(pal.accent)
                                .on_press(move || {
                                    state.worktree_dialog.set(false);
                                    state.add_worktree(
                                        folder.clone(),
                                        state.worktree_revision.get(),
                                        state.worktree_branch.get(),
                                    );
                                })
                                .child(text_ui("Create", 12.0, white().into())),
                        ),
                ),
        )
        .into_element()
}

fn app_menu(state: State, pal: Palette) -> impl IntoElement {
    if !state.app_menu.get() {
        return view().into_element();
    }
    let mut items: Vec<View> = Vec::new();
    macro_rules! push {
        ($label:expr, $shortcut:expr, $action:expr) => {
            items.push(app_menu_item(pal, $label, $shortcut, {
                move || {
                    state.app_menu.set(false);
                    $action();
                }
            }))
        };
    }
    push!("New File…", "Ctrl+Alt+N", Box::new(move || state.open_new_file()));
    push!("Open Project…", "", Box::new(move || choose_folder(state)));
    push!(
        "Close Project",
        "",
        Box::new(move || {
            state.tabs.set(Vec::new());
            state.active.set(0);
            state.active_path.set(PathBuf::new());
            state.editor_seed.set(String::new());
            state.status.set("Project closed".to_owned());
        })
    );
    items.push(view().h(px(1.0)).my(px(4.0)).mx(px(8.0)).bg(pal.border));
    push!(
        "Find in Files",
        "Ctrl+Shift+F",
        Box::new(move || {
            state.left_tool.set(crate::model::LeftTool::Search);
            state.show_left.set(true);
        })
    );
    push!(
        "Git",
        "Alt+9",
        Box::new(move || {
            state.bottom_tool.set(Some(crate::model::BottomTool::Git));
            state.show_bottom.set(true);
        })
    );
    push!(
        "CMake",
        "",
        Box::new(move || {
            state.bottom_tool.set(Some(crate::model::BottomTool::Cmake));
            state.show_bottom.set(true);
        })
    );
    push!(
        "Terminal",
        "Alt+F12",
        Box::new(move || {
            state.bottom_tool.set(Some(crate::model::BottomTool::Terminal));
            state.show_bottom.set(true);
        })
    );
    items.push(view().h(px(1.0)).my(px(4.0)).mx(px(8.0)).bg(pal.border));
    push!("Run", "Shift+F10", Box::new(move || state.run_active_configuration()));
    push!("Debug", "Shift+F9", Box::new(move || crate::debug::start(state)));
    items.push(view().h(px(1.0)).my(px(4.0)).mx(px(8.0)).bg(pal.border));
    push!("Settings…", "Ctrl+Alt+S", Box::new(move || state.settings.set(true)));
    push!("Exit", "", Box::new(move || nsh_core::element::desktop::current_window().close()));
    view()
        .absolute()
        .inset(px(0.0))
        .child(view().absolute().inset(px(0.0)).on_press(move || state.app_menu.set(false)))
        .child(
            view()
                .absolute()
                .left(px(6.0))
                .top(px(42.0))
                .w(px(280.0))
                .flex_col()
                .py(px(5.0))
                .rounded(px(8.0))
                .border(px(1.0))
                .border_color(pal.border_strong)
                .bg(pal.bg_island)
                .children(items),
        )
        .into_element()
}

fn app_menu_item(pal: Palette, label: &'static str, shortcut: &'static str, on_press: impl Fn() + 'static) -> View {
    view()
        .h(px(26.0))
        .flex_none()
        .mx(px(4.0))
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .px(px(8.0))
        .rounded(px(5.0))
        .hover(move |s| s.bg(pal.toolbar_hover))
        .on_press(on_press)
        .child(text_ui(label, 12.0, pal.fg).flex_1())
        .child(text_muted(shortcut, 10.0, pal))
}

fn run_chooser(state: State, pal: Palette) -> impl IntoElement {
    if !state.run_chooser.get() {
        return view().into_element();
    }
    view()
        .absolute()
        .inset(px(0.0))
        .child(view().absolute().inset(px(0.0)).on_press(move || state.run_chooser.set(false)))
        .child(
            view().absolute().left(px(0.0)).right(px(0.0)).top(px(92.0)).items_center().child(
                view()
                    .w(px(560.0))
                    .max_h(px(340.0))
                    .flex_col()
                    .overflow_hidden()
                    .rounded(px(10.0))
                    .border(px(1.0))
                    .border_color(pal.border_strong)
                    .bg(pal.bg_island)
                    .child(
                        view()
                            .h(px(40.0))
                            .flex_none()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .px(px(12.0))
                            .border_b_1()
                            .border_color(pal.border)
                            .child(icon(icons::RUN, 16.0, pal.success))
                            .child(text_strong("Run Configurations", 13.0, pal))
                            .child(view().flex_1())
                            .child(
                                view()
                                    .size(px(22.0))
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(4.0))
                                    .hover(move |s| s.bg(pal.toolbar_hover))
                                    .on_press(move || state.run_chooser.set(false))
                                    .child(icon(icons::CLOSE, 12.0, pal.fg_muted)),
                            ),
                    )
                    .child(scroll().flex_1().child(dynamic(move || {
                        let model = state.cmake.get();
                        let selected = state.run_target.get();
                        let mut rows: Vec<View> = Vec::new();
                        if let Some(model) = &model {
                            for (build, target) in model.executables() {
                                let name = target.name.clone();
                                let name_for_press = name.clone();
                                let active = selected.as_deref() == Some(target.name.as_str());
                                let detail = format!("{}  ·  {}", build.configuration(), build.dir);
                                let artifact = target
                                    .artifact
                                    .as_ref()
                                    .map(|artifact| {
                                        std::path::Path::new(artifact)
                                            .file_name()
                                            .map(|n| n.to_string_lossy().to_string())
                                            .unwrap_or_else(|| artifact.clone())
                                    })
                                    .unwrap_or_else(|| "not built".to_owned());
                                rows.push(
                                    view()
                                        .test_id(&format!("run-target-{name}"))
                                        .h(px(40.0))
                                        .flex_none()
                                        .mx(px(4.0))
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
                                            state.run_chooser.set(false);
                                        })
                                        .child(icon(icons::RUN, 14.0, pal.success))
                                        .child(
                                            view()
                                                .flex_col()
                                                .gap(px(1.0))
                                                .child(text_ui(name, 13.0, pal.fg_strong))
                                                .child(text_muted(detail, 10.0, pal)),
                                        )
                                        .child(view().flex_1())
                                        .child(text_muted(artifact, 10.0, pal)),
                                );
                            }
                        }
                        if rows.is_empty() {
                            return view()
                                .p(px(14.0))
                                .child(text_muted("No executable targets yet — configure CMake first", 12.0, pal))
                                .into_element();
                        }
                        view().flex_col().w_full().py(px(4.0)).children(rows).into_element()
                    }))),
            ),
        )
        .into_element()
}

fn tree_menu(state: State, pal: Palette) -> impl IntoElement {
    let Some(menu) = state.tree_menu.get() else {
        return view().into_element();
    };
    let base = if menu.dir {
        menu.path.clone()
    } else {
        menu.path.parent().map(PathBuf::from).unwrap_or_else(|| state.root.get())
    };
    let path = menu.path.clone();
    let path_for_delete = menu.path.clone();
    view()
        .absolute()
        .inset(px(0.0))
        .child(view().absolute().inset(px(0.0)).on_press(move || state.tree_menu.set(None)))
        .child(
            view()
                .absolute()
                .left(px(menu.x))
                .top(px(menu.y))
                .w(px(220.0))
                .flex_col()
                .py(px(4.0))
                .rounded(px(8.0))
                .border(px(1.0))
                .border_color(pal.border_strong)
                .bg(pal.bg_island)
                .child(menu_item_owned(pal, "New File…".to_owned(), {
                    let base = base.clone();
                    move || {
                        state.tree_menu.set(None);
                        state.selected.set(Some(base.to_string_lossy().to_string()));
                        state.open_new_file();
                    }
                }))
                .child(menu_item_owned(pal, "New Folder…".to_owned(), {
                    let base = base.clone();
                    move || {
                        state.tree_menu.set(None);
                        state.selected.set(Some(base.to_string_lossy().to_string()));
                        state.open_new_folder();
                    }
                }))
                .child(view().h(px(1.0)).my(px(4.0)).mx(px(8.0)).bg(pal.border))
                .child(menu_item_owned(pal, "Rename…".to_owned(), {
                    let path = path.clone();
                    move || {
                        state.tree_menu.set(None);
                        state.open_rename(path.clone());
                    }
                }))
                .child(menu_item_owned(pal, "Delete…".to_owned(), {
                    let path = path_for_delete;
                    move || {
                        state.tree_menu.set(None);
                        state.open_delete(path.clone());
                    }
                }))
                .child(view().h(px(1.0)).my(px(4.0)).mx(px(8.0)).bg(pal.border))
                .child(menu_item_owned(pal, "Copy Path".to_owned(), {
                    let path = path.clone();
                    move || {
                        state.tree_menu.set(None);
                        let text = path.to_string_lossy().to_string();
                        set_clipboard(text.clone());
                        state.toast.set(Some(format!("Copied {text}")));
                    }
                }))
                .child(menu_item_owned(pal, "Reveal in File Manager".to_owned(), {
                    move || {
                        state.tree_menu.set(None);
                        let path = path.clone();
                        spawn(async move {
                            let _ = reveal_path(path).await;
                        });
                    }
                })),
        )
        .into_element()
}

fn file_dialog(state: State, pal: Palette) -> impl IntoElement {
    let Some(dialog) = state.dialog.get() else {
        return view().into_element();
    };
    let (title, hint, show_input, action, danger) = match &dialog {
        crate::model::FileDialog::NewFile { base } => {
            ("New File", base.to_string_lossy().to_string(), true, "Create File", false)
        }
        crate::model::FileDialog::NewFolder { base } => {
            ("New Folder", base.to_string_lossy().to_string(), true, "Create Folder", false)
        }
        crate::model::FileDialog::Rename { path } => {
            ("Rename", path.to_string_lossy().to_string(), true, "Rename", false)
        }
        crate::model::FileDialog::RenameSymbol { .. } => {
            ("Rename Symbol", "Renames the symbol at the caret across the project".to_owned(), true, "Rename", false)
        }
        crate::model::FileDialog::BreakpointCondition { path, line } => {
            ("Breakpoint Condition", format!("{}:{}", path.display(), line + 1), true, "Set", false)
        }
        crate::model::FileDialog::BreakpointLog { path, line } => {
            ("Logpoint Message", format!("{}:{}", path.display(), line + 1), true, "Set", false)
        }
        crate::model::FileDialog::Delete { path, dir } => (
            if *dir { "Delete Folder" } else { "Delete File" },
            path.to_string_lossy().to_string(),
            false,
            "Delete",
            true,
        ),
    };
    view()
        .absolute()
        .inset(px(0.0))
        .items_center()
        .justify_center()
        .bg(c(0x00000040))
        .child(
            view()
                .w(px(460.0))
                .flex_col()
                .gap(px(10.0))
                .p(px(14.0))
                .rounded(px(8.0))
                .border(px(1.0))
                .border_color(pal.border_strong)
                .bg(pal.bg_island)
                .child(text_strong(title, 13.0, pal))
                .child(text_muted(hint, 11.0, pal).truncate())
                .child(dynamic(move || {
                    if !show_input {
                        return view().child(text_muted("This cannot be undone.", 12.0, pal)).into_element();
                    }
                    input()
                        .h(px(30.0))
                        .px(px(8.0))
                        .rounded(px(5.0))
                        .border(px(1.0))
                        .border_color(pal.accent)
                        .bg(pal.bg_island)
                        .text_color(pal.fg)
                        .text_size(px(13.0))
                        .placeholder("Name")
                        .auto_focus(true)
                        .value(move || state.dialog_input.get())
                        .on_change(move |value: &str| state.dialog_input.set(value.to_owned()))
                        .on_submit(move |_: &str| state.confirm_dialog())
                        .into_element()
                }))
                .child(
                    view()
                        .flex_row()
                        .justify_end()
                        .gap(px(6.0))
                        .child(
                            view()
                                .h(px(26.0))
                                .px(px(12.0))
                                .items_center()
                                .rounded(px(5.0))
                                .border(px(1.0))
                                .border_color(pal.border_strong)
                                .hover(move |s| s.bg(pal.toolbar_hover))
                                .on_press(move || state.dialog.set(None))
                                .child(text_ui("Cancel", 12.0, pal.fg)),
                        )
                        .child(
                            view()
                                .h(px(26.0))
                                .px(px(14.0))
                                .items_center()
                                .rounded(px(5.0))
                                .bg(if danger { pal.danger } else { pal.accent })
                                .on_press(move || state.confirm_dialog())
                                .child(text_ui(action, 12.0, white().into())),
                        ),
                ),
        )
        .into_element()
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
