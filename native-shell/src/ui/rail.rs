//! The two tool window stripes (IDEA's left and right side bars).

use nsh_core::prelude::*;

use crate::icons;
use crate::model::{BottomTool, LeftTool, RightTool, State};
use crate::theme::Palette;
use crate::ui::icon;

pub fn left_rail(state: State, pal: Palette) -> impl IntoElement {
    view()
        .w(px(36.0))
        .flex_none()
        .h_full()
        .flex_col()
        .items_center()
        .pt(px(4.0))
        .pb(px(6.0))
        .gap(px(2.0))
        .child(left_button(state, pal, icons::PROJECT, LeftTool::Project, "Project  Alt+1"))
        .child(left_button(state, pal, icons::COMMIT, LeftTool::Commit, "Commit  Alt+0"))
        .child(left_button(state, pal, icons::STRUCTURE, LeftTool::Structure, "Structure"))
        .child(left_button(state, pal, icons::BOOKMARKS, LeftTool::Bookmarks, "Bookmarks"))
        .child(left_button(state, pal, icons::SEARCH, LeftTool::Search, "Find in Files  Ctrl+Shift+F"))
        .child(view().flex_1())
        .child(bottom_button(state, pal, icons::CMAKE, BottomTool::Cmake, "CMake"))
        .child(bottom_button(state, pal, icons::BRANCH, BottomTool::Git, "Git  Alt+9"))
        .child(bottom_button(state, pal, icons::DEBUG_COLORED, BottomTool::Debug, "Debug"))
        .child(bottom_button(state, pal, icons::RUN, BottomTool::Run, "Run  Alt+4"))
        .child(bottom_button(state, pal, icons::TERMINAL, BottomTool::Terminal, "Terminal  Alt+F12"))
        .child(bottom_button(state, pal, icons::PROBLEMS, BottomTool::Problems, "Problems  Alt+6"))
}

pub fn right_rail(state: State, pal: Palette) -> impl IntoElement {
    view()
        .w(px(36.0))
        .flex_none()
        .h_full()
        .flex_col()
        .items_center()
        .pt(px(4.0))
        .pb(px(6.0))
        .gap(px(2.0))
        .child(right_button(state, pal, icons::NOTIFICATIONS, RightTool::Notifications, "Notifications"))
}

fn rail_button_base(pal: Palette) -> View {
    view()
        .size(px(32.0))
        .flex_none()
        .items_center()
        .justify_center()
        .rounded(px(8.0))
        .hover(move |s| s.bg(pal.toolbar_hover))
        .active(move |s| s.bg(pal.toolbar_pressed))
}

fn left_button(state: State, pal: Palette, markup: &'static str, tool: LeftTool, title: &'static str) -> View {
    rail_button_base(pal)
        .label(title)
        .test_id(match tool {
            LeftTool::Project => "rail-project",
            LeftTool::Commit => "rail-commit",
            LeftTool::Structure => "rail-structure",
            LeftTool::Bookmarks => "rail-bookmarks",
            LeftTool::Search => "rail-search",
        })
        .bg(move || {
            if state.left_tool.get() == tool && state.show_left.get() {
                pal.stripe_selected
            } else {
                transparent_black().into()
            }
        })
        .on_press(move || {
            if state.left_tool.get() == tool && state.show_left.get() {
                state.show_left.set(false);
            } else {
                state.left_tool.set(tool);
                state.show_left.set(true);
            }
            crate::settings::save(state);
        })
        .child(icon(markup, 20.0, pal.fg_muted).text_color(move || {
            if state.left_tool.get() == tool && state.show_left.get() { pal.fg_strong } else { pal.fg_muted }
        }))
}

fn bottom_button(state: State, pal: Palette, markup: &'static str, tool: BottomTool, title: &'static str) -> View {
    rail_button_base(pal)
        .label(title)
        .test_id(match tool {
            BottomTool::Cmake => "rail-cmake",
            BottomTool::Git => "rail-git",
            BottomTool::Hierarchy => "rail-hierarchy",
            BottomTool::Usages => "rail-usages",
            BottomTool::Debug => "rail-debug",
            BottomTool::Terminal => "rail-terminal",
            BottomTool::Problems => "rail-problems",
            BottomTool::Run => "rail-run",
        })
        .bg(move || {
            if state.bottom_tool.get() == Some(tool) && state.show_bottom.get() {
                pal.stripe_selected
            } else {
                transparent_black().into()
            }
        })
        .on_press(move || {
            if state.bottom_tool.get() == Some(tool) && state.show_bottom.get() {
                state.show_bottom.set(false);
            } else {
                state.bottom_tool.set(Some(tool));
                state.show_bottom.set(true);
            }
            crate::settings::save(state);
        })
        .child(icon(markup, 20.0, pal.fg_muted).text_color(move || {
            if state.bottom_tool.get() == Some(tool) && state.show_bottom.get() { pal.fg_strong } else { pal.fg_muted }
        }))
}

fn right_button(state: State, pal: Palette, markup: &'static str, tool: RightTool, title: &'static str) -> View {
    rail_button_base(pal)
        .label(title)
        .test_id(match tool {
            RightTool::Notifications => "rail-notifications",
        })
        .bg(move || {
            if state.right_tool.get() == tool && state.show_right.get() {
                pal.stripe_selected
            } else {
                transparent_black().into()
            }
        })
        .on_press(move || {
            if state.right_tool.get() == tool && state.show_right.get() {
                state.show_right.set(false);
            } else {
                state.right_tool.set(tool);
                state.show_right.set(true);
            }
            crate::settings::save(state);
        })
        .child(icon(markup, 20.0, pal.fg_muted).text_color(move || {
            if state.right_tool.get() == tool && state.show_right.get() { pal.fg_strong } else { pal.fg_muted }
        }))
}
