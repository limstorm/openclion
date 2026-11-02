//! "Generate definition" and "Generate Override/Implement Methods" for C/C++:
//! the declaration under the caret (reduced to what a definition stub needs) and
//! the class under the caret (its bases and member functions, reduced to what an
//! override stub needs).
//!
//! Purely syntactic, on purpose: it answers for headers the language server has
//! not indexed yet, and it only claims declarations it can read exactly.
use crate::lines::Lines;
use crate::{Ctx, StructureResult};
use ide_core::{LanguageId, Pos, Range};
use ide_language::Syntax;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::OnceLock;
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

/// One member function of a C/C++ class body, as "Generate Override/Implement
/// Methods" needs it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CppMember {
    pub name: String,
    /// `void`, `std::string`; empty for constructors and destructors.
    pub return_type: String,
    /// The text between the parentheses of the declaration.
    pub parameters: String,
    /// The qualifiers after the parameter list (` const`, ` noexcept`, ` &`).
    pub trailing: String,
    pub is_virtual: bool,
    /// `= 0`: the derived class must implement it.
    pub is_pure: bool,
    pub is_static: bool,
    /// `final`: it cannot be overridden.
    pub is_final: bool,
}

fn normalized(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

impl CppMember {
    /// Whether two members declare the same function: the name, the parameter
    /// list and the qualifiers that distinguish overloads (`const`, `&`, …).
    /// Constructors and operators never reach this type.
    pub fn same_signature(&self, other: &Self) -> bool {
        self.name == other.name
            && normalized(&self.parameters) == normalized(&other.parameters)
            && normalized(&self.trailing) == normalized(&other.trailing)
    }

    /// The declaration as a chooser shows it: `virtual int legs() const = 0`.
    pub fn signature(&self) -> String {
        let ret = if self.return_type.is_empty() { String::new() } else { format!("{} ", self.return_type) };
        let virtuality = if self.is_virtual { "virtual " } else { "" };
        let pure = if self.is_pure { " = 0" } else { "" };
        format!("{virtuality}{ret}{}({}){}{pure}", self.name, self.parameters, self.trailing)
    }

    /// The override stub inserted into the derived class body:
    /// `int legs() const override { return {}; }` (`{ }` for a `void` return).
    pub fn override_stub(&self) -> String {
        let ret = if self.return_type.is_empty() { String::new() } else { format!("{} ", self.return_type) };
        let body = if self.return_type.trim().is_empty() || self.return_type.trim() == "void" {
            "{ }"
        } else {
            "{ return {}; }"
        };
        format!("{ret}{}({}){} override {body}", self.name, self.parameters, self.trailing)
    }
}

/// A C/C++ class (or struct) body, as "Generate Override/Implement Methods"
/// needs it: its bases and its member functions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CppClass {
    pub name: String,
    /// The base classes as written, without access specifiers: `Animal`, `ns::Pet<T>`.
    pub bases: Vec<String>,
    /// The member functions declared (or defined inline) in the body.
    pub members: Vec<CppMember>,
    /// The body's braces, `{` to `}`: generated members go just before the `}`.
    pub body: Range,
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

/// The text between the parentheses of a parameter list (` (int a, int b)`).
fn parameter_text(parameters: &Node, text: &str) -> String {
    let raw = parameters.utf8_text(text.as_bytes()).unwrap_or("()").trim();
    let raw = raw.strip_prefix('(').unwrap_or(raw);
    let raw = raw.strip_suffix(')').unwrap_or(raw);
    raw.trim().to_owned()
}

/// `text` without a namespace (`ns::Foo`) or template arguments (`Foo<T>`): the
/// name a base list, a class body and the project index use.
fn bare_class_name(text: &str) -> &str {
    let last = text.trim().rsplit("::").next().unwrap_or(text);
    last.split('<').next().unwrap_or(last).trim()
}

/// `node` or a descendant of `kind`.
fn has_descendant(node: &Node, kind: &str) -> bool {
    let mut stack = vec![*node];
    while let Some(n) = stack.pop() {
        if n.kind() == kind {
            return true;
        }
        for i in 0..n.named_child_count() {
            if let Some(child) = n.named_child(i) {
                stack.push(child);
            }
        }
    }
    false
}

/// Whether a member carries the `virtual_specifier` (an anonymous `override` or
/// `final` token) spelled `specifier`.
fn has_virtual_specifier(node: &Node, text: &str, specifier: &str) -> bool {
    let mut stack = vec![*node];
    while let Some(n) = stack.pop() {
        if n.kind() == "virtual_specifier" && n.utf8_text(text.as_bytes()).is_ok_and(|t| t.trim() == specifier) {
            return true;
        }
        for i in 0..n.named_child_count() {
            if let Some(child) = n.named_child(i) {
                stack.push(child);
            }
        }
    }
    false
}

/// `virtual` is an anonymous token directly under the declaration.
fn is_virtual_member(node: &Node) -> bool {
    (0..node.child_count()).any(|i| node.child(i).is_some_and(|child| child.kind() == "virtual"))
}

/// `static` is a `storage_class_specifier` of the declaration.
fn is_static_member(node: &Node, text: &str) -> bool {
    (0..node.named_child_count()).any(|i| {
        node.named_child(i).is_some_and(|child| {
            child.kind() == "storage_class_specifier"
                && child.utf8_text(text.as_bytes()).is_ok_and(|t| t.trim() == "static")
        })
    })
}

/// A pure virtual declaration: `= 0` on a declaration or a `pure_virtual_clause`.
fn is_pure_member(node: &Node, text: &str) -> bool {
    node.child_by_field_name("default_value")
        .and_then(|value| value.utf8_text(text.as_bytes()).ok())
        .is_some_and(|value| value.trim() == "0")
        || has_descendant(node, "pure_virtual_clause")
}

/// One member function of a class body, when the declaration is a function a
/// derived class can override (not a constructor, destructor, operator, field,
/// function pointer, `= default` or `= delete`).
fn member_of(node: &Node, class_name: &str, text: &str) -> Option<CppMember> {
    if has_descendant(node, "default_method_clause") || has_descendant(node, "delete_method_clause") {
        return None;
    }
    let declarator = node.child_by_field_name("declarator")?;
    if declarator.kind() != "function_declarator" {
        return None;
    }
    // Constructors are `declaration`s in this grammar; a qualified or template
    // member (`operator==`, `~Widget`, `foo<T>`) is not a plain override.
    let effect = declarator.child_by_field_name("declarator")?;
    let (name, _) = declarator_name(&effect, text)?;
    if name.starts_with('~') || name == class_name {
        return None;
    }
    let parameters = declarator.child_by_field_name("parameters")?;
    let trailing = text.get(parameters.end_byte()..declarator.end_byte()).unwrap_or_default();
    let return_type = node
        .child_by_field_name("type")
        .and_then(|n| n.utf8_text(text.as_bytes()).ok())
        .map(clean_return_type)
        .unwrap_or_default();
    Some(CppMember {
        name,
        return_type,
        parameters: parameter_text(&parameters, text),
        trailing: clean_trailing(trailing.trim()),
        is_virtual: is_virtual_member(node),
        is_pure: is_pure_member(node, text),
        is_static: is_static_member(node, text),
        is_final: has_virtual_specifier(node, text, "final"),
    })
}

fn members_of(body: &Node, class_name: &str, text: &str) -> Vec<CppMember> {
    let mut members = Vec::new();
    for i in 0..body.named_child_count() {
        let Some(node) = body.named_child(i) else { continue };
        if !matches!(node.kind(), "field_declaration" | "function_definition") {
            continue;
        }
        if let Some(member) = member_of(&node, class_name, text) {
            members.push(member);
        }
    }
    members
}

/// The class's bases, in declaration order, without `public`/`virtual` and friends.
fn bases_of(spec: &Node, text: &str) -> Vec<String> {
    let mut bases = Vec::new();
    for i in 0..spec.named_child_count() {
        let Some(clause) = spec.named_child(i) else { continue };
        if clause.kind() != "base_class_clause" {
            continue;
        }
        for j in 0..clause.named_child_count() {
            let Some(base) = clause.named_child(j) else { continue };
            if !matches!(base.kind(), "type_identifier" | "qualified_identifier" | "template_type" | "identifier") {
                continue;
            }
            if let Ok(written) = base.utf8_text(text.as_bytes()) {
                let written = written.trim();
                if !written.is_empty() {
                    bases.push(written.to_owned());
                }
            }
        }
    }
    bases
}

fn class_from_node(node: &Node, text: &str, lines: &Lines) -> Option<CppClass> {
    let name = node.child_by_field_name("name")?.utf8_text(text.as_bytes()).ok()?.trim().to_owned();
    let body = node.child_by_field_name("body")?;
    Some(CppClass {
        members: members_of(&body, bare_class_name(&name), text),
        bases: bases_of(node, text),
        body: lines.range(body.start_byte(), body.end_byte()),
        name,
    })
}

/// The nearest `class` / `struct` whose body contains `node`, including from
/// inside a member's body: generating is allowed anywhere in the class. A local
/// or nested class is nearer than the class around it.
fn enclosing_specifier(node: Node) -> Option<Node> {
    let mut current = Some(node);
    while let Some(n) = current {
        if matches!(n.kind(), "class_specifier" | "struct_specifier") {
            return Some(n);
        }
        current = n.parent();
    }
    None
}

/// The class the caret sits in, when it sits in one.
pub(crate) fn class_at(cx: &Ctx, pos: Pos) -> StructureResult<Option<CppClass>> {
    if !matches!(cx.grammar, "c" | "cpp") {
        return Ok(None);
    }
    let start = node_at(cx.tree.root_node(), cx.lines.byte(pos));
    Ok(enclosing_specifier(start).and_then(|spec| class_from_node(&spec, cx.text(), &cx.lines)))
}

/// The first class (or struct) named `name` in `text`, wherever it is nested.
fn class_named(root: Node, text: &str, lines: &Lines, name: &str) -> Option<CppClass> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if matches!(node.kind(), "class_specifier" | "struct_specifier") {
            let named = node
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(text.as_bytes()).ok())
                .is_some_and(|written| bare_class_name(written) == name);
            if named && let Some(class) = class_from_node(&node, text, lines) {
                return Some(class);
            }
        }
        for i in (0..node.named_child_count()).rev() {
            if let Some(child) = node.named_child(i) {
                stack.push(child);
            }
        }
    }
    None
}

/// The C++ grammar for texts that are not open documents (a base class read from
/// another file); no settings are needed to map the grammar name.
fn cpp_syntax() -> &'static Syntax {
    static SYNTAX: OnceLock<Syntax> = OnceLock::new();
    SYNTAX.get_or_init(|| Syntax::with_languages(&[]))
}

/// Parses the class (or struct) named `name` out of `text` with the C++ grammar.
/// `name` may be qualified or a template instantiation (`ns::Pet<int>`); only
/// the plain name is matched.
pub fn class_in_text(text: &str, name: &str) -> Option<CppClass> {
    let name = bare_class_name(name);
    if name.is_empty() {
        return None;
    }
    let language = LanguageId::new("cpp");
    let lines = Lines::new(text);
    cpp_syntax()
        .with_parsed(&language, text, &|| false, |parsed| {
            class_named(parsed.tree.root_node(), parsed.text, &lines, name)
        })
        .ok()
        .flatten()
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
