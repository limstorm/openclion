//! "Generate definition" for C/C++: the declaration under the caret, reduced to
//! what a definition stub needs (name, class, parameters, trailing qualifiers).
//!
//! Purely syntactic, on purpose: it answers for headers the language server has
//! not indexed yet, and it only claims declarations it can read exactly.
use crate::{Ctx, StructureResult};
use ide_core::Pos;
use serde::{Deserialize, Serialize};
use specta::Type;
use tree_sitter::Node;

/// A C/C++ function declaration, as "Generate definition" needs it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CppDeclaration {
    pub name: String,
    /// `Widget` for a member function, `None` for a free function.
    pub class: Option<String>,
    /// `void`, `std::string`; empty for constructors and destructors.
    pub return_type: String,
    /// The text between the parentheses of the declaration.
    pub parameters: String,
    /// The qualifiers after the parameter list (` const`, ` noexcept`, ` &`).
    pub trailing: String,
}

impl CppDeclaration {
    /// The definition stub: `<return> Class::name(params)<trailing> { }`.
    pub fn definition(&self) -> String {
        let qualified = match &self.class {
            Some(class) => format!("{class}::{}", self.name),
            None => self.name.clone(),
        };
        let ret = if self.return_type.is_empty() { String::new() } else { format!("{} ", self.return_type) };
        format!("{ret}{qualified}({}){} {{\n}}\n", self.parameters, self.trailing)
    }
}

/// Words that a declaration may carry but a definition in the source file must
/// not repeat (or that change the meaning of the declaration entirely).
const DROPPED: &[&str] =
    &["virtual", "static", "inline", "explicit", "friend", "constexpr", "consteval", "mutable", "extern"];

/// Strips a leading `[[attribute]]` and the storage specifiers from a return type.
fn clean_return_type(text: &str) -> String {
    let mut rest = text.trim();
    while let Some(stripped) = rest.strip_prefix("[[") {
        match stripped.find("]]") {
            Some(end) => rest = stripped[end + 2..].trim_start(),
            None => break,
        }
    }
    let words: Vec<&str> = rest.split_whitespace().collect();
    let kept: Vec<&str> = words.iter().copied().filter(|w| !DROPPED.contains(w)).collect();
    kept.join(" ")
}

/// Keeps only the qualifiers a definition repeats: `const`, `volatile`,
/// `noexcept(...)`, `&`, `&&`; drops `override`/`final` and trailing punctuation.
fn clean_trailing(text: &str) -> String {
    let mut out = String::new();
    for word in text.split_whitespace() {
        let bare = word.trim_end_matches([';', ',']);
        if bare.is_empty() || bare == "override" || bare == "final" {
            continue;
        }
        out.push(' ');
        out.push_str(bare);
    }
    out
}

/// The function name of a declarator: `bar`, `bar` of `Widget::bar`, `~Widget`.
fn declarator_name(node: &Node, text: &str) -> Option<(String, Option<String>)> {
    let own = |n: &Node| n.utf8_text(text.as_bytes()).ok().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
    match node.kind() {
        "identifier" | "field_identifier" | "destructor_name" => Some((own(node)?, None)),
        "qualified_identifier" => {
            let name = own(&node.child_by_field_name("name")?)?;
            let class = own(&node.child_by_field_name("scope")?)?;
            Some((name, Some(class)))
        }
        // `operator==`, function pointers and templates are out of scope.
        _ => None,
    }
}

/// The class whose body contains `node`, when there is one.
fn enclosing_class(node: &Node, text: &str) -> Option<String> {
    let mut current = node.parent();
    while let Some(n) = current {
        if matches!(n.kind(), "class_specifier" | "struct_specifier" | "union_specifier") {
            let name = n.child_by_field_name("name")?;
            return name.utf8_text(text.as_bytes()).ok().map(|s| s.trim().to_owned());
        }
        if matches!(n.kind(), "function_definition" | "translation_unit") {
            return None;
        }
        current = n.parent();
    }
    None
}

/// `node` or its nearest ancestor of `kind`.
fn find_ancestor<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut current = Some(node);
    while let Some(n) = current {
        if n.kind() == kind {
            return Some(n);
        }
        current = n.parent();
    }
    None
}

/// The deepest node covering `byte`, by descending from the root (an empty
/// `descendant_for_byte_range` range is ambiguous at declaration boundaries).
fn node_at<'a>(root: Node<'a>, byte: usize) -> Node<'a> {
    let mut node = root;
    loop {
        let mut next = None;
        for i in 0..node.child_count() {
            let Some(child) = node.child(i) else { continue };
            if child.start_byte() <= byte && byte < child.end_byte() {
                next = Some(child);
                break;
            }
        }
        match next {
            Some(child) => node = child,
            None => return node,
        }
    }
}

/// The declaration the caret sits in, when a definition can be generated for it.
pub(crate) fn declaration_at(cx: &Ctx, pos: Pos) -> StructureResult<Option<CppDeclaration>> {
    if !matches!(cx.grammar, "c" | "cpp") {
        return Ok(None);
    }
    let text = cx.text();
    let start = node_at(cx.tree.root_node(), cx.lines.byte(pos));
    let Some(function) = find_ancestor(start, "function_declarator") else {
        return Ok(None);
    };
    // A definition is already there; a template's stub would be wrong.
    if find_ancestor(function, "function_definition").is_some()
        || find_ancestor(function, "template_declaration").is_some()
    {
        return Ok(None);
    }
    let Some(declaration) =
        find_ancestor(function, "declaration").or_else(|| find_ancestor(function, "field_declaration"))
    else {
        return Ok(None);
    };
    let declaration_text = declaration.utf8_text(text.as_bytes()).unwrap_or_default();
    if declaration_text.contains("= default")
        || declaration_text.contains("= delete")
        || declaration_text.contains("= 0")
    {
        return Ok(None);
    }
    let Some(effect) = function.child_by_field_name("declarator") else {
        return Ok(None);
    };
    let Some((name, qualified_class)) = declarator_name(&effect, text) else {
        return Ok(None);
    };
    let Some(parameters) = function.child_by_field_name("parameters") else {
        return Ok(None);
    };

    let parameters_text = parameters.utf8_text(text.as_bytes()).unwrap_or("()");
    let parameters_text = parameters_text.trim();
    let parameters_text = parameters_text.strip_prefix('(').unwrap_or(parameters_text);
    let parameters_text = parameters_text.strip_suffix(')').unwrap_or(parameters_text).trim();

    // The declaration's `type` field is the return type; constructors have none.
    let return_type = declaration
        .child_by_field_name("type")
        .and_then(|n| n.utf8_text(text.as_bytes()).ok())
        .map(clean_return_type)
        .unwrap_or_default();

    // Qualifiers after the parameter list sit on the declarator (`const`, `noexcept`).
    let trailing = text.get(parameters.end_byte()..function.end_byte()).unwrap_or_default();
    let trailing = clean_trailing(trailing.trim());

    let class = qualified_class.or_else(|| enclosing_class(&declaration, text));
    Ok(Some(CppDeclaration { name, class, return_type, parameters: parameters_text.to_owned(), trailing }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_types_lose_storage_specifiers_and_attributes() {
        assert_eq!(clean_return_type("virtual void"), "void");
        assert_eq!(clean_return_type("static inline int"), "int");
        assert_eq!(clean_return_type("[[nodiscard]] std::string"), "std::string");
        assert_eq!(clean_return_type(""), "");
    }

    #[test]
    fn trailing_qualifiers_keep_what_a_definition_repeats() {
        assert_eq!(clean_trailing("const noexcept override"), " const noexcept");
        assert_eq!(clean_trailing(""), "");
        assert_eq!(clean_trailing("&;"), " &");
    }

    #[test]
    fn stubs_are_qualified_and_carry_qualifiers() {
        let declaration = CppDeclaration {
            name: "greet".into(),
            class: Some("Greeter".into()),
            return_type: "std::string".into(),
            parameters: "const std::string& name".into(),
            trailing: " const".into(),
        };
        assert_eq!(declaration.definition(), "std::string Greeter::greet(const std::string& name) const {\n}\n");
        let constructor = CppDeclaration {
            name: "Greeter".into(),
            class: Some("Greeter".into()),
            return_type: String::new(),
            parameters: "int value".into(),
            trailing: String::new(),
        };
        assert_eq!(constructor.definition(), "Greeter::Greeter(int value) {\n}\n");
    }
}
