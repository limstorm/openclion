//! The editor island: the tab strip (IDEA's rounded selected pill) and the
//! native NSH code editor.

use std::path::PathBuf;

use nsh_base::editor::{EditorClasses, EditorCompletionRequest, EditorSelection, editor};
use nsh_core::element::FontWeight;
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
        .child(tabs_row(state, pal))
        .child(
            view()
                .flex_1()
                .flex_col()
                .min_h(px(0.0))
                .w_full()
                .child(dynamic(move || editor_body(state, pal).into_element())),
        )
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
        .font_size(px(13.0 / scale))
        .line_height(px(22.0 / scale))
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
