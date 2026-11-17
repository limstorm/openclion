//! Content for each tool window. The real ones call into `ide-*` crates; these render the same
//! shape (tree, list, table) from sample data so the chrome can be exercised before the
//! backends are wired.

use iced::widget::svg::{Handle, Svg};
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Border, Element, Length};

use crate::app::Message;
use crate::tokens::Tokens;

/// One visible line of the tree: the entry, how deep it sits, and whether it is expanded.
pub struct TreeRow<'a> {
    pub entry: &'a crate::fs::Entry,
    pub depth: usize,
    pub open: bool,
}

/// Flattens the expanded part of the tree, in the order the web tree paints it.
pub fn visible_rows<'a>(app: &'a crate::app::App) -> Vec<TreeRow<'a>> {
    fn walk<'a>(app: &'a crate::app::App, dir: &std::path::Path, depth: usize, out: &mut Vec<TreeRow<'a>>) {
        let Some(children) = app.children.get(dir) else { return };
        for entry in children {
            let open = app.expanded.contains(&entry.path);
            out.push(TreeRow { entry, depth, open });
            if entry.directory && open {
                walk(app, &entry.path, depth + 1, out);
            }
        }
    }

    let mut rows = Vec::new();
    if let Some(project) = &app.project {
        walk(app, &project.root, 0, &mut rows);
    }
    rows
}

/// The icon a tab shows before its title: the file's own type icon.
pub fn file_tab_icon<'a>(path: Option<&std::path::Path>, light: bool) -> Element<'a, Message> {
    let bytes = match path.and_then(|path| path.file_name()).and_then(|name| name.to_str()) {
        Some(name) => crate::icons::file_icon(name, light),
        None => crate::icons::file_icon("welcome.md", light),
    };
    icon(bytes, 1.0)
}

/// One 16 px icon from the embedded SVG set.
fn icon(bytes: &'static [u8], opacity: f32) -> Element<'static, Message> {
    Svg::new(Handle::from_memory(bytes)).width(16).height(16).opacity(opacity).into()
}

/// `$HOME` abbreviated to `~`, the way the web tree renders a project path.
fn display_path(path: &std::path::Path) -> String {
    let text = path.to_string_lossy().into_owned();
    match std::env::var("HOME").ok().filter(|home| !home.is_empty()) {
        Some(home) if text.starts_with(&home) => format!("~{}", &text[home.len()..]),
        _ => text,
    }
}

pub fn project_panel(app: &crate::app::App) -> Element<'_, Message> {
    let tokens = app.tokens();
    let mut list = column![].spacing(0);

    // The tree toolbar the web shows under the tool window header: collapse, actions, settings.
    list = list.push(
        container(
            row![
                Space::new().width(crate::metrics::TREE_PADDING),
                text("⌄").size(crate::metrics::FONT_TINY).color(tokens.fg_muted),
                text("⋯").size(crate::metrics::FONT_SMALL).color(tokens.fg_muted),
                Space::new().width(Length::Fill),
                text("⛭").size(crate::metrics::FONT_SMALL).color(tokens.fg_muted),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .height(Length::Fixed(crate::metrics::FIELD))
        .width(Length::Fill),
    );

    // The web tree paints the project root as its first row.
    if let Some(project) = &app.project {
        list = list.push(
            container(
                row![
                    Space::new().width(crate::metrics::TREE_PADDING),
                    icon(crate::icons::chevron(true, app.light), 1.0),
                    icon(crate::icons::folder_icon(app.light), 1.0),
                    text(project.name.clone())
                        .size(crate::metrics::FONT_UI)
                        .color(tokens.fg)
                        .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::default() }),
                    text(display_path(&project.root))
                        .size(crate::metrics::FONT_SMALL)
                        .color(tokens.fg_muted)
                        .wrapping(iced::widget::text::Wrapping::None),
                ]
                .spacing(crate::metrics::TREE_GAP)
                .align_y(Alignment::Center),
            )
            .height(Length::Fixed(crate::metrics::ROW)),
        );
    }

    for row in visible_rows(app) {
        let entry = row.entry;
        let selected = app.selected.as_deref() == Some(entry.path.as_path());
        // Parity with the web tree: git-ignored entries are dimmed, excluded ones dim their icon.
        let light = app.light;
        let faded = entry.ignored || entry.excluded;
        let color = if selected {
            tokens.fg
        } else if faded {
            tokens.fg_muted
        } else {
            tokens.fg
        };
        // The web tints an excluded folder's whole row instead of dimming only its text.
        let background = if selected {
            Some(tokens.tree_selected)
        } else if entry.excluded {
            Some(tokens.excluded_row)
        } else {
            None
        };
        let icon_opacity = if entry.excluded {
            0.5
        } else if entry.ignored {
            0.7
        } else {
            1.0
        };
        let depth = row.depth as f32 * crate::metrics::TREE_INDENT;
        let label = entry.name.clone();
        let path = entry.path.clone();
        let directory = entry.directory;

        list = list.push(
            button(
                row![
                    Space::new().width(crate::metrics::TREE_PADDING + depth),
                    if entry.directory {
                        row![
                            icon(crate::icons::chevron(row.open, light), 1.0),
                            icon(crate::icons::folder_icon(light), icon_opacity),
                        ]
                        .spacing(2)
                        .align_y(Alignment::Center)
                        .into()
                    } else {
                        icon(crate::icons::file_icon(&entry.name, light), icon_opacity)
                    },
                    text(label).size(crate::metrics::FONT_UI).color(color),
                ]
                .spacing(crate::metrics::TREE_GAP)
                .align_y(Alignment::Center),
            )
            .padding([0, 0])
            .height(Length::Fixed(crate::metrics::ROW))
            .width(Length::Fill)
            .style(move |_theme, status| {
                let hovered = matches!(status, button::Status::Hovered);
                let fill = background.or(hovered.then_some(tokens.tree_hover));
                button::Style {
                    background: fill.map(Into::into),
                    border: Border { radius: crate::metrics::RADIUS_ROW.into(), ..Border::default() },
                    text_color: color,
                    ..button::Style::default()
                }
            })
            .on_press(if directory { Message::ToggleDir(path) } else { Message::OpenFile(path) }),
        );
    }

    scrollable(list).height(Length::Fill).style(quiet_scrollbar).into()
}

/// The web tree's scrollbar is an overlay: no rail, a thin scroller.
fn quiet_scrollbar(_theme: &iced::Theme, _status: iced::widget::scrollable::Status) -> iced::widget::scrollable::Style {
    use iced::widget::scrollable::{Rail, Scroller, Style};

    Style {
        container: iced::widget::container::Style::default(),
        gap: None,
        auto_scroll: iced::widget::scrollable::AutoScroll {
            background: iced::Background::Color(iced::Color::TRANSPARENT),
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            icon: iced::Color::TRANSPARENT,
        },
        vertical_rail: Rail {
            background: None,
            border: iced::Border::default(),
            scroller: Scroller {
                background: iced::Background::Color(iced::Color::TRANSPARENT),
                border: iced::Border::default(),
            },
        },
        horizontal_rail: Rail {
            background: None,
            border: iced::Border::default(),
            scroller: Scroller {
                background: iced::Background::Color(iced::Color::TRANSPARENT),
                border: iced::Border::default(),
            },
        },
    }
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
                row![text(file).size(12).color(tokens.fg), text(line).size(11).color(tokens.fg_muted)].spacing(8),
                text(content).size(11).color(tokens.fg_muted),
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
            row![text(hash).size(11).color(tokens.accent), text(subject).size(12).color(tokens.fg_muted)]
                .spacing(8)
                .padding([3, 6]),
        );
    }
    scrollable(list).height(Length::Fill).into()
}

pub fn problems_panel<'a>(tokens: &'a Tokens) -> Element<'a, Message> {
    let problems = [
        (tokens.warning, "keys.go:42", "unused parameter `opts`"),
        (tokens.danger, "main.go:17", "cannot use cfg.Store (variable of type *Store)"),
        (tokens.warning, "smoke_test.go:88", "missing t.Parallel()"),
    ];
    let mut list = column![].spacing(2);
    for (color, location, message) in problems {
        list = list.push(
            row![
                text("●").size(10).color(color),
                text(location).size(12).color(tokens.fg),
                text(message).size(12).color(tokens.fg_muted)
            ]
            .spacing(8)
            .padding([3, 6]),
        );
    }
    scrollable(list).height(Length::Fill).into()
}

pub fn text_panel<'a>(tokens: &'a Tokens, label: &'a str) -> Element<'a, Message> {
    container(text(label).size(crate::metrics::FONT_SMALL).color(tokens.fg_muted))
        .padding(10)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
