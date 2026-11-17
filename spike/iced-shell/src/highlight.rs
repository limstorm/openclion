//! Syntax highlighting for the languages the sample project uses.
//!
//! The web editor highlights with CodeMirror's Lezer grammars, which has no Rust counterpart;
//! the closest equivalent is the tree-sitter grammar's own `queries/highlights.scm`, which is
//! what this module compiles. Capture names are mapped onto the editor's ten classes, and the
//! colours come from the same `--syn-*` palette, so the result reads the same even though the
//! token boundaries can differ by a character or two from Lezer's.

use std::sync::OnceLock;

use iced_editor::{Highlight, SyntaxClass};
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

struct Language {
    language: tree_sitter::Language,
    query: Query,
}

fn build(name: &str, language: tree_sitter::Language, source: &str) -> Option<Language> {
    let query = Query::new(&language, source).ok()?;
    let _ = name;
    Some(Language { language, query })
}

fn language_for(id: &str) -> Option<&'static Language> {
    static GO: OnceLock<Option<Language>> = OnceLock::new();
    static C: OnceLock<Option<Language>> = OnceLock::new();
    static CPP: OnceLock<Option<Language>> = OnceLock::new();
    static RUST: OnceLock<Option<Language>> = OnceLock::new();
    static PYTHON: OnceLock<Option<Language>> = OnceLock::new();
    static JAVASCRIPT: OnceLock<Option<Language>> = OnceLock::new();

    match id {
        "go" => GO
            .get_or_init(|| build("go", tree_sitter_go::LANGUAGE.into(), include_str!("../assets/queries/go.scm")))
            .as_ref(),
        "c" => C
            .get_or_init(|| build("c", tree_sitter_c::LANGUAGE.into(), include_str!("../assets/queries/c.scm")))
            .as_ref(),
        "cpp" => CPP
            .get_or_init(|| build("cpp", tree_sitter_cpp::LANGUAGE.into(), include_str!("../assets/queries/cpp.scm")))
            .as_ref(),
        "rust" => RUST
            .get_or_init(|| {
                build("rust", tree_sitter_rust::LANGUAGE.into(), include_str!("../assets/queries/rust.scm"))
            })
            .as_ref(),
        "python" => PYTHON
            .get_or_init(|| {
                build("python", tree_sitter_python::LANGUAGE.into(), include_str!("../assets/queries/python.scm"))
            })
            .as_ref(),
        "javascript" | "typescript" | "tsx" | "jsx" => JAVASCRIPT
            .get_or_init(|| {
                build(
                    "javascript",
                    tree_sitter_javascript::LANGUAGE.into(),
                    include_str!("../assets/queries/javascript.scm"),
                )
            })
            .as_ref(),
        _ => None,
    }
}

fn class_of(capture: &str) -> SyntaxClass {
    let base = capture.split('.').next().unwrap_or(capture);
    match base {
        "keyword" => SyntaxClass::Keyword,
        "string" | "escape" | "regex" => SyntaxClass::String,
        "comment" => SyntaxClass::Comment,
        "number" | "float" | "integer" => SyntaxClass::Number,
        "constant" => SyntaxClass::Constant,
        "function" | "method" | "constructor" => SyntaxClass::Function,
        "type" | "class" | "struct" | "enum" | "interface" | "trait" | "namespace" | "module" => SyntaxClass::Type,
        "operator" | "punctuation" => SyntaxClass::Punctuation,
        "variable" | "property" | "field" | "parameter" => SyntaxClass::Variable,
        _ => SyntaxClass::Other,
    }
}

/// Highlight spans for `text`, in `char` offsets. Empty when the language has no grammar here.
pub fn spans(language_id: &str, text: &str) -> Vec<Highlight> {
    let Some(entry) = language_for(language_id) else {
        return Vec::new();
    };

    let mut parser = Parser::new();
    if parser.set_language(&entry.language).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(text, None) else {
        return Vec::new();
    };

    let rope = ropey::Rope::from_str(text);
    let mut out = Vec::new();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&entry.query, tree.root_node(), text.as_bytes());
    while let Some(matched) = matches.next() {
        for capture in matched.captures {
            let name = entry.query.capture_names()[capture.index as usize];
            let range = capture.node.byte_range();
            let start = rope.byte_to_char(range.start);
            let end = rope.byte_to_char(range.end);
            if start < end {
                out.push(Highlight { start, end, class: class_of(name) });
            }
        }
    }

    // Later captures refine earlier ones (tree-sitter queries overlap), so keep the last
    // range that covers a position and sort by start for the renderer.
    out.sort_by_key(|highlight| (highlight.start, highlight.end));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn go_keywords_and_strings_are_found() {
        let source = "package main\n\nimport \"fmt\"\n\nfunc main() {\n\t// hi\n}\n";
        let spans = spans("go", source);
        assert!(!spans.is_empty(), "no spans for Go");
        assert!(spans.iter().any(|span| span.class == SyntaxClass::Keyword), "no keyword span");
        assert!(spans.iter().any(|span| span.class == SyntaxClass::String), "no string span");
        assert!(spans.iter().any(|span| span.class == SyntaxClass::Comment), "no comment span");
    }

    #[test]
    fn unknown_language_is_empty() {
        assert!(spans("brainfuck", "+++").is_empty());
    }
}
