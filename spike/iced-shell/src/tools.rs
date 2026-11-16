//! Content for each tool window. The real ones call into `ide-*` crates; these render the same
//! shape (tree, list, table) from sample data so the chrome can be exercised before the
//! backends are wired.

use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Element, Length};

use crate::app::Message;
use crate::tokens::Tokens;

/// A row of the project tree: depth, label, and whether it is a directory.
pub struct Row {
    pub depth: usize,
    pub label: &'static str,
    pub directory: bool,
}

pub fn project_rows() -> Vec<Row> {
    vec![
        Row { depth: 0, label: "kwbasedb", directory: true },
        Row { depth: 1, label: ".github", directory: true },
        Row { depth: 1, label: "build", directory: true },
        Row { depth: 1, label: "cmake", directory: true },
        Row { depth: 1, label: "docs", directory: true },
        Row { depth: 1, label: "pkg", directory: true },
        Row { depth: 2, label: "sql", directory: true },
        Row { depth: 1, label: "scripts", directory: true },
        Row { depth: 1, label: "src", directory: true },
        Row { depth: 2, label: "api", directory: true },
        Row { depth: 2, label: "ccl", directory: true },
        Row { depth: 2, label: "keys.go", directory: false },
        Row { depth: 2, label: "main.go", directory: false },
        Row { depth: 1, label: "tests", directory: true },
        Row { depth: 2, label: "smoke_test.go", directory: false },
        Row { depth: 1, label: "CMakeLists.txt", directory: false },
        Row { depth: 1, label: "README.md", directory: false },
        Row { depth: 1, label: "go.mod", directory: false },
    ]
}

pub fn project_panel<'a>(tokens: &'a Tokens, active: Option<&str>) -> Element<'a, Message> {
    let mut list = column![].spacing(1);
    for row in project_rows() {
        let selected = active == Some(row.label);
        let icon = if row.directory { "▸" } else { "·" };
        let label = row.label;

        list = list.push(
            button(
                row![
                    Space::new().width(row.depth as f32 * 12.0),
                    text(icon).size(11).color(tokens.muted),
                    text(label).size(12).color(if selected { tokens.text } else { tokens.muted }),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
            )
            .padding([2, 6])
            .width(Length::Fill)
            .style(move |_theme, _status| button::Style {
                background: selected.then(|| tokens.accent_muted.into()),
                text_color: tokens.text,
                ..button::Style::default()
            })
            .on_press(Message::OpenFile(label)),
        );
    }

    scrollable(list).height(Length::Fill).into()
}

pub fn search_panel<'a>(tokens: &'a Tokens) -> Element<'a, Message> {
    let hits = [
        ("keys.go", "42", "func (k Key) Decode()"),
        ("main.go", "17", "cfg.Store = store"),
        ("smoke_test.go", "88", "t.Run(\"smoke\""),
    ];
    let mut list = column![].spacing(2);
    for (file, line, content) in hits {
        list = list.push(
            column![
                row![text(file).size(12).color(tokens.text), text(line).size(11).color(tokens.muted)].spacing(8),
                text(content).size(11).color(tokens.muted),
            ]
            .spacing(1)
            .padding([3, 6]),
        );
    }
    scrollable(list).height(Length::Fill).into()
}

pub fn git_panel<'a>(tokens: &'a Tokens) -> Element<'a, Message> {
    let log = [
        ("1423af4", "fix(cef): the X on the last window quits"),
        ("6a85a41", "feat(app): closing the project quits"),
        ("ee9ffe1", "build(cef): strip by default"),
        ("ab934fa", "fix(cef): give the window a WM_CLASS"),
    ];
    let mut list = column![].spacing(2);
    for (hash, subject) in log {
        list = list.push(
            row![text(hash).size(11).color(tokens.accent), text(subject).size(12).color(tokens.muted)]
                .spacing(8)
                .padding([3, 6]),
        );
    }
    scrollable(list).height(Length::Fill).into()
}

pub fn problems_panel<'a>(tokens: &'a Tokens) -> Element<'a, Message> {
    let problems = [
        (tokens.warning, "keys.go:42", "unused parameter `opts`"),
        (tokens.error, "main.go:17", "cannot use cfg.Store (variable of type *Store)"),
        (tokens.warning, "smoke_test.go:88", "missing t.Parallel()"),
    ];
    let mut list = column![].spacing(2);
    for (color, location, message) in problems {
        list = list.push(
            row![
                text("●").size(10).color(color),
                text(location).size(12).color(tokens.text),
                text(message).size(12).color(tokens.muted)
            ]
            .spacing(8)
            .padding([3, 6]),
        );
    }
    scrollable(list).height(Length::Fill).into()
}

pub fn text_panel<'a>(tokens: &'a Tokens, label: &'a str) -> Element<'a, Message> {
    container(text(label).size(12).color(tokens.muted)).padding(10).width(Length::Fill).height(Length::Fill).into()
}

/// A tab strip inside a dock, for the tools that share a dock in the web workbench.
pub fn dock_tabs<'a>(tokens: &'a Tokens, tabs: &[(&'static str, &'a str)], active: usize) -> Element<'a, Message> {
    let mut row_of_tabs = row![].spacing(1);
    for (index, (id, title)) in tabs.iter().enumerate() {
        let selected = index == active;
        row_of_tabs = row_of_tabs.push(
            button(text(*title).size(11))
                .padding([3, 8])
                .style(move |_theme, _status| button::Style {
                    background: selected.then(|| tokens.accent_muted.into()),
                    text_color: if selected { tokens.text } else { tokens.muted },
                    ..button::Style::default()
                })
                .on_press(Message::ShowTool(id)),
        );
    }
    container(row_of_tabs).width(Length::Fill).into()
}
