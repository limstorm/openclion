//! OpenCLion's native shell: the whole workbench is written in Rust with
//! nsh's retained native views. No WebView, no JavaScript engine.

mod backend;
mod debug;
mod icons;
mod lsp;
mod model;
mod terminal;
mod theme;
mod ui;

use nsh_core::desktop::{TitleBarOptions, TitleBarStyle, WindowAttributes};
use nsh_core::element::desktop::current_window;
use nsh_core::prelude::*;

use model::State;

const LIGHT_THEME: &str = include_str!("../assets/theme-light.json");
const DARK_THEME: &str = include_str!("../assets/theme-dark.json");

/// The app's UI set (`nsh.json` `"rust"`).
#[native_module("openclion/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>().assets(assets!());
}

/// The application's root component: the workbench, with the theme signal
/// re-drawing every colour binding when it changes.
#[component]
pub fn App() -> impl IntoElement {
    let state = State::new();
    configure_window();
    install_shortcuts(state);
    state.refresh_cmake();
    state.refresh_vcs();
    terminal::ensure_started(state);
    debug::init(state);
    lsp::start(state);
    effect(move || {
        let path = state.active_path.get();
        let seed = state.editor_seed.get();
        if path.as_os_str().is_empty() {
            lsp::close(state);
        } else {
            let language = model::language_of(&path);
            lsp::open(state, &path, &language, &seed);
        }
    });
    // The native editor and other nsh components follow the app's theme, not
    // only our own palette.

    view().flex_1().relative().size_full().overflow_hidden().child(dynamic(move || {
        let dark = state.dark.get();
        set_color_scheme(if dark { Appearance::Dark } else { Appearance::Light });
        let json = if dark { DARK_THEME } else { LIGHT_THEME };
        if let Ok(tokens) = nsh_base::tokens_of(Some(json)) {
            set_tokens(tokens);
        }
        set_gtk_dark(dark);
        let pal = theme::Palette::of(dark);
        ui::chrome(state, pal).into_element()
    }))
}

fn configure_window() {
    let mut bar = TitleBarOptions::default();
    bar.style = TitleBarStyle::Hidden;
    bar.height = Some(px(40.0));
    let mut attributes = WindowAttributes::default();
    attributes.title_bar = Some(bar);
    current_window().set(attributes);
}

fn install_shortcuts(state: State) {
    on_key("mod+p", move |_| {
        state.quick_query.set(String::new());
        state.quick_open.set(true);
    });
    on_key("F9", move |_| {
        crate::debug::toggle_breakpoint(state, state.cursor.get().0.saturating_sub(1));
    });
    on_key("mod+s", move |_| state.save_active());
    on_key("mod+w", move |_| {
        let index = state.active.get();
        state.close_tab(index);
    });
    on_key("mod+b", move |_| state.show_left.update(|open| *open = !*open));
    on_key("mod+j", move |_| state.show_bottom.update(|open| *open = !*open));
    on_key("mod+shift+f", move |_| {
        state.left_tool.set(model::LeftTool::Search);
        state.show_left.set(true);
    });
    on_key("mod+`", move |_| {
        state.bottom_tool.set(Some(model::BottomTool::Terminal));
        state.show_bottom.update(|open| *open = !*open);
    });
    on_key("escape", move |_| {
        state.quick_open.set(false);
        state.project_menu.set(false);
        state.settings.set(false);
        state.toast.set(None);
    });
}

#[cfg(target_os = "linux")]
fn set_gtk_dark(dark: bool) {
    spawn(async move {
        let _ = run_on_main_thread(move || {
            if !gtk4::is_initialized() {
                return;
            }
            if let Some(settings) = gtk4::Settings::default() {
                settings.set_gtk_application_prefer_dark_theme(dark);
                // The workbench draws its own title bar and window buttons.
                settings.set_gtk_decoration_layout(Some(":"));
            }
        })
        .await;
    });
}

#[cfg(not(target_os = "linux"))]
fn set_gtk_dark(_dark: bool) {}
