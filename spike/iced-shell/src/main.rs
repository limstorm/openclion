//! M1 slice: the native workbench shell.
//!
//! Everything here mirrors a piece of the web frontend: the registries in `registry`, the
//! palette in `tokens`, the strings in `i18n`, and the layout in `workbench`. The text area is
//! the reusable widget from `../iced-editor`.

mod app;
mod fs;
mod i18n;
mod icons;
mod metrics;
mod registry;
mod tokens;
mod tools;
mod workbench;

use iced::{Element, Subscription, Task, Theme};

pub use app::{App, Message};

/// The same fonts the web build ships (`@fontsource` Inter and JetBrains Mono, converted from
/// the woff2 files to ttf): a different UI font would move every glyph and break pixel parity.
const INTER: &[u8] = include_bytes!("../assets/fonts/Inter.ttf");
const JETBRAINS_MONO: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono.ttf");

/// `--font-ui: "Inter Variable", Inter, …` at `--fs-ui: 13px`.
fn ui_font() -> iced::Font {
    iced::Font { family: iced::font::Family::Name("Inter"), ..iced::Font::default() }
}

/// `--font-mono: "JetBrains Mono Variable", …` at `--fs-code: 13px`.
pub fn code_font() -> iced::Font {
    iced::Font { family: iced::font::Family::Name("JetBrains Mono"), ..iced::Font::default() }
}

fn main() -> iced::Result {
    iced::application(App::boot, App::update, App::view)
        .font(INTER)
        .font(JETBRAINS_MONO)
        .default_font(ui_font())
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
