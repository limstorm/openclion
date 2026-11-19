//! Inline SVG icons, vendored from OpenCLion's `src/ui/expui` set (JetBrains
//! expui, Apache-2.0) so the native shell draws the same glyphs as the web UI.

use nsh_core::element::ImageSrc;
use nsh_core::prelude::*;

macro_rules! icons {
    ($( $name:ident = $file:literal ),* $(,)?) => {
        $( pub const $name: &str = include_str!(concat!("../assets/icons/", $file)); )*
    };
}

icons! {
    PROJECT = "project.svg",
    COMMIT = "commit.svg",
    STRUCTURE = "structure.svg",
    BOOKMARKS = "bookmarks.svg",
    SERVICES = "services.svg",
    RUN = "run.svg",
    TERMINAL = "terminal.svg",
    PROBLEMS = "problems.svg",
    NOTIFICATIONS = "notifications.svg",
    DATABASE = "database.svg",
    AGENTS = "agents.svg",
    CMAKE = "cmake.svg",
    CHEVRON_DOWN = "chevron-down.svg",
    CHEVRON_RIGHT = "chevron-right.svg",
    CHEVRON_UP = "chevron-up.svg",
    CLOSE = "close.svg",
    ADD = "add.svg",
    MORE_HORIZONTAL = "more-horizontal.svg",
    MORE_VERTICAL = "more-vertical.svg",
    SEARCH = "search.svg",
    SETTINGS = "settings.svg",
    REFRESH = "refresh.svg",
    COLLAPSE_ALL = "collapse-all.svg",
    SPLIT = "split.svg",
    FOLDER = "folder.svg",
    FOLDER_SOURCE = "folder-source.svg",
    FOLDER_TEST = "folder-test.svg",
    FILE_CPP = "file-cpp.svg",
    FILE_C = "file-c.svg",
    FILE_H = "file-h.svg",
    FILE_MD = "file-md.svg",
    FILE_TEXT = "file-text.svg",
    FILE_JSON = "file-json.svg",
    FILE_RUST = "file-rust.svg",
    FILE_CONFIG = "file-config.svg",
    FILE_CMAKE = "file-cmake.svg",
    FILE_README = "file-readme.svg",
    RUN_COLORED = "run-colored.svg",
    DEBUG_COLORED = "debug-colored.svg",
}

/// Draws `markup` at `size` points, in the surrounding text colour where the
/// icon uses `currentColor`.
pub fn icon(markup: &'static str, size: f32) -> Svg {
    svg(ImageSrc::svg_markup(markup)).w(px(size)).h(px(size)).flex_shrink_0()
}

/// A file-type icon, by extension (the mappings of `src/ui/icons.tsx`).
pub fn file_icon(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    if lower.starts_with("readme") {
        return FILE_README;
    }
    if lower == ".gitignore" || lower == "cmakelists.txt" || lower == "cmakecache.txt" {
        return match lower.as_str() {
            "cmakelists.txt" | "cmakecache.txt" => FILE_CMAKE,
            _ => FILE_CONFIG,
        };
    }
    match ext {
        "c" => FILE_C,
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => FILE_CPP,
        "h" => FILE_H,
        "md" | "markdown" => FILE_MD,
        "json" | "jsonc" => FILE_JSON,
        "rs" => FILE_RUST,
        "toml" | "yaml" | "yml" | "ini" | "cfg" | "conf" | "txt" => FILE_CONFIG,
        "cmake" => FILE_CMAKE,
        "cpp.inc" => FILE_CPP,
        _ => FILE_TEXT,
    }
}
