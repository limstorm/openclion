//! The workbench layout: activity strip, docks, editor tabs, status bar and the palette.
//!
//! Composition mirrors the web shell: one strip on the left, one dock per side, a centre column
//! with the tab strip over the editor group, and the status bar at the bottom.

use iced::widget::{Space, button, column, container, row, scrollable, stack, text, text_input};
use iced::{Alignment, Border, Element, Length};

use crate::app::{App, Message};
use crate::registry::{Command, Dock, TOOL_WINDOWS};

use crate::{i18n, tools};

pub fn view(app: &App) -> Element<'_, Message> {
    let base = row![strip(app), left_dock(app), centre(app), right_dock(app)];

    if app.palette.is_some() { stack![base, palette(app)].into() } else { base.into() }
}

/// The activity bar: every tool window, grouped by dock, the active one highlighted.
fn strip(app: &App) -> Element<'_, Message> {
    let tokens = app.tokens();
    let mut buttons = column![].spacing(2);

    for dock in [Dock::Left, Dock::Bottom, Dock::Right] {
        for tool in TOOL_WINDOWS.iter().filter(|tool| tool.dock == dock) {
            let active = is_active(app, tool.id);
            buttons = buttons.push(
                button(text(tool.icon).size(15))
                    .padding([5, 8])
                    .style(move |_theme, _status| button::Style {
                        background: active.then(|| tokens.accent_muted.into()),
                        text_color: if active { tokens.accent } else { tokens.muted },
                        ..button::Style::default()
                    })
                    .on_press(Message::ShowTool(tool.id)),
            );
        }
        buttons = buttons.push(Space::new().height(6));
    }

    container(buttons)
        .padding(4)
        .width(Length::Fixed(40.0))
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(tokens.panel.into()),
            border: tokens.border_style(),
            ..container::Style::default()
        })
        .into()
}

fn is_active(app: &App, id: &str) -> bool {
    app.left == Some(id) || app.right == Some(id) || app.bottom == Some(id)
}

fn left_dock(app: &App) -> Element<'_, Message> {
    dock(app, Dock::Left, 260.0)
}

fn right_dock(app: &App) -> Element<'_, Message> {
    dock(app, Dock::Right, 280.0)
}

fn dock(app: &App, dock: Dock, width: f32) -> Element<'_, Message> {
    let tokens = app.tokens();
    let active = match dock {
        Dock::Left => app.left,
        Dock::Right => app.right,
        Dock::Bottom => app.bottom,
    };
    let Some(id) = active else {
        return Space::new().width(0).into();
    };

    let titles = App::dock_titles(dock, app.language());
    let tabs = tools::dock_tabs(tokens, &titles, titles.iter().position(|(tool_id, _)| *tool_id == id).unwrap_or(0));

    let header = row![
        tabs,
        button(text("×").size(13).color(tokens.muted))
            .padding([0, 6])
            .style(|_theme, _status| button::Style { background: None, ..button::Style::default() })
            .on_press(Message::HideTool(dock)),
    ]
    .align_y(Alignment::Center);

    let body = tool_body(app, id);

    let panel = column![header, body].height(Length::Fill);

    if dock == Dock::Bottom {
        container(panel)
            .height(Length::Fixed(200.0))
            .width(Length::Fill)
            .style(move |_theme| container::Style {
                background: Some(tokens.panel.into()),
                border: tokens.border_style(),
                ..container::Style::default()
            })
            .into()
    } else {
        container(panel)
            .width(Length::Fixed(width))
            .height(Length::Fill)
            .style(move |_theme| container::Style {
                background: Some(tokens.panel.into()),
                border: tokens.border_style(),
                ..container::Style::default()
            })
            .into()
    }
}

fn tool_body<'a>(app: &'a App, id: &'static str) -> Element<'a, Message> {
    let tokens = app.tokens();
    match id {
        "project" => tools::project_panel(app),
        "search" => tools::search_panel(tokens),
        "git" | "commit" => tools::git_panel(tokens),
        "problems" | "inspection" | "duplicates" => tools::problems_panel(tokens),
        "terminal" => tools::text_panel(tokens, "shell — (M1: ide-terminal + 自研网格)"),
        "run" => tools::text_panel(tokens, "run — (M1: 运行配置与 DAP 控制台)"),
        "structure" => tools::text_panel(tokens, "structure — (M1: ide-lsp outline)"),
        "todo" => tools::text_panel(tokens, "todo — (M1: 扫描 TODO/FIXME)"),
        "cmake" => tools::text_panel(tokens, "cmake — (M1: ide-cmake 目标树)"),
        "build" => tools::text_panel(tokens, "build — (M1: ide-build 输出)"),
        "database" => tools::text_panel(tokens, "database — (M1: 表/查询)"),
        other => tools::text_panel(tokens, other),
    }
}

/// Tab strip over the editor group, plus the bottom dock and the status bar.
fn centre(app: &App) -> Element<'_, Message> {
    column![
        tabs(app),
        editor_area(app),
        if app.bottom.is_some() { dock(app, Dock::Bottom, 0.0) } else { Space::new().height(0).into() },
        status(app),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn tabs(app: &App) -> Element<'_, Message> {
    let tokens = app.tokens();
    let mut tabs = row![].spacing(2).align_y(Alignment::Center);

    for (index, tab) in app.tabs.iter().enumerate() {
        let active = index == app.active_tab;
        tabs = tabs.push(
            button(
                row![
                    text(tab.title.clone()).size(12).color(if active { tokens.text } else { tokens.muted }),
                    text("×").size(11).color(tokens.muted),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .padding([4, 10])
            .style(move |_theme, _status| button::Style {
                background: Some(if active { tokens.background } else { tokens.panel }.into()),
                border: Border {
                    color: if active { tokens.accent } else { tokens.border },
                    width: if active { 1.5 } else { 1.0 },
                    radius: 2.0.into(),
                },
                text_color: tokens.text,
                ..button::Style::default()
            })
            .on_press(Message::ActivateTab(index)),
        );
    }

    container(tabs)
        .padding([0, 4])
        .height(Length::Fixed(30.0))
        .width(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(tokens.panel.into()),
            border: tokens.border_style(),
            ..container::Style::default()
        })
        .into()
}

fn editor_area(app: &App) -> Element<'_, Message> {
    let tokens = app.tokens();
    let Some(tab) = app.tabs.get(app.active_tab) else {
        return container(text(i18n::t(app.language(), "workbench.welcome")).size(14).color(tokens.muted))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(24)
            .into();
    };

    let palette = iced_editor::Palette {
        background: tokens.background,
        text: tokens.text,
        selection: tokens.accent_muted,
        caret: tokens.text,
        preedit: tokens.warning,
        preedit_underline: tokens.warning,
    };

    container(iced_editor::Editor::new(tab.editor.clone(), app.editor_id()).palette(palette))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(tokens.background.into()),
            ..container::Style::default()
        })
        .into()
}

fn status(app: &App) -> Element<'_, Message> {
    let tokens = app.tokens();
    let language = app.language();

    let (line, column) = app
        .tabs
        .get(app.active_tab)
        .map(|tab| {
            let state = tab.editor.borrow();
            (state.line() + 1, state.column() + 1)
        })
        .unwrap_or((0, 0));

    let mut bar = row![].spacing(2).align_y(Alignment::Center);
    for label in ["main".to_owned(), format!("{line}:{column}"), "UTF-8".to_owned()] {
        bar = bar.push(container(text(label).size(11).color(tokens.muted)).padding([0, 6]));
    }
    bar = bar.push(Space::new().width(Length::Fill));
    bar = bar.push(
        button(
            text(format!("{} · {}", if app.light { "Light" } else { "Dark" }, i18n::t(language, "status.ready")))
                .size(11),
        )
        .padding([0, 6])
        .style(|_theme, _status| button::Style {
            background: None,
            text_color: tokens.muted,
            ..button::Style::default()
        })
        .on_press(Message::ToggleTheme),
    );
    bar = bar.push(
        button(text(if language == i18n::Language::Zh { "中文" } else { "EN" }).size(11))
            .padding([0, 6])
            .style(|_theme, _status| button::Style {
                background: None,
                text_color: tokens.muted,
                ..button::Style::default()
            })
            .on_press(Message::ToggleLanguage),
    );
    bar = bar.push(
        button(text("Ctrl+Shift+P").size(11))
            .padding([0, 6])
            .style(|_theme, _status| button::Style {
                background: None,
                text_color: tokens.muted,
                ..button::Style::default()
            })
            .on_press(Message::TogglePalette),
    );

    container(bar)
        .padding([2, 4])
        .height(Length::Fixed(24.0))
        .width(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(tokens.panel.into()),
            border: tokens.border_style(),
            ..container::Style::default()
        })
        .into()
}

fn palette(app: &App) -> Element<'_, Message> {
    let tokens = app.tokens();
    let filter = app.palette.clone().unwrap_or_default();

    let mut results = column![].spacing(1);
    for id in app.matching_commands(&filter) {
        let Some(command) = Command::by_id(id) else { continue };
        results = results.push(
            button(text(command.title).size(13).color(tokens.text))
                .width(Length::Fill)
                .padding([4, 8])
                .style(|_theme, _status| button::Style {
                    background: None,
                    text_color: tokens.text,
                    ..button::Style::default()
                })
                .on_press(Message::RunCommand(command.id)),
        );
    }

    let panel = container(column![
        row![
            text_input(i18n::t(app.language(), "palette.placeholder"), &filter)
                .id(app.palette_input.clone())
                .on_input(Message::PaletteInput)
                .on_submit(Message::PaletteSubmit)
                .padding(7)
                .size(14)
                .width(Length::Fill),
            // iced swallows Enter in the focused input and does not raise `on_submit` from
            // inside this overlay, so the palette also has an explicit submit control.
            button(text("⏎").size(14))
                .padding([6, 10])
                .on_press(Message::PaletteSubmit)
                .style(|_theme, _status| button::Style { background: None, ..button::Style::default() }),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
        scrollable(results).height(Length::Fixed(280.0)),
    ])
    .padding(8)
    .width(Length::Fixed(560.0))
    .style(move |_theme| container::Style {
        background: Some(tokens.panel.into()),
        border: Border { color: tokens.accent, width: 1.0, radius: 4.0.into() },
        ..container::Style::default()
    });

    container(panel)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(110)
        .align_x(Alignment::Center)
        .align_y(Alignment::Start)
        .into()
}
