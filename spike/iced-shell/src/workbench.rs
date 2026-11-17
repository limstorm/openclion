//! The workbench layout: activity strip, docks, editor tabs, status bar and the palette.
//!
//! Composition mirrors the web shell: one strip on the left, one dock per side, a centre column
//! with the tab strip over the editor group, and the status bar at the bottom.

use iced::widget::{Space, button, column, container, row, scrollable, stack, text, text_input};
use iced::{Alignment, Border, Element, Length};

use crate::app::{App, Message};
use crate::metrics::*;
use crate::registry::{Command, Dock, TOOL_WINDOWS};
use crate::tokens::Tokens;
use crate::{i18n, tools};

pub fn view(app: &App) -> Element<'_, Message> {
    let base = column![title_bar(app), row![strip(app), left_dock(app), splitter(app), centre(app), right_dock(app)]];

    if app.palette.is_some() { stack![base, palette(app)].into() } else { base.into() }
}

/// `--h-titlebar: 40px`, painted in `--bg-frame`: the web shell has its own title bar and the
/// native window has none, so drawing it is what makes the two line up.
fn title_bar(app: &App) -> Element<'_, Message> {
    let tokens = app.tokens();
    let project = app.project.as_ref().map(|project| project.name.clone());

    // The web title bar is `[gap 44][18px project badge][8px][project name][chevron] …`.
    let badge: Element<'_, Message> = match &project {
        Some(name) => {
            let initial = name.chars().next().unwrap_or('O').to_ascii_uppercase();
            container(text(initial.to_string()).size(FONT_TINY).color(tokens.app))
                .width(Length::Fixed(18.0))
                .height(Length::Fixed(18.0))
                .center_x(Length::Fixed(18.0))
                .center_y(Length::Fixed(18.0))
                .style(move |_theme| container::Style {
                    background: Some(tokens.success.into()),
                    border: Border { radius: 4.0.into(), ..Border::default() },
                    ..container::Style::default()
                })
                .into()
        }
        None => Space::new().width(0).into(),
    };

    let name: Element<'_, Message> = match &project {
        Some(name) => text(name.clone()).size(FONT_UI).color(tokens.fg).into(),
        None => Space::new().width(0).into(),
    };

    container(
        row![Space::new().width(STRIPE + 8.0), badge, name, text("⌄").size(FONT_TINY).color(tokens.fg_muted),]
            .spacing(8)
            .align_y(Alignment::Center),
    )
    .height(Length::Fixed(TITLEBAR))
    .width(Length::Fill)
    .style(move |_theme| container::Style { background: Some(tokens.frame.into()), ..container::Style::default() })
    .into()
}

/// The activity bar: every tool window, grouped by dock, the active one highlighted.
fn strip(app: &App) -> Element<'_, Message> {
    let tokens = app.tokens();
    let mut buttons = column![].spacing(2);

    for dock in [Dock::Left, Dock::Bottom, Dock::Right] {
        for tool in TOOL_WINDOWS.iter().filter(|tool| tool.dock == dock) {
            let active = is_active(app, tool.id);
            buttons = buttons.push(
                button(text(tool.icon).size(16))
                    .padding([0, 0])
                    .width(Length::Fixed(STRIPE))
                    .height(Length::Fixed(STRIPE_BUTTON))
                    .style(move |_theme, _status| button::Style {
                        background: active.then(|| tokens.selected.into()),
                        text_color: if active { tokens.accent } else { tokens.fg_muted },
                        ..button::Style::default()
                    })
                    .on_press(Message::ShowTool(tool.id)),
            );
        }
        buttons = buttons.push(Space::new().height(6));
    }

    container(buttons)
        .padding([0, 0])
        .width(Length::Fixed(STRIPE))
        .height(Length::Fill)
        .style(move |_theme| container::Style { background: Some(tokens.frame.into()), ..container::Style::default() })
        .into()
}

/// The 6px band the web puts between a tool window and the editor; dragging it resizes the
/// panel, exactly like the web's splitter.
fn splitter<'a>(app: &'a App) -> Element<'a, Message> {
    let tokens = app.tokens();
    if app.left.is_none() {
        return Space::new().width(0).into();
    }

    crate::splitter::Splitter::new(tokens.frame, tokens.selected, Message::ResizeDock).into()
}

fn is_active(app: &App, id: &str) -> bool {
    app.left == Some(id) || app.right == Some(id) || app.bottom == Some(id)
}

fn left_dock(app: &App) -> Element<'_, Message> {
    dock(app, Dock::Left, app.left_width)
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

    let title =
        i18n::t(app.language(), crate::registry::ToolWindow::by_id(id).map_or("tool.project", |tool| tool.title_key));

    // `--h-header: 40px`: one tool window per dock, its title and the actions affordance.
    let header = row![
        text(title).size(FONT_UI).color(tokens.fg),
        text("⌄").size(FONT_TINY).color(tokens.fg_muted),
        Space::new().width(Length::Fill),
        // The web reveals these on hover; until hover state exists they stay visible but muted.
        button(text("⋯").size(FONT_UI).color(tokens.fg_muted))
            .padding([0, 4])
            .style(|_theme, _status| button::Style { background: None, ..button::Style::default() }),
        button(text("×").size(FONT_UI).color(tokens.fg_muted))
            .padding([0, 4])
            .style(|_theme, _status| button::Style { background: None, ..button::Style::default() })
            .on_press(Message::HideTool(dock)),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .padding([0, 12]);

    let body = tool_body(app, id);

    let panel =
        column![container(header).height(Length::Fixed(HEADER)).align_y(Alignment::Center), body].height(Length::Fill);

    if dock == Dock::Bottom {
        container(panel)
            .height(Length::Fixed(200.0))
            .width(Length::Fill)
            .style(move |_theme| container::Style {
                background: Some(tokens.elev.into()),
                ..container::Style::default()
            })
            .into()
    } else {
        container(panel)
            .width(Length::Fixed(width))
            .height(Length::Fill)
            .style(move |_theme| container::Style {
                background: Some(tokens.elev.into()),
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
        divider(app.tokens()),
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
                    tools::file_tab_icon(tab.path.as_deref(), app.light),
                    text(tab.title.clone()).size(FONT_UI).color(if active { tokens.fg } else { tokens.fg_secondary }),
                    text("×").size(FONT_SMALL).color(tokens.fg_muted),
                ]
                .spacing(6)
                .align_y(Alignment::Center)
                .padding([0, 10]),
            )
            .padding([0, 0])
            .height(Length::Fixed(TAB))
            .style(move |_theme, status| {
                let hovered = matches!(status, button::Status::Hovered);
                let background = if active {
                    tokens.tab_selected
                } else if hovered {
                    tokens.tab_hover
                } else {
                    tokens.app
                };
                button::Style {
                    background: Some(background.into()),
                    border: Border {
                        color: if active { tokens.tab_selected_border } else { tokens.border },
                        width: 1.0,
                        radius: RADIUS_TAB.into(),
                    },
                    text_color: tokens.fg,
                    ..button::Style::default()
                }
            })
            .on_press(Message::ActivateTab(index)),
        );
    }

    // The web puts a "+" after the tabs (and the editor actions on the right edge).
    tabs = tabs.push(
        button(text("+").size(FONT_UI).color(tokens.fg_muted))
            .padding([0, 8])
            .height(Length::Fixed(TAB))
            .style(|_theme, _status| button::Style { background: None, ..button::Style::default() }),
    );

    // Measured from the web build: the tab strip occupies 39px and a 1px border follows it,
    // so the editor starts exactly 40px below the title bar.
    container(tabs)
        .padding([5, 4])
        .height(Length::Fixed(TABBAR + 1.0))
        .width(Length::Fill)
        .style(move |_theme| container::Style { background: Some(tokens.elev.into()), ..container::Style::default() })
        .into()
}

/// The web paints a 1px `--border-editor-tabs` under the strip; a separate row keeps the tab
/// bar itself at exactly `--h-tabbar` pixels instead of losing two to a box border.
fn divider<'a>(tokens: &Tokens) -> Element<'a, Message> {
    let color = tokens.border;
    container(Space::new().width(Length::Fill))
        .height(Length::Fixed(1.0))
        .width(Length::Fill)
        .style(move |_theme| container::Style { background: Some(color.into()), ..container::Style::default() })
        .into()
}

fn editor_area(app: &App) -> Element<'_, Message> {
    let tokens = app.tokens();
    let Some(tab) = app.tabs.get(app.active_tab) else {
        return container(text(i18n::t(app.language(), "workbench.welcome")).size(14).color(tokens.fg_muted))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(24)
            .into();
    };

    let palette = iced_editor::Palette {
        background: tokens.app,
        text: tokens.fg,
        selection: tokens.selected,
        caret: tokens.caret,
        preedit: tokens.warning,
        preedit_underline: tokens.warning,
        gutter_text: tokens.gutter_text,
        gutter_text_current: tokens.gutter_text_current,
        syntax: iced_editor::SyntaxColors {
            keyword: tokens.syn.keyword,
            string: tokens.syn.string,
            number: tokens.syn.number,
            comment: tokens.syn.comment,
            function: tokens.syn.function,
            ty: tokens.syn.ty,
            constant: tokens.syn.constant,
            punctuation: tokens.syn.punctuation,
            variable: tokens.syn.variable,
            other: tokens.fg,
        },
    };

    let code_font = iced::Font { family: iced::font::Family::Name("JetBrains Mono"), ..iced::Font::default() };

    container(iced_editor::Editor::new(tab.editor.clone(), app.editor_id()).palette(palette).font(code_font))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| container::Style { background: Some(tokens.app.into()), ..container::Style::default() })
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

    let mut segments: Vec<String> = Vec::new();
    if let Some(branch) = app.branch() {
        segments.push(branch);
    }
    segments.push(format!("{line}:{column}"));
    if let Some(selected) = app.selection_chars() {
        segments.push(format!("{selected} selected"));
    }
    if let Some(indent) = app.indentation() {
        segments.push(indent);
    }
    segments.push("UTF-8".to_owned());
    segments.push(app.active_language());

    let mut bar = row![].spacing(16).align_y(Alignment::Center);
    for segment in segments {
        bar = bar.push(text(segment).size(FONT_UI).color(tokens.fg_secondary));
    }
    bar = bar.push(Space::new().width(Length::Fill));
    bar = bar.push(
        button(
            text(format!("{} · {}", if app.light { "Light" } else { "Dark" }, i18n::t(language, "status.ready")))
                .size(FONT_UI),
        )
        .padding([0, 4])
        .style(|_theme, _status| button::Style {
            background: None,
            text_color: tokens.fg_secondary,
            ..button::Style::default()
        })
        .on_press(Message::ToggleTheme),
    );
    bar = bar.push(
        button(text(if language == i18n::Language::Zh { "中文" } else { "EN" }).size(FONT_UI))
            .padding([0, 4])
            .style(|_theme, _status| button::Style {
                background: None,
                text_color: tokens.fg_secondary,
                ..button::Style::default()
            })
            .on_press(Message::ToggleLanguage),
    );
    bar = bar.push(
        button(text("Ctrl+Shift+P").size(FONT_UI))
            .padding([0, 4])
            .style(|_theme, _status| button::Style {
                background: None,
                text_color: tokens.fg_secondary,
                ..button::Style::default()
            })
            .on_press(Message::TogglePalette),
    );

    container(bar)
        .padding([0, 12])
        .height(Length::Fixed(STATUSBAR))
        .width(Length::Fill)
        .style(move |_theme| container::Style { background: Some(tokens.frame.into()), ..container::Style::default() })
        .into()
}

fn palette(app: &App) -> Element<'_, Message> {
    let tokens = app.tokens();
    let filter = app.palette.clone().unwrap_or_default();

    let mut results = column![].spacing(1);
    for id in app.matching_commands(&filter) {
        let Some(command) = Command::by_id(id) else { continue };
        results = results.push(
            button(text(command.title).size(13).color(tokens.fg))
                .width(Length::Fill)
                .padding([4, 8])
                .style(|_theme, _status| button::Style {
                    background: None,
                    text_color: tokens.fg,
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
        background: Some(tokens.elev.into()),
        border: Border { color: tokens.accent, width: 1.0, radius: RADIUS_SMALL.into() },
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
