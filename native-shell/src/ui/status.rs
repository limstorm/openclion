//! The 34-point status bar: file breadcrumbs on the left, editor details on
//! the right, exactly where IntelliJ's New UI puts them.

use std::path::PathBuf;

use nsh_core::prelude::*;

use crate::icons;
use crate::model::State;
use crate::theme::Palette;
use crate::ui::{icon, text_ui};

pub fn status_bar(state: State, pal: Palette) -> impl IntoElement {
    view()
        .h(px(34.0))
        .flex_none()
        .w_full()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .pl(px(12.0))
        .pr(px(10.0))
        .child(breadcrumbs(state, pal))
        .child(view().flex_1())
        .child(
            text_ui(move || format!("{}:{}", state.cursor.get().0, state.cursor.get().1), 13.0, pal.fg_muted)
                .flex_none(),
        )
        .child(item(pal, "LF"))
        .child(item(pal, "UTF-8"))
        .child(item(pal, "Vim"))
        .child(item(pal, "4 spaces"))
        .child(
            view()
                .size(px(24.0))
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(px(4.0))
                .hover(move |s| s.bg(pal.toolbar_hover))
                .on_press(move || state.settings.set(true))
                .child(icon(icons::SETTINGS, 16.0, pal.fg_muted)),
        )
}

fn item(pal: Palette, label: &'static str) -> View {
    view().flex_none().px(px(4.0)).child(text_ui(label, 13.0, pal.fg_muted))
}

fn breadcrumbs(state: State, pal: Palette) -> impl IntoElement {
    view().flex_row().items_center().gap(px(4.0)).child(dynamic(move || {
        let root = state.root.get();
        let active: PathBuf = state.active_path.get();
        let root_name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| root.to_string_lossy().to_string());
        let mut crumbs: Vec<String> = Vec::new();
        if let Ok(relative) = active.strip_prefix(&root) {
            let parts: Vec<String> =
                relative.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
            crumbs.extend(parts);
        }
        view()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .px(px(2.0))
                    .rounded(px(4.0))
                    .child(icon(icons::PROJECT, 16.0, pal.fg_muted))
                    .child(text_ui(root_name, 13.0, pal.fg_secondary)),
            )
            .children(crumbs.into_iter().map(move |crumb| {
                view()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .child(icon(icons::CHEVRON_RIGHT, 10.0, pal.fg_faint))
                    .child(icon(crate::icons::file_icon(&crumb), 16.0, pal.fg_muted))
                    .child(text_ui(crumb, 13.0, pal.fg_secondary))
            }))
    }))
}
