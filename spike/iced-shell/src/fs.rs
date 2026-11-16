//! Filesystem access for the workbench, using the same crate the web shell calls
//! (`ide-vfs::read_dir`, which already carries gitignore/exclude flags per entry).

use std::path::{Path, PathBuf};

/// One child of a directory, in the shape the tree renders.
#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub directory: bool,
    /// Matched by `.gitignore` or the exclude settings: IDEA dims the label.
    pub ignored: bool,
    /// Explicitly excluded: IDEA dims the icon as well.
    pub excluded: bool,
}

/// The same excludes the web shell passes: the default file-exclude list plus IDEA's ignored
/// names (`.git`, `.idea`, `node_modules`, `target`, …), so both trees agree on what to hide.
fn excludes() -> Vec<String> {
    ide_core::settings::Settings::default().file_excludes()
}

/// Directories first, then case-insensitive names — the order IDEA lists a tree in.
pub fn read_dir(path: &Path) -> Result<Vec<Entry>, String> {
    let entries: Vec<Entry> = ide_vfs::read_dir(path, &excludes())
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|entry| Entry {
            name: entry.name,
            path: entry.path.to_path_buf(),
            directory: matches!(entry.kind, ide_vfs::EntryKind::Dir),
            ignored: entry.ignored,
            excluded: entry.excluded,
        })
        .collect();

    // `ide_vfs::read_dir` already sorts directories first and then naturally (IDEA's order),
    // so the order the web tree shows is kept as-is.
    Ok(entries)
}

/// Reads a file as text. Anything above [`MAX_FILE`] is refused rather than freezing the UI.
pub fn read_text(path: &Path) -> Result<String, String> {
    const MAX_FILE: u64 = 8 * 1024 * 1024;

    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_FILE {
        return Err(format!(
            "{} is {:.1} MB, too large for the prototype editor",
            display_name(path),
            metadata.len() as f64 / 1_048_576.0
        ));
    }
    std::fs::read_to_string(path).map_err(|error| error.to_string())
}

pub fn display_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned())
}

/// The language name shown in the status bar, from the embedded `languages/*.toml` registry —
/// the same definitions and detection order the web shell uses.
pub fn language_of(path: &Path) -> String {
    let registry = registry();
    let id = registry.detect(path, None);
    registry.get(&id).map_or_else(|| "Plain text".to_owned(), |config| config.name)
}

fn registry() -> &'static ide_language::LanguageRegistry {
    static REGISTRY: std::sync::OnceLock<ide_language::LanguageRegistry> = std::sync::OnceLock::new();
    REGISTRY
        .get_or_init(|| ide_language::LanguageRegistry::load(None).expect("the embedded language definitions parse"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_lookup_comes_from_the_embedded_definitions() {
        assert_eq!(language_of(Path::new("/w/keys.go")), "Go");
        assert_eq!(language_of(Path::new("/w/CMakeLists.txt")), "CMake");
        assert_eq!(language_of(Path::new("/w/main.cpp")), "C++");
        assert_eq!(language_of(Path::new("/w/README.md")), "Markdown");
        assert!(!language_of(Path::new("/w/unknown.zzz")).is_empty());
    }

    #[test]
    fn directories_sort_before_files_then_case_insensitively() {
        let dir = std::env::temp_dir().join("iced-shell-sort-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("zeta")).unwrap();
        std::fs::create_dir_all(dir.join("Alpha")).unwrap();
        std::fs::write(dir.join("b.txt"), b"b").unwrap();
        std::fs::write(dir.join("A.txt"), b"a").unwrap();

        let entries = read_dir(&dir).unwrap();
        let order: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(order, vec!["Alpha", "zeta", "A.txt", "b.txt"]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn display_name_falls_back_to_the_path() {
        assert_eq!(display_name(Path::new("/w/src/keys.go")), "keys.go");
        assert_eq!(display_name(Path::new("/")), "/");
    }
}
