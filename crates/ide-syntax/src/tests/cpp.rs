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

/// The class at the `nth` (1-based) occurrence of `at`.
fn class_at_nth(src: &str, language: &str, at: &str, nth: usize) -> Option<CppClass> {
    let language = LanguageId::new(language);
    let snap = Snapshot::text(&language, src);
    let byte = *occurrences(src, at).get(nth - 1).expect("occurrence in the fixture");
    service().cpp_class_at(&snap, pos_of(src, byte), &none()).unwrap()
}

fn class_at(src: &str, language: &str, at: &str) -> Option<CppClass> {
    class_at_nth(src, language, at, 1)
}

const ANIMALS: &str = "\
class Animal {
public:
    virtual ~Animal() = default;
    virtual void speak() const = 0;
    virtual int legs() noexcept { return 4; }
    virtual void eat() &;
    void plain();
    static int count();
    Animal();
    Animal(int);
    operator bool() const;
    virtual void mixed() = delete;
    virtual void def() = default;
};

struct Dog : public Animal, private virtual ns::Pet<int>, protected Other {
    void speak() const override;
    int x;
};
";

#[test]
fn classes_expose_bases_and_the_members_a_derived_class_can_override() {
    let animal = class_at(ANIMALS, "cpp", "Animal").unwrap();
    assert_eq!(animal.name, "Animal");
    assert!(animal.bases.is_empty());
    let names: Vec<&str> = animal.members.iter().map(|m| m.name.as_str()).collect();
    // The destructor, constructors, operators and `= default`/`= delete` are not
    // members a derived class implements.
    assert_eq!(names, ["speak", "legs", "eat", "plain", "count"]);

    let speak = &animal.members[0];
    assert_eq!(speak.return_type, "void");
    assert_eq!(speak.parameters, "");
    assert_eq!(speak.trailing, " const");
    assert!(speak.is_virtual && speak.is_pure && !speak.is_static);
    assert_eq!(speak.signature(), "virtual void speak() const = 0");
    assert_eq!(speak.override_stub(), "void speak() const override { }");

    // Inline virtual definitions are still overridable; non-void returns get a value.
    let legs = &animal.members[1];
    assert!(legs.is_virtual && !legs.is_pure);
    assert_eq!(legs.trailing, " noexcept");
    assert_eq!(legs.override_stub(), "int legs() noexcept override { return {}; }");

    // Ref qualifiers survive; a plain and a static member are not virtual.
    assert_eq!(animal.members[2].trailing, " &");
    assert!(!animal.members[3].is_virtual);
    assert!(animal.members[4].is_static && !animal.members[4].is_virtual);
}

#[test]
fn bases_lose_access_specifiers_and_member_trailing_drops_override() {
    let dog = class_at(ANIMALS, "cpp", "Dog").unwrap();
    assert_eq!(dog.name, "Dog");
    assert_eq!(dog.bases, ["Animal", "ns::Pet<int>", "Other"]);
    assert_eq!(dog.members.len(), 1);
    let speak = &dog.members[0];
    assert_eq!(speak.name, "speak");
    assert_eq!(speak.trailing, " const");
    assert!(!speak.is_virtual);
    assert_eq!(speak.override_stub(), "void speak() const override { }");
}

#[test]
fn the_class_is_found_from_anywhere_in_its_body() {
    assert_eq!(class_at(ANIMALS, "cpp", "Animal").unwrap().name, "Animal");
    assert_eq!(class_at(ANIMALS, "cpp", "speak").unwrap().name, "Animal");
    assert_eq!(class_at(ANIMALS, "cpp", "legs").unwrap().name, "Animal");
    // `int x;` is a data member: no class member function, but the class is still found.
    let dog = class_at_nth(ANIMALS, "cpp", "x", 1).unwrap();
    assert_eq!(dog.name, "Dog");
    // Inside an inline member body, and in a nested class of another class.
    let inline = "class Inline {\n    void run() {\n        int local = 0;\n    }\n    virtual void go() = 0;\n};\n";
    assert_eq!(class_at(inline, "cpp", "local").unwrap().name, "Inline");
    let nested = "class Outer {\n    struct Inner {\n        void f();\n    };\n};\n";
    assert_eq!(class_at(nested, "cpp", "f").unwrap().name, "Inner");
    // A free function outside the class has none.
    let src = "int add(int a, int b);\nclass Later { };\n";
    assert!(class_at(src, "cpp", "add").is_none());
    // After the closing brace the caret is outside the class.
    assert!(class_at("class Empty {\n};\nint after;\n", "cpp", "after").is_none());
}

#[test]
fn c_headers_with_cpp_classes_fall_back_to_the_cpp_grammar_for_classes() {
    let src = "class Greeter {\npublic:\n    virtual void greet() = 0;\n};\n";
    let class = class_at(src, "c", "greet").unwrap();
    assert_eq!(class.name, "Greeter");
    assert!(class.members[0].is_pure);
}

#[test]
fn other_languages_have_no_classes() {
    let src = "struct Point { x: i32 }\n";
    assert!(class_at(src, "rust", "Point").is_none());
}

#[test]
fn a_class_can_be_parsed_from_plain_text_by_name() {
    let src = "\
namespace app {
class Base {
public:
    virtual void run() = 0;
    virtual int score() const = 0;
protected:
    virtual void reset();
};
}
class Other { };
";
    let base = class_in_text(src, "Base").unwrap();
    assert_eq!(base.name, "Base");
    assert_eq!(base.bases, [] as [String; 0]);
    let names: Vec<&str> = base.members.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["run", "score", "reset"]);
    assert_eq!(base.members[0].override_stub(), "void run() override { }");
    assert_eq!(base.members[1].override_stub(), "int score() const override { return {}; }");
    assert_eq!(base.members[2].override_stub(), "void reset() override { }");

    // Qualified and template spellings resolve by their plain name.
    assert_eq!(class_in_text(src, "app::Base").unwrap().name, "Base");
    assert_eq!(class_in_text(src, "Base<int>").unwrap().name, "Base");
    assert!(class_in_text(src, "Missing").is_none());
    assert!(class_in_text(src, "").is_none());
}

#[test]
fn class_members_keep_noexcept_and_final_is_flagged_not_offered() {
    let src = "\
class Base {
public:
    virtual void a() noexcept(false);
    virtual void b() final;
    virtual void c() override;
};
";
    let base = class_in_text(src, "Base").unwrap();
    assert_eq!(base.members[0].trailing, " noexcept(false)");
    assert!(base.members[1].is_final);
    // `override` on a base member is dropped, like on a derived one.
    assert_eq!(base.members[2].trailing, "");
}
