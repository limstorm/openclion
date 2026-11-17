//! The workbench layout: activity strip, docks, editor tabs, status bar and the palette.
//!
//! Composition mirrors the web shell: one strip on the left, one dock per side, a centre column
//! with the tab strip over the editor group, and the status bar at the bottom.

use iced::widget::svg::{Handle, Svg};
use iced::widget::{Space, button, column, container, row, scrollable, stack, text, text_input};
use iced::{Alignment, Border, Color, Element, Length};

use crate::app::{App, Message};
use crate::metrics::*;
use crate::registry::{Command, Dock, TOOL_WINDOWS};
use crate::tokens::Tokens;
use crate::{i18n, icons, project, textfit, tools};

pub fn view(app: &App) -> Element<'_, Message> {
    // `--island-gap: 6px` between every island; the stripe is transparent on the frame and
    // carries the gap itself (`--stripe-pad: var(--island-gap)`).
    let base =
        column![title_bar(app), row![strip(app), left_dock(app), splitter(app), centre(app), gap(), right_dock(app),],];

    // The web's `.framed` paints `--bg-frame` under a radial wash of the project colour at 22%
    // from the top-left corner. iced has no radial gradient, so the same tint is laid down as
    // a steep linear gradient from that corner.
    let framed = framed(app, base);

    if app.palette.is_some() {
        stack![framed, palette(app)].into()
    } else if app.menu {
        stack![framed, main_menu(app)].into()
    } else {
        framed
    }
}

/// `--h-titlebar: 40px`, painted in `--bg-frame`: the web shell has its own title bar and the
/// native window has none, so drawing it is what makes the two line up.
/// The web's `TitleBar`: 40px, `padding: 0 8px 0 10px`, `gap: 4px`, carrying the main menu
/// button, the project widget, the toolbar's right side, the gear and the caption buttons.
fn title_bar<'a>(app: &'a App) -> Element<'a, Message> {
    let tokens = app.tokens();
    let light = app.light;

    let menu = icon_button(icons::main_menu(), 16.0, tokens, Message::ToggleMenu);

    // `.widget`: 30px tall, `padding: 0 8px`, `radius-popup` (8px), `gap: 6px`.
    let project: Element<'a, Message> = match &app.project {
        Some(project) => {
            let name = project.name.clone();
            let (start_color, _end) = project_colors(name.as_str());
            let avatar: Element<'a, Message> =
                container(text(project::initials(&name)).size(FONT_TINY).color(Color::WHITE))
                    .width(Length::Fixed(20.0))
                    .height(Length::Fixed(20.0))
                    .center_x(Length::Fixed(20.0))
                    .center_y(Length::Fixed(20.0))
                    .style(move |_theme| container::Style {
                        background: Some(start_color.into()),
                        border: Border { radius: RADIUS_SMALL.into(), ..Border::default() },
                        ..container::Style::default()
                    })
                    .into();

            button(
                row![
                    avatar,
                    text(name)
                        .size(FONT_UI)
                        .color(tokens.fg)
                        .font(iced::Font { weight: iced::font::Weight::Medium, ..iced::Font::default() }),
                    Svg::new(Handle::from_memory(icons::chevron(false, light))).width(12).height(12),
                ]
                .spacing(6)
                .align_y(Alignment::Center)
                .padding([0, 8]),
            )
            .height(Length::Fixed(30.0))
            .padding([0, 0])
            .style(move |_theme, status| button::Style {
                background: matches!(status, button::Status::Hovered).then(|| tokens.toolbar_hover.into()),
                border: Border { radius: 8.0.into(), ..Border::default() },
                ..button::Style::default()
            })
            .on_press(Message::ToggleMenu)
            .into()
        }
        None => Space::new().width(0).into(),
    };

    // `.tools`: 30x30 icon buttons, `margin-left: 12px`, `gap: 4px`.
    let gear = icon_button(icons::settings(light), 18.0, tokens, Message::ToggleMenu);

    // `.captions`: 46x40 each, flush with the right edge, `margin-left: 8px`.
    let minimize = caption_button(icons::caption(icons::Caption::Minimize), tokens, Message::Minimize);
    let maximize = caption_button(icons::caption(icons::Caption::Maximize), tokens, Message::Maximize);
    let close = caption_button(icons::caption(icons::Caption::Close), tokens, Message::CloseWindow);

    container(
        row![
            menu,
            project,
            Space::new().width(Length::Fill),
            container(gear).padding([0, 0]).width(Length::Fixed(30.0)),
            container(row![minimize, maximize, close]).padding([0, 8]),
        ]
        .spacing(4)
        .align_y(Alignment::Center)
        .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 10.0 }),
    )
    .height(Length::Fixed(TITLEBAR))
    .width(Length::Fill)
    .into()
}

/// A 16px SVG in a 30x30 hoverable square, the title bar's `.tool` / `.mainMenuButton`.
fn icon_button(bytes: &'static [u8], size: f32, tokens: &Tokens, message: Message) -> Element<'static, Message> {
    let tokens = *tokens;
    button(Svg::new(Handle::from_memory(bytes)).width(size).height(size))
        .width(Length::Fixed(30.0))
        .height(Length::Fixed(30.0))
        .padding([0, 0])
        .style(move |_theme, status| button::Style {
            background: matches!(status, button::Status::Hovered).then(|| tokens.toolbar_hover.into()),
            border: Border { radius: 8.0.into(), ..Border::default() },
            ..button::Style::default()
        })
        .on_press(message)
        .into()
}

/// `.caption`: 46px wide, full height, no radius; the close button tints red on hover.
fn caption_button(bytes: &'static [u8], tokens: &Tokens, message: Message) -> Element<'static, Message> {
    let tokens = *tokens;
    let close = matches!(message, Message::CloseWindow);
    button(Svg::new(Handle::from_memory(bytes)).width(16).height(16))
        .width(Length::Fixed(46.0))
        .height(Length::Fixed(TITLEBAR))
        .padding([0, 0])
        .style(move |_theme, status| button::Style {
            background: match status {
                button::Status::Hovered if close => Some(Color::from_rgb(0.898, 0.286, 0.302).into()),
                button::Status::Hovered => Some(tokens.toolbar_hover.into()),
                _ => None,
            },
            border: Border::default(),
            ..button::Style::default()
        })
        .on_press(message)
        .into()
}

/// `--project-color-N` (`RecentProject.ColorN`), identical in both themes.
fn project_colors(name: &str) -> (Color, Color) {
    const SLOTS: [(Color, Color); 9] = [
        (
            Color::from_rgb(224.0 / 255.0, 136.0 / 255.0, 85.0 / 255.0),
            Color::from_rgb(233.0 / 255.0, 128.0 / 255.0, 111.0 / 255.0),
        ),
        (
            Color::from_rgb(176.0 / 255.0, 139.0 / 255.0, 20.0 / 255.0),
            Color::from_rgb(187.0 / 255.0, 127.0 / 255.0, 25.0 / 255.0),
        ),
        (
            Color::from_rgb(161.0 / 255.0, 163.0 / 255.0, 89.0 / 255.0),
            Color::from_rgb(135.0 / 255.0, 170.0 / 255.0, 89.0 / 255.0),
        ),
        (
            Color::from_rgb(59.0 / 255.0, 146.0 / 255.0, 184.0 / 255.0),
            Color::from_rgb(97.0 / 255.0, 131.0 / 255.0, 236.0 / 255.0),
        ),
        (
            Color::from_rgb(53.0 / 255.0, 116.0 / 255.0, 240.0 / 255.0),
            Color::from_rgb(122.0 / 255.0, 100.0 / 255.0, 240.0 / 255.0),
        ),
        (
            Color::from_rgb(200.0 / 255.0, 77.0 / 255.0, 143.0 / 255.0),
            Color::from_rgb(169.0 / 255.0, 86.0 / 255.0, 207.0 / 255.0),
        ),
        (
            Color::from_rgb(149.0 / 255.0, 90.0 / 255.0, 224.0 / 255.0),
            Color::from_rgb(168.0 / 255.0, 77.0 / 255.0, 224.0 / 255.0),
        ),
        (
            Color::from_rgb(36.0 / 255.0, 163.0 / 255.0, 148.0 / 255.0),
            Color::from_rgb(39.0 / 255.0, 156.0 / 255.0, 205.0 / 255.0),
        ),
        (
            Color::from_rgb(95.0 / 255.0, 173.0 / 255.0, 101.0 / 255.0),
            Color::from_rgb(61.0 / 255.0, 150.0 / 255.0, 139.0 / 255.0),
        ),
    ];
    SLOTS[project::color_index(name) - 1]
}

/// `--island-gap`: the frame showing through between two islands.
fn gap() -> Element<'static, Message> {
    Space::new().width(Length::Fixed(ISLAND_GAP)).height(Length::Fill).into()
}

/// `.island`: a rounded `--bg-island` surface on the frame.
fn island<'a>(content: impl Into<Element<'a, Message>>, tokens: &Tokens) -> Element<'a, Message> {
    let tokens = *tokens;
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(tokens.island.into()),
            border: Border { radius: ISLAND_RADIUS.into(), ..Border::default() },
            ..container::Style::default()
        })
        .into()
}

/// `.framed`: `--bg-frame` with the project colour washing in from the top-left corner at 22%.
fn framed<'a>(app: &'a App, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    let tokens = app.tokens();
    let tint = app
        .project
        .as_ref()
        .map(|project| Color { a: 0.22, ..project_colors(project.name.as_str()).0 })
        .unwrap_or(Color::TRANSPARENT);

    iced::widget::stack![
        crate::wash::Wash { tint, frame: tokens.frame },
        container(content).width(Length::Fill).height(Length::Fill),
    ]
    .into()
}

/// The activity stripe, ported from `ToolWindows.module.css`: 44px wide (32px button + 6px
/// padding + 6px island gap), 32x32 buttons with `radius-popup`, an 18px icon, 2px apart, a
/// transparent background (it sits on the window frame) and `--bg-stripe-*` states.
fn strip(app: &App) -> Element<'_, Message> {
    let tokens = app.tokens();
    let light = app.light;
    let mut buttons = column![].spacing(2).padding([0, 0]);

    for tool in TOOL_WINDOWS {
        let active = is_active(app, tool.id);
        buttons = buttons.push(
            button(Svg::new(Handle::from_memory(icons::tool_window(tool.id, light))).width(18).height(18))
                .width(Length::Fixed(32.0))
                .height(Length::Fixed(32.0))
                .padding([0, 0])
                .style(move |_theme, status| {
                    let hovered = matches!(status, button::Status::Hovered);
                    button::Style {
                        background: if active {
                            Some(tokens.stripe_selected.into())
                        } else if hovered {
                            Some(tokens.stripe_hover.into())
                        } else {
                            None
                        },
                        border: Border { radius: 8.0.into(), ..Border::default() },
                        ..button::Style::default()
                    }
                })
                .on_press(Message::ShowTool(tool.id)),
        );
    }

    container(buttons)
        .padding(iced::Padding { top: 0.0, right: 6.0, bottom: 6.0, left: 6.0 })
        .width(Length::Fixed(STRIPE))
        .height(Length::Fill)
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

/// A tool window island: the 40px header (`padding: 0 6px 0 8px`, `gap: 6px`) with the
/// title button, its chevron and the two header actions, then the body.
fn dock(app: &App, dock: Dock, width: f32) -> Element<'_, Message> {
    let tokens = app.tokens();
    let light = app.light;
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

    // `.titleButton`: 24px tall, `gap: 4px`, `padding: 0 4px`, `radius-small`, hover background.
    let title_button = button(
        row![
            text(title)
                .size(FONT_UI)
                .color(tokens.fg)
                .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::default() }),
            Svg::new(Handle::from_memory(icons::chevron(true, light))).width(12).height(12),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding([0, 4])
    .height(Length::Fixed(24.0))
    .style(move |_theme, status| button::Style {
        background: matches!(status, button::Status::Hovered).then(|| tokens.toolbar_hover.into()),
        border: Border { radius: RADIUS_SMALL.into(), ..Border::default() },
        ..button::Style::default()
    });

    // `.headerActions`: 16px icons in small buttons, `gap: 2px`, revealed on hover.
    let shown = app.dock_hovered == Some(id);
    let actions: Element<'_, Message> = if shown {
        row![
            action_button(icons::tool_window_action(icons::ToolWindowAction::More, light), tokens, None),
            action_button(
                icons::tool_window_action(icons::ToolWindowAction::Hide, light),
                tokens,
                Some(Message::HideTool(dock)),
            ),
        ]
        .spacing(2)
        .into()
    } else {
        Space::new().width(0).into()
    };

    let header = container(
        row![title_button, Space::new().width(Length::Fill), actions]
            .spacing(6)
            .align_y(Alignment::Center)
            .padding(iced::Padding { top: 0.0, right: 6.0, bottom: 0.0, left: 8.0 }),
    )
    .height(Length::Fixed(HEADER))
    .width(Length::Fill);

    let panel = column![header, tool_body(app, id)].height(Length::Fill);

    let body: Element<'_, Message> = if dock == Dock::Bottom {
        container(panel)
            .height(Length::Fixed(200.0))
            .width(Length::Fill)
            .style(move |_theme| container::Style {
                background: Some(tokens.island.into()),
                border: Border { radius: ISLAND_RADIUS.into(), ..Border::default() },
                ..container::Style::default()
            })
            .into()
    } else {
        container(panel)
            .width(Length::Fixed(width))
            .height(Length::Fill)
            .style(move |_theme| container::Style {
                background: Some(tokens.island.into()),
                border: Border { radius: ISLAND_RADIUS.into(), ..Border::default() },
                ..container::Style::default()
            })
            .into()
    };

    // `mouse_area` gives the island's hover state, which reveals the header actions.
    let id_for_hover = id;
    iced::widget::mouse_area(body)
        .on_enter(Message::HoverDock(Some(id_for_hover)))
        .on_exit(Message::HoverDock(None))
        .into()
}

/// A small (16px) header action button.
fn action_button(bytes: &'static [u8], tokens: &Tokens, message: Option<Message>) -> Element<'static, Message> {
    let tokens = *tokens;
    let button = button(Svg::new(Handle::from_memory(bytes)).width(16).height(16))
        .padding([0, 0])
        .width(Length::Fixed(20.0))
        .height(Length::Fixed(20.0))
        .style(move |_theme, status| button::Style {
            background: matches!(status, button::Status::Hovered).then(|| tokens.toolbar_hover.into()),
            border: Border { radius: RADIUS_SMALL.into(), ..Border::default() },
            ..button::Style::default()
        });
    match message {
        Some(message) => button.on_press(message).into(),
        None => button.into(),
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
    let tokens = app.tokens();
    // The tab strip and the editor share one island; the hairline between them is the
    // `--border-editor-tabs` line the web paints.
    let editor = island(column![tabs(app), divider(tokens), editor_area(app)].height(Length::Fill), tokens);

    let mut column = column![editor].height(Length::Fill);

    if app.bottom.is_some() {
        column = column.push(Space::new().height(Length::Fixed(ISLAND_GAP)));
        column = column.push(container(dock(app, Dock::Bottom, 0.0)).height(Length::Fixed(200.0)).width(Length::Fill));
    }

    column.push(status(app)).width(Length::Fill).height(Length::Fill).into()
}

/// The editor tab strip, ported from `ui/Tabs.tsx` + `Tabs.module.css`: `.tabButton` is
/// `padding: 0 10px` with `gap: 6px`, `.tabLabel` caps at 260px with an ellipsis,
/// `.closeSlot` is 20px wide with `margin-right: 4px`, and the close button only shows on the
/// selected tab or under the pointer.
fn tabs(app: &App) -> Element<'_, Message> {
    let tokens = app.tokens();
    let mut tabs = row![].spacing(2).align_y(Alignment::Center);

    for (index, tab) in app.tabs.iter().enumerate() {
        let active = index == app.active_tab;
        let hovered = app.tab_hovered == Some(index);
        let label = textfit::ellipsize(&tab.title, 260.0, FONT_UI, iced::Font::default());

        // `.closeSlot`: a 20px box; the `×` appears on hover or when selected.
        let close: Element<'_, Message> = if active || hovered {
            button(text("×").size(FONT_SMALL).color(tokens.fg_muted))
                .padding([0, 0])
                .width(Length::Fixed(20.0))
                .style(|_theme, status| button::Style {
                    background: matches!(status, button::Status::Hovered)
                        .then(|| Color::from_rgba(0.0, 0.0, 0.0, 0.08).into()),
                    border: Border { radius: RADIUS_SMALL.into(), ..Border::default() },
                    ..button::Style::default()
                })
                .on_press(Message::CloseTab(index))
                .into()
        } else {
            Space::new().width(Length::Fixed(20.0)).into()
        };

        let tab_button = button(
            row![
                tools::file_tab_icon(tab.path.as_deref(), app.light),
                text(label).size(FONT_UI).color(if active { tokens.fg } else { tokens.fg_secondary }),
                close,
            ]
            .spacing(6)
            .align_y(Alignment::Center)
            .padding(iced::Padding { top: 0.0, right: 2.0, bottom: 0.0, left: 10.0 }),
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
        .on_press(Message::ActivateTab(index));

        tabs = tabs.push(
            iced::widget::mouse_area(tab_button)
                .on_enter(Message::HoverTab(Some(index)))
                .on_exit(Message::HoverTab(None)),
        );
    }

    // `.tabActions`: 16px icon buttons, `gap: 2px`, `padding: 0 6px 0 8px` — split right and
    // the tab's options menu, exactly the pair the web puts after the strip.
    tabs = tabs.push(
        container(
            row![
                action_button(icons::split(app.light), tokens, None),
                action_button(icons::tool_window_action(icons::ToolWindowAction::More, app.light), tokens, None),
            ]
            .spacing(2)
            .padding(iced::Padding { top: 0.0, right: 6.0, bottom: 0.0, left: 8.0 }),
        )
        .center_y(Length::Fill),
    );

    // The strip is part of the editor island, so it paints nothing itself. It occupies
    // `--h-tabbar` + 1: the web's strip is 39px tall with the 1px border under it.
    container(tabs).padding([5, 4]).height(Length::Fixed(TABBAR + 1.0)).width(Length::Fill).into()
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
        scrollbar: if tokens.fg == iced::Color::BLACK {
            iced::Color::from_rgba(0.0, 0.0, 0.0, 0.2)
        } else {
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.2)
        },
        gutter_border: tokens.border,
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

/// The ☰ menu: the top-level entries the web's `MainMenu` renders, in the same order as
/// `src-tauri/src/menu.rs` builds them. Rows are placeholders until each submenu is ported.
fn main_menu(app: &App) -> Element<'_, Message> {
    let tokens = app.tokens();
    const TITLES: [&str; 11] =
        ["File", "Edit", "View", "Navigate", "Code", "Refactor", "Build", "Run", "Tools", "Window", "Help"];

    let mut rows = column![].spacing(0);
    for title in TITLES {
        rows = rows.push(
            button(text(title).size(FONT_UI).color(tokens.fg).width(Length::Fill))
                .padding([0, 12])
                .height(Length::Fixed(28.0))
                .width(Length::Fill)
                .style(move |_theme, status| button::Style {
                    background: matches!(status, button::Status::Hovered).then(|| tokens.toolbar_hover.into()),
                    text_color: tokens.fg,
                    ..button::Style::default()
                }),
        );
    }

    let panel = container(rows).padding([4, 0]).width(Length::Fixed(220.0)).style(move |_theme| container::Style {
        background: Some(tokens.elev.into()),
        border: Border { color: tokens.border_strong, width: 1.0, radius: 8.0.into() },
        ..container::Style::default()
    });

    container(panel)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(iced::Padding { top: 44.0, right: 0.0, bottom: 0.0, left: 6.0 })
        .align_x(Alignment::Start)
        .align_y(Alignment::Start)
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
