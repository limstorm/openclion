//! C/C++ source ↔ header pairing: CLion's "Go to Related Symbol" (switch
//! between a translation unit and the header that declares it, and back).
//!
//! The conventional layouts are checked directly (fast, deterministic), and the
//! project index is the fallback for unusual trees (`src/greeter.cpp` next to
//! `include/greeter.h` is the common case that the conventions cover).
use crate::rpc::Dispatcher;
use crate::{Host, Project, apply_workspace_edit};
use ide_core::{DocId, FileEdit, IdeError, IdeResult, IpcPath, Pos, Range, RequestId, TextEdit, WorkspaceEdit};
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
}
