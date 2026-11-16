//! Ignore rules shared by directory listing, walking and watching:
//! user exclude globs plus layered `.gitignore` / `.ignore` files.

use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Exclude globs from settings (`files.exclude`), e.g. `**/node_modules`.
///
/// Patterns are matched against paths relative to a base directory; a path is
/// excluded when it or any of its ancestors (below the base) matches. Patterns
/// without a `/` match at any depth, like gitignore; a leading `/` anchors at the base.
#[derive(Clone, Debug)]
pub struct ExcludeMatcher {
    set: GlobSet,
}

impl ExcludeMatcher {
    pub fn new(patterns: &[String]) -> Self {
        let mut builder = GlobSetBuilder::new();
        for raw in patterns {
            let pattern = raw.trim().trim_end_matches('/');
            if pattern.is_empty() {
                continue;
            }
            // A leading `/` anchors the pattern at the base, like gitignore.
            let anchored = pattern.starts_with('/');
            let pattern = pattern.trim_start_matches("./").trim_start_matches('/');
            if pattern.is_empty() {
                continue;
            }
            let mut variants = vec![pattern.to_owned()];
            if !anchored && !pattern.contains('/') {
                variants.push(format!("**/{pattern}"));
            }
            for v in variants {
                match Glob::new(&v) {
                    Ok(glob) => {
                        builder.add(glob);
                    }
                    Err(e) => tracing::warn!("invalid exclude pattern {raw:?}: {e}"),
                }
            }
        }
        let set = builder.build().unwrap_or_else(|e| {
            tracing::warn!("invalid exclude patterns: {e}");
            GlobSet::empty()
        });
        Self { set }
    }

    /// Whether `rel` itself matches (ancestors are not consulted).
    pub fn is_match(&self, rel: &Path) -> bool {
        !self.set.is_empty() && self.set.is_match(rel)
    }

    /// `rel` is relative to the base directory.
    pub fn is_excluded(&self, rel: &Path) -> bool {
        if self.set.is_empty() {
            return false;
        }
        rel.ancestors().filter(|p| !p.as_os_str().is_empty()).any(|p| self.set.is_match(p))
    }
}

/// Walks up from `start` looking for a directory containing `.git`.
pub(crate) fn find_repo_root(start: &Path) -> Option<PathBuf> {
    start.ancestors().find(|dir| dir.join(".git").exists()).map(Path::to_path_buf)
}

/// Layered gitignore matching: per-directory `.gitignore` / `.ignore` from the
/// path's directory up to the repository root (or the file system root when
/// not in a repository), then `.git/info/exclude` and the global excludes file.
pub(crate) struct GitignoreStack {
    repo_root: Option<PathBuf>,
    /// `.git/info/exclude` and the global excludes file, rooted at the repo root.
    repo_level: RwLock<Vec<Gitignore>>,
    cache: Mutex<HashMap<PathBuf, Option<Arc<Gitignore>>>>,
    /// One stack per nested repository root, shared by every directory below it.
    nested_stacks: Mutex<HashMap<PathBuf, Arc<GitignoreStack>>>,
    /// The nested stack (if any) each parent directory resolves to.
    nested: Mutex<HashMap<PathBuf, Option<Arc<GitignoreStack>>>>,
}

impl GitignoreStack {
    pub fn new(near: &Path) -> Self {
        let repo_root = find_repo_root(near);
        let repo_level = RwLock::new(Self::load_repo_level(repo_root.as_deref()));
        Self {
            repo_root,
            repo_level,
            cache: Mutex::default(),
            nested_stacks: Mutex::default(),
            nested: Mutex::default(),
        }
    }

    fn load_repo_level(repo_root: Option<&Path>) -> Vec<Gitignore> {
        let mut repo_level = Vec::new();
        if let Some(root) = repo_root {
            let mut builder = GitignoreBuilder::new(root);
            let exclude = gix::discover(root)
                .ok()
                .map(|r| r.common_dir().join("info/exclude"))
                .unwrap_or_else(|| root.join(".git/info/exclude"));
            if exclude.is_file() {
                builder.add(exclude);
            }
            if let Ok(gi) = builder.build()
                && !gi.is_empty()
            {
                repo_level.push(gi);
            }
            let (global, err) = GitignoreBuilder::new(root).build_global();
            if let Some(err) = err {
                tracing::debug!("global gitignore: {err}");
            }
            if !global.is_empty() {
                repo_level.push(global);
            }
        }
        repo_level
    }

    /// Re-reads `.git/info/exclude` and the global excludes file.
    pub fn reload_repo_level(&self) {
        self.nested.lock().clear();
        self.nested_stacks.lock().clear();
        *self.repo_level.write() = Self::load_repo_level(self.repo_root.as_deref());
    }

    /// The shared nested stacks, so invalidations reach their own caches too.
    fn shared_stacks(&self) -> Vec<Arc<GitignoreStack>> {
        self.nested_stacks.lock().values().cloned().collect()
    }

    /// Forgets the cached rules of `dir` and every directory below it.
    pub fn invalidate_tree(&self, dir: &Path) {
        for stack in self.shared_stacks() {
            stack.invalidate_tree(dir);
        }
        {
            let mut nested = self.nested.lock();
            // Directory membership can change (a nested repository appears or goes away);
            // entries below `dir` were resolved against the old tree.
            if nested.contains_key(dir) {
                nested.retain(|k, _| !k.starts_with(dir));
            }
        }
        let mut cache = self.cache.lock();
        // Lookups always populate every ancestor, so no key below `dir` exists without `dir`.
        if cache.contains_key(dir) {
            cache.retain(|k, _| !k.starts_with(dir));
        }
    }

    pub fn repo_root(&self) -> Option<&Path> {
        self.repo_root.as_deref()
    }

    /// Forgets the cached rules of `dir` (after its `.gitignore` changed).
    pub fn invalidate(&self, dir: &Path) {
        for stack in self.shared_stacks() {
            stack.invalidate(dir);
        }
        self.cache.lock().remove(dir);
    }

    fn matcher_for(&self, dir: &Path) -> Option<Arc<Gitignore>> {
        if let Some(cached) = self.cache.lock().get(dir) {
            return cached.clone();
        }
        let mut builder = GitignoreBuilder::new(dir);
        let mut any = false;
        for name in [".gitignore", ".ignore"] {
            let file = dir.join(name);
            if file.is_file() {
                any = true;
                if let Some(err) = builder.add(&file) {
                    tracing::debug!("{}: {err}", file.display());
                }
            }
        }
        let matcher = if any {
            match builder.build() {
                Ok(gi) if !gi.is_empty() => Some(Arc::new(gi)),
                Ok(_) => None,
                Err(e) => {
                    tracing::debug!("{}: {e}", dir.display());
                    None
                }
            }
        } else {
            None
        };
        self.cache.lock().insert(dir.to_path_buf(), matcher.clone());
        matcher
    }

    /// The stack for the nested repository `parent` belongs to, if that is not this
    /// stack's own repository.
    ///
    /// One stack per *directory* used to be built instead: every `Self::new` runs
    /// `gix::discover` and rebuilds the global excludes matcher, so a vendored tree
    /// (this project has five nested repositories with thousands of directories)
    /// spent seconds to minutes of CPU in the watcher's first walk. The rules are the
    /// same for every directory of a repository, and the stack keeps its own
    /// per-directory `.gitignore` cache, so one stack per repository root suffices.
    fn nested_stack(&self, parent: &Path) -> Option<Arc<GitignoreStack>> {
        let mut nested = self.nested.lock();
        if let Some(cached) = nested.get(parent) {
            return cached.clone();
        }
        let found = find_repo_root(parent).filter(|root| self.repo_root.as_ref() != Some(root)).map(|root| {
            let mut stacks = self.nested_stacks.lock();
            if let Some(stack) = stacks.get(&root) {
                return stack.clone();
            }
            let stack = Arc::new(Self::new(&root));
            stacks.insert(root, stack.clone());
            stack
        });
        nested.insert(parent.to_path_buf(), found.clone());
        found
    }

    /// Whether `path` (absolute) is ignored. `is_dir` refers to `path` itself.
    pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        // A project's watcher can span independent nested repositories. Their local
        // excludes and ignore ancestry must stop at their own repository boundary.
        if let Some(parent) = path.parent()
            && let Some(nested) = self.nested_stack(parent)
        {
            return nested.is_ignored(path, is_dir);
        }
        if self.repo_root.as_deref() == Some(path) {
            return false;
        }
        for dir in path.ancestors().skip(1) {
            if let Some(m) = self.matcher_for(dir)
                && let Ok(rel) = path.strip_prefix(dir)
            {
                match m.matched_path_or_any_parents(rel, is_dir) {
                    Match::Ignore(_) => return true,
                    Match::Whitelist(_) => return false,
                    Match::None => {}
                }
            }
            if self.repo_root.as_deref() == Some(dir) {
                break;
            }
        }
        if let Some(root) = &self.repo_root
            && let Ok(rel) = path.strip_prefix(root)
            && !rel.as_os_str().is_empty()
        {
            for gi in self.repo_level.read().iter() {
                match gi.matched_path_or_any_parents(rel, is_dir) {
                    Match::Ignore(_) => return true,
                    Match::Whitelist(_) => return false,
                    Match::None => {}
                }
            }
        }
        false
    }

    /// Like [`is_ignored`], but only checks whether `path` itself matches —
    /// ignored ancestors do not count. Lets a directory like `target/` (itself
    /// ignored) still emit its own create/remove so the tree can show it
    /// appearing marked as ignored, while files inside it stay filtered out.
    pub fn is_ignored_self(&self, path: &Path, is_dir: bool) -> bool {
        if let Some(parent) = path.parent()
            && let Some(nested) = self.nested_stack(parent)
        {
            return nested.is_ignored_self(path, is_dir);
        }
        if self.repo_root.as_deref() == Some(path) {
            return false;
        }
        for dir in path.ancestors().skip(1) {
            if let Some(m) = self.matcher_for(dir)
                && let Ok(rel) = path.strip_prefix(dir)
            {
                match m.matched(rel, is_dir) {
                    Match::Ignore(_) => return true,
                    Match::Whitelist(_) => return false,
                    Match::None => {}
                }
            }
            if self.repo_root.as_deref() == Some(dir) {
                break;
            }
        }
        if let Some(root) = &self.repo_root
            && let Ok(rel) = path.strip_prefix(root)
            && !rel.as_os_str().is_empty()
        {
            for gi in self.repo_level.read().iter() {
                match gi.matched(rel, is_dir) {
                    Match::Ignore(_) => return true,
                    Match::Whitelist(_) => return false,
                    Match::None => {}
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_stack_per_nested_repository() {
        let root = std::env::temp_dir().join(format!("ide-vfs-nested-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let nested = root.join("vendor");
        std::fs::create_dir_all(nested.join(".git")).unwrap();
        std::fs::write(nested.join(".gitignore"), "ignored/\n").unwrap();
        std::fs::create_dir_all(nested.join("ignored")).unwrap();
        for i in 0..50 {
            std::fs::create_dir_all(nested.join(format!("pkg{i}/src"))).unwrap();
        }
        let stack = GitignoreStack::new(&root);
        for i in 0..50 {
            assert!(!stack.is_ignored(&nested.join(format!("pkg{i}/src")), true));
            assert!(!stack.is_ignored(&nested.join(format!("pkg{i}/src/main.rs")), false));
        }
        // The nested repository's own rules still win.
        assert!(stack.is_ignored(&nested.join("ignored"), true));
        // Every directory below the nested repository shares its one stack; building one
        // per directory rediscovered the repository and rebuilt the global matcher each
        // time, which is what made the first watch walk crawl.
        assert_eq!(stack.nested_stacks.lock().len(), 1);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn exclude_patterns() {
        let m = ExcludeMatcher::new(&["**/node_modules".into(), "target".into(), "docs/*.tmp".into(), "[".into()]);
        assert!(m.is_excluded(Path::new("node_modules")));
        assert!(m.is_excluded(Path::new("a/node_modules/x/y.js")));
        assert!(m.is_excluded(Path::new("target")));
        assert!(m.is_excluded(Path::new("crates/x/target/debug")));
        assert!(m.is_excluded(Path::new("docs/a.tmp")));
        assert!(!m.is_excluded(Path::new("src/main.rs")));
        assert!(!m.is_excluded(Path::new("targets")));
        assert!(!ExcludeMatcher::new(&[]).is_excluded(Path::new("x")));
        // A leading `/` anchors at the base (Project Structure › Excluded folders).
        let anchored = ExcludeMatcher::new(&["/build".into()]);
        assert!(anchored.is_excluded(Path::new("build/out.o")));
        assert!(!anchored.is_excluded(Path::new("web/build/out.o")));
    }
}
