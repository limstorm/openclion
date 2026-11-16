//! The workbench's own SVG icons, embedded straight from `src/ui/expui` — the same files the
//! web shell renders, so the native tree gets the real icon set instead of glyph stand-ins.
//!
//! The set ships two variants per icon (`x.svg` for light, `x_dark.svg` for dark), which is why
//! every lookup takes the theme.

macro_rules! icons {
    ($($name:ident => $path:literal),* $(,)?) => {
        $(const $name: &[u8] = include_bytes!(concat!("../../../src/ui/expui/", $path));)*
    };
}

icons! {
    RUST_DARK => "fileTypes/rust_dark.svg",
    RUST_LIGHT => "fileTypes/rust.svg",
    C_DARK => "fileTypes/c_dark.svg",
    C_LIGHT => "fileTypes/c.svg",
    CPP_DARK => "fileTypes/cpp_dark.svg",
    CPP_LIGHT => "fileTypes/cpp.svg",
    JSON_DARK => "fileTypes/json_dark.svg",
    JSON_LIGHT => "fileTypes/json.svg",
    MARKDOWN_DARK => "fileTypes/markdown_dark.svg",
    MARKDOWN_LIGHT => "fileTypes/markdown.svg",
    TOML_DARK => "fileTypes/toml_dark.svg",
    TOML_LIGHT => "fileTypes/toml.svg",
    CONFIG_DARK => "fileTypes/config_dark.svg",
    CONFIG_LIGHT => "fileTypes/config.svg",
    SQL_DARK => "fileTypes/sql_dark.svg",
    SQL_LIGHT => "fileTypes/sql.svg",
    PYTHON_DARK => "fileTypes/python_dark.svg",
    PYTHON_LIGHT => "fileTypes/python.svg",
    SHELL_DARK => "fileTypes/shell_dark.svg",
    SHELL_LIGHT => "fileTypes/shell.svg",
    JAVASCRIPT_DARK => "fileTypes/javaScript_dark.svg",
    JAVASCRIPT_LIGHT => "fileTypes/javaScript.svg",
    HTML_DARK => "fileTypes/html_dark.svg",
    HTML_LIGHT => "fileTypes/html.svg",
    CSS_DARK => "fileTypes/css_dark.svg",
    CSS_LIGHT => "fileTypes/css.svg",
    TEXT_DARK => "fileTypes/text_dark.svg",
    TEXT_LIGHT => "fileTypes/text.svg",
    FOLDER_DARK => "nodes/folder_dark.svg",
    FOLDER_LIGHT => "nodes/folder.svg",
    CHEVRON_RIGHT_DARK => "general/chevronRight_dark.svg",
    CHEVRON_RIGHT_LIGHT => "general/chevronRight.svg",
    CHEVRON_DOWN_DARK => "general/chevronDown_dark.svg",
    CHEVRON_DOWN_LIGHT => "general/chevronDown.svg",
}

fn pick(dark: &'static [u8], light: &'static [u8], light_theme: bool) -> &'static [u8] {
    if light_theme { light } else { dark }
}

/// Icon for a file, chosen by its exact name first (the language definitions do the same) and
/// then by extension.
pub fn file_icon(name: &str, light_theme: bool) -> &'static [u8] {
    let lower = name.to_ascii_lowercase();
    let by_name = match lower.as_str() {
        "cmakelists.txt" | "cmakepresets.json" | "makefile" | "go.mod" | "go.sum" => {
            Some(pick(CONFIG_DARK, CONFIG_LIGHT, light_theme))
        }
        "cargo.toml" | "cargo.lock" => Some(pick(TOML_DARK, TOML_LIGHT, light_theme)),
        _ => None,
    };
    if let Some(icon) = by_name {
        return icon;
    }

    let extension = lower.rsplit_once('.').map(|(_, extension)| extension).unwrap_or_default();
    match extension {
        "rs" => pick(RUST_DARK, RUST_LIGHT, light_theme),
        "c" | "h" => pick(C_DARK, C_LIGHT, light_theme),
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => pick(CPP_DARK, CPP_LIGHT, light_theme),
        "json" => pick(JSON_DARK, JSON_LIGHT, light_theme),
        "md" | "markdown" => pick(MARKDOWN_DARK, MARKDOWN_LIGHT, light_theme),
        "toml" => pick(TOML_DARK, TOML_LIGHT, light_theme),
        "yaml" | "yml" | "ini" | "conf" | "cfg" => pick(CONFIG_DARK, CONFIG_LIGHT, light_theme),
        "sql" => pick(SQL_DARK, SQL_LIGHT, light_theme),
        "py" => pick(PYTHON_DARK, PYTHON_LIGHT, light_theme),
        "sh" | "bash" | "zsh" => pick(SHELL_DARK, SHELL_LIGHT, light_theme),
        "js" | "mjs" | "cjs" | "ts" | "tsx" | "jsx" => pick(JAVASCRIPT_DARK, JAVASCRIPT_LIGHT, light_theme),
        "html" | "htm" => pick(HTML_DARK, HTML_LIGHT, light_theme),
        "css" => pick(CSS_DARK, CSS_LIGHT, light_theme),
        _ => pick(TEXT_DARK, TEXT_LIGHT, light_theme),
    }
}

pub fn folder_icon(light_theme: bool) -> &'static [u8] {
    pick(FOLDER_DARK, FOLDER_LIGHT, light_theme)
}

pub fn chevron(open: bool, light_theme: bool) -> &'static [u8] {
    if open {
        pick(CHEVRON_DOWN_DARK, CHEVRON_DOWN_LIGHT, light_theme)
    } else {
        pick(CHEVRON_RIGHT_DARK, CHEVRON_RIGHT_LIGHT, light_theme)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_a_non_empty_svg() {
        for icon in [
            file_icon("a.rs", false),
            file_icon("b.cpp", true),
            file_icon("CMakeLists.txt", false),
            file_icon("unknown.zzz", false),
            folder_icon(false),
            chevron(true, false),
        ] {
            let text = std::str::from_utf8(icon).expect("svg is utf-8");
            assert!(text.contains("<svg"), "icon is not an svg");
        }
    }

    #[test]
    fn dark_and_light_variants_differ() {
        assert_ne!(file_icon("a.rs", false), file_icon("a.rs", true));
        assert_ne!(folder_icon(false), folder_icon(true));
    }
}
