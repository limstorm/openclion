//! "Generate definition": the declaration under the caret, on real C/C++ headers.

use crate::tests::{none, occurrences, pos_of, service};
use crate::*;
use ide_core::LanguageId;

/// The declaration at the `nth` (1-based) occurrence of `at`.
fn declaration_at_nth(src: &str, language: &str, at: &str, nth: usize) -> Option<CppDeclaration> {
    let language = LanguageId::new(language);
    let snap = Snapshot::text(&language, src);
    let byte = *occurrences(src, at).get(nth - 1).expect("occurrence in the fixture");
    service().cpp_declaration_at(&snap, pos_of(src, byte), &none()).unwrap()
}

fn declaration(src: &str, language: &str, at: &str) -> Option<CppDeclaration> {
    declaration_at_nth(src, language, at, 1)
}

#[test]
fn free_function_in_a_header_gains_no_class() {
    let src = "int add(int a, int b);\n";
    let declaration = declaration(src, "cpp", "add").unwrap();
    assert_eq!(declaration.class, None);
    assert_eq!(declaration.return_type, "int");
    assert_eq!(declaration.parameters, "int a, int b");
    assert_eq!(declaration.definition(), "int add(int a, int b) {\n}\n");
}

#[test]
fn member_functions_are_qualified_by_their_class() {
    let src = "\
class Greeter {
public:
    std::string greet(const std::string& name) const;
    static int count();
    virtual void reset() = 0;
};
";
    let greet = declaration(src, "cpp", "greet").unwrap();
    assert_eq!(greet.class.as_deref(), Some("Greeter"));
    assert_eq!(greet.return_type, "std::string");
    assert_eq!(greet.trailing, " const");
    assert_eq!(greet.definition(), "std::string Greeter::greet(const std::string& name) const {\n}\n");

    // Storage specifiers are dropped; the return type survives.
    let count = declaration(src, "cpp", "count").unwrap();
    assert_eq!(count.return_type, "int");
    assert_eq!(count.definition(), "int Greeter::count() {\n}\n");

    // Pure virtual: nothing to generate.
    assert!(declaration(src, "cpp", "reset").is_none());
}

#[test]
fn constructors_and_destructors_have_no_return_type() {
    let src = "\
class Widget {
public:
    Widget(int value);
    explicit Widget(std::string name);
    ~Widget();
};
";
    let widget = declaration_at_nth(src, "cpp", "Widget", 2).unwrap();
    assert_eq!(widget.return_type, "");
    assert_eq!(widget.parameters, "int value");
    assert_eq!(widget.definition(), "Widget::Widget(int value) {\n}\n");

    let destructor = declaration(src, "cpp", "~Widget").unwrap();
    assert_eq!(destructor.return_type, "");
    assert_eq!(destructor.definition(), "Widget::~Widget() {\n}\n");
}

#[test]
fn declarations_that_are_already_definitions_or_special_members_are_skipped() {
    let src = "\
extern \"C\" void linked();
void defined() { }
struct Node { void next(); };
";
    assert!(declaration(src, "cpp", "defined").is_none());
    let linked = declaration(src, "cpp", "linked").unwrap();
    // `extern "C"` does not change the definition, but its storage is dropped.
    assert_eq!(linked.definition(), "void linked() {\n}\n");
    assert_eq!(declaration(src, "cpp", "next").unwrap().class.as_deref(), Some("Node"));
}

#[test]
fn with_a_body_the_query_is_silent() {
    let src = "void run() {\n    helper();\n}\nvoid helper();\n";
    assert!(declaration_at_nth(src, "cpp", "helper", 2).is_some());
    assert!(declaration(src, "cpp", "run").is_none());
}

#[test]
fn c_headers_with_cpp_classes_fall_back_to_the_cpp_grammar() {
    // `.h` is mapped to C; a C++ class in it still answers.
    let src = "class Greeter {\npublic:\n    void greet();\n};\n";
    let c = declaration(src, "c", "greet");
    assert_eq!(c.unwrap().class.as_deref(), Some("Greeter"));
}

#[test]
fn other_languages_have_no_declarations() {
    let src = "fn add(a: i32) -> i32;\n";
    assert!(declaration(src, "rust", "add").is_none());
}
