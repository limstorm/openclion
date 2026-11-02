//! C/C++ source ↔ header pairing: CLion's "Go to Related Symbol" (switch
//! between a translation unit and the header that declares it, and back).
//!
//! The conventional layouts are checked directly (fast, deterministic), and the
//! project index is the fallback for unusual trees (`src/greeter.cpp` next to
//! `include/greeter.h` is the common case that the conventions cover).
use crate::rpc::Dispatcher;
use crate::{Host, Project, apply_workspace_edit};
use ide_core::{
    DocId, FileEdit, IdeError, IdeResult, IpcPath, Pos, Range, RequestId, SymbolKind, TextEdit, WorkspaceEdit,
};
use ide_index::{SymbolFilter, SymbolMatch};
use ide_syntax::{CppClass, CppMember, class_in_text};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::{Path, PathBuf};

/// Extensions that are compiled (the "source" side).
const SOURCES: &[&str] = &["c", "cc", "cpp", "cxx", "c++", "m", "mm"];
/// Extensions that declare (the "header" side).
const HEADERS: &[&str] = &["h", "hh", "hpp", "hxx", "h++", "inl", "ipp", "tpp"];

fn extension(path: &Path) -> Option<String> {
    path.extension().map(|e| e.to_string_lossy().to_lowercase())
}

fn is_source(ext: &str) -> bool {
    SOURCES.contains(&ext)
}

/// The extensions to look for on the other side of `path`.
fn counterparts(path: &Path) -> Option<&'static [&'static str]> {
    let ext = extension(path)?;
    if is_source(&ext) {
        Some(HEADERS)
    } else if HEADERS.contains(&ext.as_str()) {
        Some(SOURCES)
    } else {
        None
    }
}

fn with_extension(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    dir.join(format!("{stem}.{ext}"))
}

/// The directory on the other side of a `src/` ↔ `include/` layout, when the
/// file sits under one of those components (`apps/web/src` → `apps/web/include`).
fn sibling_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut current = Some(dir);
    while let Some(d) = current {
        let name = d.file_name().map(|n| n.to_string_lossy().to_lowercase());
        match name.as_deref() {
            Some("src" | "source") => {
                if let Some(base) = d.parent() {
                    out.push(base.join("include"));
                    out.push(base.join("inc"));
                }
                break;
            }
            Some("include" | "inc") => {
                if let Some(base) = d.parent() {
                    out.push(base.join("src"));
                    out.push(base.join("source"));
                }
                break;
            }
            _ => current = d.parent(),
        }
    }
    out
}

/// Candidate paths in the conventional layouts of the file's own tree, most
/// likely first, without checking existence.
fn conventional_paths(path: &Path) -> Vec<PathBuf> {
    let Some(exts) = counterparts(path) else { return Vec::new() };
    let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
        return Vec::new();
    };
    let Some(dir) = path.parent() else { return Vec::new() };
    let mut out = Vec::new();
    // Same directory first: `foo.cpp` ↔ `foo.h`.
    for ext in exts {
        out.push(with_extension(dir, &stem, ext));
    }
    // Sibling `src/` ↔ `include/`, including the `include/foo/foo.h` nesting.
    let own = dir.file_name().map(|n| n.to_string_lossy().into_owned());
    for base in sibling_dirs(dir) {
        for ext in exts {
            out.push(with_extension(&base, &stem, ext));
            out.push(with_extension(&base.join(&stem), &stem, ext));
            if let Some(own) = &own {
                out.push(with_extension(&base.join(own), &stem, ext));
            }
        }
    }
    out
}

/// Index fallback: any file with the same stem on the other side, nearest path first.
fn indexed_paths(project: &Project, path: &Path) -> Vec<PathBuf> {
    let Some(exts) = counterparts(path) else { return Vec::new() };
    let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = project
        .index
        .find_files(&stem, 200)
        .into_iter()
        .map(|m| m.path.to_path_buf())
        .filter(|p| p.file_stem().is_some_and(|s| s == stem.as_str()))
        .filter(|p| extension(p).is_some_and(|e| exts.contains(&e.as_str())))
        .filter(|p| p != path)
        .collect();
    // Same directory, then fewer path segments, then alphabetically: stable.
    let dir = path.parent().unwrap_or(Path::new(""));
    found.sort_by_key(|p| {
        let same_dir = p.parent() != Some(dir);
        let depth = p.components().count();
        (same_dir, depth, p.to_string_lossy().into_owned())
    });
    found.dedup();
    found
}

/// Related headers (from a source) or sources (from a header) of `path`, best first.
pub fn related(project: &Project, path: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for candidate in conventional_paths(path) {
        if candidate.is_file() && !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    for candidate in indexed_paths(project, path) {
        if !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    out
}

/// Where "Generate definition" put the stub.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedDefinition {
    pub file: IpcPath,
    /// 1-based line of the generated stub.
    pub line: u32,
}

fn is_header(path: &Path) -> bool {
    extension(path).is_some_and(|e| HEADERS.contains(&e.as_str()))
}

fn is_source_file(path: &Path) -> bool {
    extension(path).is_some_and(|e| SOURCES.contains(&e.as_str()))
}

/// Generates the definition of the declaration at `pos` in the header `doc` and
/// appends it to the matching source file (the counterpart `Go to Related Symbol`
/// would open). Returns the file and the line the stub starts on.
pub async fn generate_definition(
    host: &Host,
    window: &str,
    doc: DocId,
    pos: Pos,
    request: Option<RequestId>,
) -> IdeResult<GeneratedDefinition> {
    let project = host.project(window)?;
    let header = project.documents.snapshot(doc)?;
    if !is_header(&header.path) {
        return Err(IdeError::invalid("Generate Definition works from a C/C++ header file"));
    }
    let Some(declaration) = crate::ops::syntax::syntax_cpp_declaration(host, window, doc, pos, request).await? else {
        return Err(IdeError::unavailable("There is no function declaration here to generate a definition for"));
    };
    let Some(target) = related(&project, &header.path).into_iter().find(|p| is_source_file(p)) else {
        return Err(IdeError::unavailable(format!(
            "No source file corresponds to {}: create one next to it, or in `src/`, then try again",
            header.path.display()
        )));
    };

    // Open the target so the edit lands in the editor and on disk together.
    let target_doc = match project.documents.by_path(&target) {
        Some(existing) => existing,
        None => project.documents.open(&target)?.id,
    };
    let target_snapshot = project.documents.snapshot(target_doc)?;
    let text = target_snapshot.text.clone();
    let stub = declaration.definition();
    let mut insertion = String::new();
    if !text.is_empty() && !text.ends_with('\n') {
        insertion.push('\n');
    }
    if !text.trim_end().is_empty() {
        insertion.push('\n');
    }
    let leading_newlines = insertion.matches('\n').count();
    insertion.push_str(&stub);
    let newlines_before = text.matches('\n').count() + leading_newlines;
    let line = newlines_before as u32 + 1;

    let end = ide_text::Document::new(&text).offset_to_pos(text.len() as u32);
    let edit = WorkspaceEdit {
        files: vec![FileEdit::Edit {
            path: target.clone().into(),
            edits: vec![TextEdit { range: Range::new(end, end), new_text: insertion }],
        }],
    };
    apply_workspace_edit(&project.documents, &edit)?;
    Ok(GeneratedDefinition { file: target.into(), line })
}

/// One base-class virtual member "Generate Override/Implement Methods" can add
/// to a class, as the chooser lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GeneratableMember {
    pub name: String,
    /// The base declaration, e.g. `virtual void speak() const = 0`.
    pub signature: String,
    /// The base class, as written in the derived class's base list.
    pub class: String,
    /// Where the base class is declared.
    pub file: IpcPath,
    /// 1-based line of the base class declaration.
    pub line: u32,
}

/// A member to generate, still tied to the base class and file it came from.
struct MissingMember {
    member: CppMember,
    class: String,
    file: PathBuf,
    line: u32,
}

impl From<MissingMember> for GeneratableMember {
    fn from(missing: MissingMember) -> Self {
        Self {
            class: missing.class,
            file: missing.file.into(),
            line: missing.line,
            name: missing.member.name.clone(),
            signature: missing.member.signature(),
        }
    }
}

/// `ns::Animal` → `Animal`; template instantiations are left out (their members
/// would have to be instantiated to be spelled correctly).
fn base_name(written: &str) -> Option<&str> {
    let written = written.trim();
    if written.contains('<') {
        return None;
    }
    let name = written.rsplit("::").next().unwrap_or(written).trim();
    (!name.is_empty()).then_some(name)
}

/// The first `class` / `struct` symbol named `name` in the project index.
fn base_symbol(project: &Project, name: &str) -> Option<SymbolMatch> {
    project
        .index
        .find_symbols(name, SymbolFilter::Types, 20)
        .into_iter()
        .find(|m| matches!(m.kind, SymbolKind::Class | SymbolKind::Struct) && m.name == name)
}

/// The quoted includes of a C/C++ file (`#include "animal.h"`), in order.
fn quoted_includes(text: &str) -> Vec<&str> {
    let mut includes = Vec::new();
    for line in text.lines() {
        let line = line.trim_start();
        let Some(rest) = line.strip_prefix('#') else { continue };
        let Some(rest) = rest.trim_start().strip_prefix("include") else { continue };
        let Some(rest) = rest.trim_start().strip_prefix('"') else { continue };
        if let Some(end) = rest.find('"') {
            includes.push(&rest[..end]);
        }
    }
    includes
}

/// The base class `name` of a class in `path`: the index's symbol first, then the
/// file itself and its quoted includes. `.h` headers are indexed as C, so a C++
/// base declared in one may have no class symbol: reading the included file is
/// the fallback. Returns the class and where it was declared.
fn resolve_base(project: &Project, path: &Path, text: &str, name: &str) -> Option<(CppClass, PathBuf, u32)> {
    if let Some(symbol) = base_symbol(project, name)
        && let Ok(content) = project.text_of(&symbol.path)
        && let Some(class) = class_in_text(&content, name).or_else(|| class_in_text(&content, &symbol.name))
    {
        return Some((class, symbol.path.to_path_buf(), symbol.range.start.line + 1));
    }
    if let Some(class) = class_in_text(text, name) {
        let line = class.body.start.line + 1;
        return Some((class, path.to_path_buf(), line));
    }
    let dir = path.parent().unwrap_or(Path::new(""));
    for include in quoted_includes(text) {
        let file = dir.join(include);
        if !file.is_file() {
            continue;
        }
        let Ok(content) = project.text_of(&file) else { continue };
        if let Some(class) = class_in_text(&content, name) {
            let line = class.body.start.line + 1;
            return Some((class, file, line));
        }
    }
    None
}

/// The base-class members `class` neither declares nor defines, in base order.
/// Virtual only: a pure virtual member is mandatory, a plain virtual one is the
/// CLion "Override Methods" entry, and static or `final` members cannot override.
fn missing_members(project: &Project, class: &CppClass, path: &Path, text: &str) -> Vec<MissingMember> {
    let mut missing: Vec<MissingMember> = Vec::new();
    for base in &class.bases {
        let Some(name) = base_name(base) else { continue };
        let Some((base_class, file, line)) = resolve_base(project, path, text, name) else { continue };
        for member in base_class.members {
            if !member.is_virtual || member.is_static || member.is_final {
                continue;
            }
            if class.members.iter().any(|own| own.same_signature(&member))
                || missing.iter().any(|found| found.member.same_signature(&member))
            {
                continue;
            }
            missing.push(MissingMember { class: base.clone(), file: file.clone(), line, member });
        }
    }
    missing
}

/// The class at `pos` and the base members it does not override yet.
async fn class_members(
    host: &Host,
    window: &str,
    doc: DocId,
    pos: Pos,
) -> IdeResult<Option<(CppClass, Vec<MissingMember>)>> {
    let project = host.project(window)?;
    let Some(class) = crate::ops::syntax::syntax_cpp_class(host, window, doc, pos, None).await? else {
        return Ok(None);
    };
    let snapshot = project.documents.snapshot(doc)?;
    let resolved = class.clone();
    let missing =
        host.blocking(move || Ok(missing_members(&project, &resolved, &snapshot.path, &snapshot.text))).await?;
    Ok(Some((class, missing)))
}

/// The base-class virtual members the class at `pos` can implement, best first:
/// the list behind the "Override Methods" chooser. `name`, `signature` and the
/// location of the base class that declares each one.
pub async fn members_to_generate(host: &Host, window: &str, doc: DocId, pos: Pos) -> IdeResult<Vec<GeneratableMember>> {
    let Some((_, missing)) = class_members(host, window, doc, pos).await? else {
        return Err(IdeError::unavailable("Put the caret in a C/C++ class to generate its overrides"));
    };
    Ok(missing.into_iter().map(GeneratableMember::from).collect())
}

/// The indentation generated stubs get: the class's member indentation (the
/// last member-like line inside the body), else one level past the closing brace.
fn member_indent(text: &str, body: Range, close_line_start: usize) -> String {
    let document = ide_text::Document::new(text);
    let start = document.pos_to_byte(body.start) + 1;
    let end = close_line_start.max(start).min(text.len());
    let inside = &text[start.min(end)..end];
    for line in inside.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.ends_with(';') || trimmed.ends_with('}') || trimmed.ends_with(',') {
            return line[..line.len() - line.trim_start().len()].to_owned();
        }
    }
    let close_line = &text[close_line_start..];
    format!("{}    ", &close_line[..close_line.len() - close_line.trim_start().len()])
}

/// The stubs to insert before the class body's closing brace, where to insert
/// them (a byte offset) and the 1-based line the first one will start on.
fn member_insertion(text: &str, body: Range, members: &[MissingMember]) -> (String, usize, u32) {
    let document = ide_text::Document::new(text);
    let close = document.pos_to_byte(body.end).saturating_sub(1);
    let close_line_start = text[..close].rfind('\n').map_or(0, |i| i + 1);
    let indent = member_indent(text, body, close_line_start);
    // `};` on its own line gets the stubs before it; after a member on the same
    // line the stubs open new lines before the brace instead.
    let own_line = text[close_line_start..close].trim().is_empty();
    let mut insertion = String::new();
    if own_line {
        for member in members {
            insertion.push_str(&indent);
            insertion.push_str(&member.member.override_stub());
            insertion.push('\n');
        }
    } else {
        for member in members {
            insertion.push('\n');
            insertion.push_str(&indent);
            insertion.push_str(&member.member.override_stub());
        }
        insertion.push('\n');
    }
    let before = if own_line { close_line_start } else { close };
    let first_newline = if own_line { 0 } else { 1 };
    let line = text[..before].matches('\n').count() as u32 + 1 + first_newline;
    (insertion, before, line)
}

/// Inserts a stub for every base-class virtual member the class at `pos` does
/// not override yet, inside its body before the closing `};`. Returns the file
/// and the 1-based line of the first stub.
pub async fn generate_members(host: &Host, window: &str, doc: DocId, pos: Pos) -> IdeResult<GeneratedDefinition> {
    let project = host.project(window)?;
    let Some((class, missing)) = class_members(host, window, doc, pos).await? else {
        return Err(IdeError::unavailable("Put the caret in a C/C++ class to generate its overrides"));
    };
    if missing.is_empty() {
        return Err(IdeError::unavailable(if class.bases.is_empty() {
            "This class has no base classes to override methods from"
        } else {
            "No base-class virtual methods to override were found"
        }));
    }
    let snapshot = project.documents.snapshot(doc)?;
    let (insertion, offset, line) = member_insertion(&snapshot.text, class.body, &missing);
    let at = ide_text::Document::new(&snapshot.text).byte_to_pos(offset);
    let edit = WorkspaceEdit {
        files: vec![FileEdit::Edit {
            path: snapshot.path.clone(),
            edits: vec![TextEdit { range: Range::new(at, at), new_text: insertion }],
        }],
    };
    apply_workspace_edit(&project.documents, &edit)?;
    Ok(GeneratedDefinition { file: snapshot.path, line })
}

pub fn related_files(host: &Host, window: &str, path: PathBuf) -> IdeResult<Vec<IpcPath>> {
    let project = host.project(window)?;
    let path = if path.is_absolute() { path } else { project.root.join(path) };
    Ok(related(&project, &path).into_iter().map(IpcPath::from).collect())
}

pub(crate) fn register(d: &mut Dispatcher) {
    d.add("cpp_related_files", |c| async move { related_files(&c.host, &c.window, c.arg("path")?) });
    d.add("cpp_generate_definition", |c| async move {
        generate_definition(&c.host, &c.window, c.arg("doc")?, c.arg("pos")?, c.arg("request")?).await
    });
    d.add(
        "cpp_members",
        |c| async move { members_to_generate(&c.host, &c.window, c.arg("doc")?, c.arg("pos")?).await },
    );
    d.add("cpp_generate_members", |c| async move {
        generate_members(&c.host, &c.window, c.arg("doc")?, c.arg("pos")?).await
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HostConfig;
    use std::sync::Arc;

    fn touch(root: &Path, rel: &str) -> PathBuf {
        touch_with(root, rel, "")
    }

    fn touch_with(root: &Path, rel: &str, content: &str) -> PathBuf {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        path
    }

    fn host_with_project(root: &Path) -> crate::Host {
        let base = root.parent().unwrap_or(root);
        let host = crate::Host::new(HostConfig {
            config_dir: base.join("config"),
            data_dir: base.join("data"),
            local_data_dir: base.join("data"),
            languages: Arc::new(ide_language::LanguageRegistry::load(None).unwrap()),
            syntax: Arc::default(),
            sink_for: Box::new(|_| Arc::new(|_| {})),
            owner: |w| w,
            runtime: tokio::runtime::Handle::current(),
            project_environment: false,
        });
        host.open_project("main", root).unwrap();
        host
    }

    /// Opens the project and waits for the initial scan: "Generate Override
    /// Methods" resolves base classes through the index.
    fn host_with_indexed_project(root: &Path) -> crate::Host {
        let host = host_with_project(root);
        assert!(
            host.project("main").unwrap().index.wait_idle(std::time::Duration::from_secs(30)),
            "indexing did not finish"
        );
        host
    }

    fn class_pos(path: &Path, name: &str) -> Pos {
        let text = std::fs::read_to_string(path).unwrap();
        let offset = text.find(name).unwrap() as u32;
        ide_text::Document::new(&text).offset_to_pos(offset)
    }

    fn related_of(host: &crate::Host, path: &Path) -> Vec<PathBuf> {
        related(&host.project("main").unwrap(), path)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn pairs_sources_and_headers_in_conventional_layouts() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("project");
        std::fs::create_dir(&root).unwrap();
        let cpp = touch(&root, "src/greeter.cpp");
        let header = touch(&root, "include/greeter.h");
        let host = host_with_project(&root);
        assert_eq!(related_of(&host, &cpp), vec![header.clone()]);
        assert_eq!(related_of(&host, &header), vec![cpp.clone()]);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn prefers_the_same_directory_and_ignores_unrelated_stems() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("project");
        std::fs::create_dir(&root).unwrap();
        let cpp = touch(&root, "app/greeter.cpp");
        let same = touch(&root, "app/greeter.hpp");
        touch(&root, "src/other.h");
        let host = host_with_project(&root);
        assert_eq!(related_of(&host, &cpp)[0], same);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn generate_definition_appends_the_stub_to_the_source_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("project");
        std::fs::create_dir(&root).unwrap();
        let header = touch_with(
            &root,
            "include/greeter.h",
            "class Greeter {\npublic:\n    std::string greet(const std::string& name) const;\n    Greeter(int);\n};\n",
        );
        let source = touch_with(&root, "src/greeter.cpp", "#include \"greeter.h\"\n");
        let host = host_with_project(&root);
        let project = host.project("main").unwrap();
        let doc = project.documents.open(&header).unwrap().id;
        let pos = {
            let text = std::fs::read_to_string(&header).unwrap();
            let offset = text.find("greet").unwrap() as u32;
            ide_text::Document::new(&text).offset_to_pos(offset)
        };
        let generated = generate_definition(&host, "main", doc, pos, None).await.unwrap();
        assert_eq!(generated.file.to_path_buf(), source);
        assert_eq!(generated.line, 3);
        let text = std::fs::read_to_string(&source).unwrap();
        assert!(
            text.ends_with(
                "#include \"greeter.h\"\n\nstd::string Greeter::greet(const std::string& name) const {\n}\n"
            ),
            "{text}"
        );

        // A constructor has no return type; the second declaration generates its own stub.
        let doc = project.documents.open(&header).unwrap().id;
        let pos = {
            let text = std::fs::read_to_string(&header).unwrap();
            let offset = text.rfind("Greeter(int)").unwrap() as u32;
            ide_text::Document::new(&text).offset_to_pos(offset)
        };
        generate_definition(&host, "main", doc, pos, None).await.unwrap();
        let text = std::fs::read_to_string(&source).unwrap();
        assert!(text.ends_with("Greeter::Greeter(int) {\n}\n"), "{text}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn generate_definition_refuses_a_missing_counterpart_or_bad_position() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("project");
        std::fs::create_dir(&root).unwrap();
        let header = touch_with(&root, "lonely.h", "void ping();\n");
        let host = host_with_project(&root);
        let project = host.project("main").unwrap();
        let doc = project.documents.open(&header).unwrap().id;
        let pos = Pos::new(0, 6);
        let error = generate_definition(&host, "main", doc, pos, None).await.unwrap_err();
        assert!(error.message.contains("No source file corresponds"), "{}", error.message);

        // In a source file the declaration is not a header: refused before anything runs.
        let source = touch_with(&root, "src/ping.cpp", "void ping();\n");
        let doc = project.documents.open(&source).unwrap().id;
        let error = generate_definition(&host, "main", doc, pos, None).await.unwrap_err();
        assert!(error.message.contains("header file"), "{}", error.message);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn files_without_a_cpp_extension_have_no_related_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("project");
        std::fs::create_dir(&root).unwrap();
        let readme = touch(&root, "README.md");
        let host = host_with_project(&root);
        assert!(related_of(&host, &readme).is_empty());
    }

    const ANIMAL_HPP: &str = "\
class Animal {
public:
    virtual ~Animal() = default;
    virtual void speak() const = 0;
    virtual int legs() const { return 4; }
    virtual void eat() &;
    void breathe();
};
";

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn generate_members_inserts_override_stubs_for_the_missing_members() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("project");
        std::fs::create_dir(&root).unwrap();
        let base = touch_with(&root, "include/animal.hpp", ANIMAL_HPP);
        let derived = touch_with(
            &root,
            "include/dog.hpp",
            "#pragma once\n#include \"animal.hpp\"\nclass Dog : public Animal {\npublic:\n    void bark();\n};\n",
        );
        let host = host_with_indexed_project(&root);
        let project = host.project("main").unwrap();
        let doc = project.documents.open(&derived).unwrap().id;
        let pos = class_pos(&derived, "Dog");

        // The chooser list: name, signature, and the base class's file and line.
        let listed = members_to_generate(&host, "main", doc, pos).await.unwrap();
        let names: Vec<&str> = listed.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["speak", "legs", "eat"], "virtual members only, destructor and non-virtual left out");
        assert_eq!(listed[0].signature, "virtual void speak() const = 0");
        assert_eq!(listed[0].class, "Animal");
        assert_eq!(listed[0].file.to_path_buf(), base);
        assert_eq!(listed[0].line, 1);

        // One stub per missing member, inside the body before `};`.
        let doc = project.documents.open(&derived).unwrap().id;
        let generated = generate_members(&host, "main", doc, pos).await.unwrap();
        assert_eq!(generated.file.to_path_buf(), derived);
        assert_eq!(generated.line, 6);
        assert_eq!(
            std::fs::read_to_string(&derived).unwrap(),
            "#pragma once\n#include \"animal.hpp\"\nclass Dog : public Animal {\npublic:\n    void bark();\n    void speak() const override { }\n    int legs() const override { return {}; }\n    void eat() & override { }\n};\n"
        );

        // Nothing is left to generate.
        let doc = project.documents.open(&derived).unwrap().id;
        assert!(members_to_generate(&host, "main", doc, pos).await.unwrap().is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn generate_members_does_not_duplicate_implemented_members() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("project");
        std::fs::create_dir(&root).unwrap();
        touch_with(&root, "include/animal.hpp", ANIMAL_HPP);
        let derived = touch_with(
            &root,
            "include/dog.hpp",
            "class Dog : public Animal {\npublic:\n    void speak() const override;\n    void eat() & override;\n};\n",
        );
        let host = host_with_indexed_project(&root);
        let project = host.project("main").unwrap();
        let doc = project.documents.open(&derived).unwrap().id;
        let generated = generate_members(&host, "main", doc, class_pos(&derived, "Dog")).await.unwrap();
        assert_eq!(generated.line, 5);
        assert_eq!(
            std::fs::read_to_string(&derived).unwrap(),
            "class Dog : public Animal {\npublic:\n    void speak() const override;\n    void eat() & override;\n    int legs() const override { return {}; }\n};\n"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn base_classes_in_c_style_headers_are_read_through_includes() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("project");
        std::fs::create_dir(&root).unwrap();
        // `.h` is indexed as C, so `Base` has no class symbol: the quoted include
        // tells the feature which file to read instead.
        touch_with(&root, "include/base.h", "class Base {\npublic:\n    virtual void run() = 0;\n};\n");
        let derived = touch_with(&root, "include/impl.hpp", "#include \"base.h\"\nclass Impl : public Base {\n};\n");
        let host = host_with_indexed_project(&root);
        let project = host.project("main").unwrap();
        assert!(base_symbol(&project, "Base").is_none(), "a C header has no C++ class symbol");
        let doc = project.documents.open(&derived).unwrap().id;
        let generated = generate_members(&host, "main", doc, class_pos(&derived, "Impl")).await.unwrap();
        assert_eq!(generated.line, 3);
        assert_eq!(
            std::fs::read_to_string(&derived).unwrap(),
            "#include \"base.h\"\nclass Impl : public Base {\n    void run() override { }\n};\n"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_class_without_a_base_reports_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("project");
        std::fs::create_dir(&root).unwrap();
        let plain = touch_with(&root, "plain.hpp", "class Plain {\npublic:\n    void run();\n};\n");
        let free = touch_with(&root, "free.hpp", "int add(int a, int b);\n");
        let host = host_with_indexed_project(&root);
        let project = host.project("main").unwrap();
        let doc = project.documents.open(&plain).unwrap().id;
        let pos = class_pos(&plain, "Plain");
        assert!(members_to_generate(&host, "main", doc, pos).await.unwrap().is_empty());
        let error = generate_members(&host, "main", doc, pos).await.unwrap_err();
        assert!(error.message.contains("no base classes"), "{}", error.message);

        // Outside a class there is nothing to list or generate.
        let doc = project.documents.open(&free).unwrap().id;
        let error = members_to_generate(&host, "main", doc, Pos::new(0, 0)).await.unwrap_err();
        assert!(error.message.contains("caret in a C/C++ class"), "{}", error.message);
        let error = generate_members(&host, "main", doc, Pos::new(0, 0)).await.unwrap_err();
        assert!(error.message.contains("caret in a C/C++ class"), "{}", error.message);
    }
}
