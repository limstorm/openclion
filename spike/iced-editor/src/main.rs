//! M2.0: a rope-backed editor widget drawn by our own `Widget` impl.
//!
//! Scope is deliberately small — caret, selection, typing, and the IME preedit string —
//! because those are the parts `text_editor` cannot give us. Glyph metrics are estimated
//! from the font size (0.6 em per ASCII cell, 1 em for wide characters) instead of queried
//! through `Paragraph::grapheme_position`; a real editor uses the latter, and that is the
//! one line in `caret_x` that would change.

mod editor;

use editor::{Editor, Shared};
use iced::widget::{column, container, row, text};
use iced::{Alignment, Element, Length, Task, Theme};

fn main() -> iced::Result {
    iced::application(App::boot, App::update, App::view)
        .title("iced editor spike")
        .theme(|_: &App| Theme::Dark)
        .window_size((1100.0, 760.0))
        .subscription(App::subscription)
        .run()
}

struct App {
    editor: Shared,
    editor_id: iced::advanced::widget::Id,
    /// Set while the widget has focus, so the window title can show it.
    focused: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Window(iced::window::Event),
}

impl App {
    fn boot() -> (Self, Task<Message>) {
        let sample = "\
// iced editor spike — 在这里输入，含中文（IBus）
//
// 光标：点击放置，Shift+方向键或拖拽选择
// 中文：切到拼音输入法，组合串会带下划线显示在光标处

#include <string>

struct Greeter {
    std::string name;

    void greet() const {
        std::println(\"hello {}\", name);
    }
};
";
        let mut state = editor::EditorState::new(sample);
        state.line_height = 20.0;
        (
            Self { editor: Shared::new(state), editor_id: iced::advanced::widget::Id::unique(), focused: false },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Window(iced::window::Event::Focused) => {
                self.focused = true;
            }
            Message::Window(iced::window::Event::Unfocused) => self.focused = false,
            Message::Window(_) => {}
        }
        Task::none()
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        iced::window::events().map(|(_id, event)| Message::Window(event))
    }

    fn view(&self) -> Element<'_, Message> {
        let editor = Editor::new(self.editor.clone(), self.editor_id.clone());

        let state = self.editor.borrow();
        let position = format!("line {} · col {} · {} chars", state.line() + 1, state.column() + 1, state.len_chars());
        let selection = state.selection().map_or_else(|| "none".to_owned(), |(a, b)| format!("{}–{}", a, b));
        drop(state);

        column![
            container(editor).height(Length::Fill).width(Length::Fill),
            container(
                row![
                    text("iced editor spike").size(12),
                    text(format!("selection {selection}")).size(12),
                    iced::widget::Space::new().width(Length::Fill),
                    text(position).size(12),
                    text("IME: 中文（IBus）").size(12),
                ]
                .spacing(16)
                .align_y(Alignment::Center)
            )
            .padding([4, 10])
            .height(Length::Fixed(26.0))
            .width(Length::Fill)
            .style(container::rounded_box),
        ]
        .into()
    }
}
