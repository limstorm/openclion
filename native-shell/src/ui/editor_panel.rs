//! The editor island: the tab strip (IDEA's rounded selected pill) and the
//! native NSH code editor.

use std::path::PathBuf;

use nsh_base::editor::{
    EditorApi, EditorClasses, EditorCommand, EditorCompletionRequest, EditorSearch, EditorSearchResult,
    EditorSelection, editor,
};
use nsh_core::element::FontWeight;
use nsh_core::element::input;
use nsh_core::prelude::*;

use crate::icons;
use crate::model::{State, language_of};
use crate::theme::{Palette, UI_FONT};
use crate::ui::{icon, text_muted, text_ui};

pub fn editor_area(state: State, pal: Palette) -> impl IntoElement {
    view()
        .flex_1()
        .flex_col()
        .min_h(px(0.0))
        .w_full()
        .overflow_hidden()
        .rounded(px(10.0))
        .bg(pal.bg_island)
        .relative()
        .child(tabs_row(state, pal))
        .child(
            view()
                .flex_1()
                .flex_col()
                .min_h(px(0.0))
                .w_full()
                .child(dynamic(move || editor_body(state, pal).into_element())),
        )
        .child(dynamic(move || find_bar(state, pal).into_element()))
}

fn tabs_row(state: State, pal: Palette) -> impl IntoElement {
    view().h(px(38.0)).flex_none().w_full().child(dynamic(move || {
        let tabs = state.tabs.get();
        let active = state.active.get();
        view()
            .h(px(38.0))
            .w_full()
            .flex_row()
            .items_center()
            .gap(px(2.0))
            .px(px(4.0))
            .overflow_hidden()
            .children(tabs.iter().enumerate().map(|(index, tab)| tab_item(state, pal, index, tab, index == active)))
            .child(
                view()
                    .size(px(28.0))
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.0))
                    .hover(move |s| s.bg(pal.toolbar_hover))
                    .on_press(move || {
                        state.quick_query.set(String::new());
                        state.quick_open.set(true);
                    })
                    .child(icon(icons::ADD, 16.0, pal.fg_muted)),
            )
            .child(
                view()
                    .size(px(28.0))
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.0))
                    .hover(move |s| s.bg(pal.toolbar_hover))
                    .on_press(move || {
                        state.show_right.update(|open| *open = !*open);
                    })
                    .child(icon(icons::SPLIT, 16.0, pal.fg_muted)),
            )
            .child(
                view()
                    .size(px(28.0))
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.0))
                    .hover(move |s| s.bg(pal.toolbar_hover))
                    .on_press(move || {
                        state.status.set("Editor tab options: Close All, Split Right".to_owned());
                    })
                    .child(icon(icons::MORE_HORIZONTAL, 16.0, pal.fg_muted)),
            )
            .child(view().flex_1())
    }))
}

fn tab_item(state: State, pal: Palette, index: usize, tab: &crate::model::Tab, active: bool) -> View {
    let title = tab.title.clone();
    let icon_markup = crate::icons::file_icon(&title);
    let close_path = tab.path.clone();
    let modified = tab.modified;
    view()
        .h(px(28.0))
        .max_w(px(220.0))
        .flex_none()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .pl(px(10.0))
        .pr(px(6.0))
        .rounded(px(6.0))
        .bg(if active { pal.tab_selected } else { transparent_black().into() })
        .border(if active { px(1.0) } else { px(0.0) })
        .border_color(pal.tab_border)
        .hover(move |s| if active { s.bg(pal.tab_selected) } else { s.bg(pal.tab_hover) })
        .on_press(move || {
            state.activate_tab(index);
        })
        .child(icon(icon_markup, 16.0, pal.fg_muted))
        .child(text_ui(title.clone(), 13.0, pal.fg_strong).font_family(UI_FONT.family).truncate())
        .child(if modified {
            view()
                .size(px(16.0))
                .flex_none()
                .items_center()
                .justify_center()
                .child(view().size(px(6.0)).rounded(px(3.0)).bg(pal.fg_secondary))
                .into_element()
        } else {
            view()
                .size(px(16.0))
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(px(4.0))
                .hover(move |s| s.bg(pal.toolbar_hover))
                .on_press(move || {
                    if let Some(index) = state.tabs.get().iter().position(|tab| tab.path == close_path) {
                        state.close_tab(index);
                    }
                })
                .child(icon(icons::CLOSE, 12.0, pal.fg_muted))
                .into_element()
        })
}

fn editor_body(state: State, pal: Palette) -> impl IntoElement {
    let path: PathBuf = state.active_path.get();
    if path.as_os_str().is_empty() {
        return welcome(state, pal).into_element();
    }
    let seed = state.editor_seed.get();
    let language = language_of(&path);
    let font = state.editor_font.get();
    let scale = {
        let value = font_scale();
        if value > 0.1 { value } else { 1.0 }
    };
    let mut classes = EditorClasses::default();
    classes.line_number = Some("text-[12px]".into());
    classes.content = Some("pl-2".into());
    editor()
        .component_ref(&state.editor)
        .key(format!("{}-{}", path.to_string_lossy(), pal.dark))
        .flex_1()
        .w_full()
        .min_h(px(0.0))
        .default_value(seed)
        .language(language)
        .font_family(SharedString::from("JetBrains Mono, monospace"))
        .font_size(px(font / scale))
        .line_height(px((font + 9.0) / scale))
        .line_numbers(true)
        .folding(true)
        .indent_guides(true)
        .active_line(false)
        .completion(true)
        .completion_trigger_characters(vec![
            SharedString::from("."),
            SharedString::from("->"),
            SharedString::from("::"),
            SharedString::from("<"),
        ])
        .classes(classes)
        .diagnostics(move || state.diagnostics.get())
        .decorations(move || crate::debug::decorations(state))
        .search(move || {
            if !state.find_open.get() {
                return None;
            }
            let query = state.find_query.get();
            if query.is_empty() {
                return None;
            }
            Some(EditorSearch::new(query))
        })
        .on_search_change(move |result: &EditorSearchResult| {
            state.find_count.set(result.count);
            state.find_current.set(result.current);
        })
        .on_line_number_press(move |line: &u32| {
            crate::debug::toggle_breakpoint(state, *line);
        })
        .on_change(move |text: &SharedString| {
            let text = text.to_string();
            state.update_active_text(text.clone());
            crate::lsp::change(state, &text);
        })
        .on_completion_request(move |request: &EditorCompletionRequest| {
            crate::lsp::completion(state, request, state.editor);
        })
        .on_definition_request(move |request: &nsh_base::editor::EditorDefinitionRequest| {
            crate::lsp::definition(state, request.position, state.editor);
        })
        .on_selection_change(move |selections: &Vec<EditorSelection>| {
            if let Some(selection) = selections.last() {
                state.cursor.set((selection.head.line + 1, selection.head.column + 1));
            }
        })
        .into_element()
}

fn find_bar(state: State, pal: Palette) -> impl IntoElement {
    if !state.find_open.get() {
        return view().into_element();
    }
    let replace = state.replace_open.get();
    view()
        .absolute()
        .top(px(42.0))
        .right(px(18.0))
        .w(px(380.0))
        .flex_col()
        .gap(px(4.0))
        .p(px(6.0))
        .rounded(px(7.0))
        .border(px(1.0))
        .border_color(pal.border_strong)
        .bg(pal.bg_island)
        .child(
            view()
                .h(px(28.0))
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .child(icon(icons::SEARCH, 14.0, pal.fg_muted))
                .child(
                    input()
                        .flex_1()
                        .h(px(24.0))
                        .bg(pal.bg_island)
                        .text_color(pal.fg)
                        .text_size(px(12.0))
                        .placeholder("Find")
                        .auto_focus(true)
                        .value(move || state.find_query.get())
                        .on_change(move |value: &str| state.find_query.set(value.to_owned()))
                        .on_submit(move |_: &str| find_step(state, true)),
                )
                .child(text_muted(
                    move || {
                        let count = state.find_count.get();
                        if count == 0 {
                            "No matches".to_owned()
                        } else {
                            let current = state.find_current.get().map(|c| c + 1).unwrap_or(1);
                            format!("{current}/{count}")
                        }
                    },
                    11.0,
                    pal,
                ))
                .child(bar_button(pal, icons::CHEVRON_UP, "Previous", move || find_step(state, false)))
                .child(bar_button(pal, icons::CHEVRON_DOWN, "Next", move || find_step(state, true)))
                .child(bar_button(pal, icons::CLOSE, "Close", move || {
                    state.find_open.set(false);
                })),
        )
        .child(dynamic(move || {
            if !replace {
                return view().into_element();
            }
            view()
                .flex_col()
                .gap(px(4.0))
                .child(
                    view()
                        .h(px(28.0))
                        .flex_row()
                        .items_center()
                        .gap(px(4.0))
                        .child(
                            input()
                                .flex_1()
                                .h(px(24.0))
                                .bg(pal.bg_island)
                                .text_color(pal.fg)
                                .text_size(px(12.0))
                                .placeholder("Replace")
                                .value(move || state.replace_query.get())
                                .on_change(move |value: &str| state.replace_query.set(value.to_owned()))
                                .on_submit(move |_: &str| replace_current(state)),
                        )
                        .child(text_button(pal, "Replace", move || replace_current(state)))
                        .child(text_button(pal, "All", move || replace_all(state))),
                )
                .into_element()
        }))
        .into_element()
}

fn bar_button(pal: Palette, markup: &'static str, label: &'static str, on_press: impl Fn() + 'static) -> View {
    view()
        .size(px(24.0))
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .hover(move |s| s.bg(pal.toolbar_hover))
        .label(label)
        .on_press(on_press)
        .child(icon(markup, 13.0, pal.fg_muted))
}

fn text_button(pal: Palette, label: &'static str, on_press: impl Fn() + 'static) -> View {
    view()
        .h(px(24.0))
        .px(px(8.0))
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .border(px(1.0))
        .border_color(pal.border_strong)
        .hover(move |s| s.bg(pal.toolbar_hover))
        .on_press(on_press)
        .child(text_ui(label, 11.0, pal.fg_secondary))
}

fn find_step(state: State, forward: bool) {
    if let Some(api) = state.editor.api::<EditorApi>() {
        api.run(if forward { EditorCommand::FindNext } else { EditorCommand::FindPrevious });
    }
}

fn replace_current(state: State) {
    if let Some(api) = state.editor.api::<EditorApi>() {
        api.replace(state.replace_query.get());
    }
}

fn replace_all(state: State) {
    if let Some(api) = state.editor.api::<EditorApi>() {
        let result = api.replace_all(state.replace_query.get());
        state.status.set(format!("Replaced {} occurrence(s)", result.count));
    }
}

fn welcome(state: State, pal: Palette) -> impl IntoElement {
    view().flex_1().w_full().items_center().justify_center().child(
        view()
            .flex_col()
            .items_center()
            .gap(px(10.0))
            .child(text_ui("OpenCLion", 24.0, pal.fg_faint).font_weight(FontWeight::LIGHT).font_family(UI_FONT.family))
            .child(text_muted("Open a file from the Project tool window", 13.0, pal))
            .child(
                view()
                    .flex_col()
                    .gap(px(6.0))
                    .child(hint(pal, "Ctrl+P", "Go to File"))
                    .child(hint(pal, "Ctrl+Shift+F", "Find in Files"))
                    .child(hint(pal, "Ctrl+S", "Save All")),
            )
            .child(
                view()
                    .mt(px(6.0))
                    .px(px(12.0))
                    .py(px(6.0))
                    .rounded(px(6.0))
                    .bg(pal.accent)
                    .on_press(move || {
                        state.quick_query.set(String::new());
                        state.quick_open.set(true);
                    })
                    .child(text_ui("Open a file", 13.0, white().into())),
            ),
    )
}

fn hint(pal: Palette, keys: &'static str, label: &'static str) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap(px(12.0))
        .child(text_ui(keys, 12.0, pal.link).font_family(UI_FONT.family).w(px(96.0)))
        .child(text_ui(label, 12.0, pal.fg_muted).w(px(120.0)))
}
