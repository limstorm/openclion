//! M1 slice: the native workbench shell.
//!
//! Everything here mirrors a piece of the web frontend: the registries in `registry`, the
//! palette in `tokens`, the strings in `i18n`, and the layout in `workbench`. The text area is
//! the reusable widget from `../iced-editor`.

mod app;
mod i18n;
mod registry;
mod tokens;
mod tools;
mod workbench;

use iced::{Element, Subscription, Task, Theme};

pub use app::{App, Message};

fn main() -> iced::Result {
    iced::application(App::boot, App::update, App::view)
        .title(App::title)
        .theme(|app: &App| if app.light { Theme::Light } else { Theme::Dark })
        .subscription(App::subscription)
        .window_size((1280.0, 820.0))
        .run()
}

impl App {
    pub fn view(&self) -> Element<'_, Message> {
        workbench::view(self)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        iced::keyboard::listen().map(Message::Key)
    }

    /// Exposed for `iced::application`, which wants a boot returning `(State, Task)`.
    pub fn boot_with_task() -> (Self, Task<Message>) {
        Self::boot()
    }
}
