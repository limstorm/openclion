//! C/C++ source ↔ header pairing: CLion's "Go to Related Symbol" (switch
//! between a translation unit and the header that declares it, and back).
//!
//! The conventional layouts are checked directly (fast, deterministic), and the
//! project index is the fallback for unusual trees (`src/greeter.cpp` next to
//! `include/greeter.h` is the common case that the conventions cover).
use crate::rpc::Dispatcher;
use crate::{Host, Project};
use ide_core::{IdeResult, IpcPath};
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

pub fn related_files(host: &Host, window: &str, path: PathBuf) -> IdeResult<Vec<IpcPath>> {
    let project = host.project(window)?;
    let path = if path.is_absolute() { path } else { project.root.join(path) };
    Ok(related(&project, &path).into_iter().map(IpcPath::from).collect())
}

pub(crate) fn register(d: &mut Dispatcher) {
    d.add("cpp_related_files", |c| async move { related_files(&c.host, &c.window, c.arg("path")?) });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HostConfig;
    use std::sync::Arc;

    fn touch(root: &Path, rel: &str) -> PathBuf {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "").unwrap();
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
    async fn files_without_a_cpp_extension_have_no_related_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("project");
        std::fs::create_dir(&root).unwrap();
        let readme = touch(&root, "README.md");
        let host = host_with_project(&root);
        assert!(related_of(&host, &readme).is_empty());
    }
}
