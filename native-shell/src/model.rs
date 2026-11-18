//! The native shell's state: the workspace tree, the open editors, the
//! visible tool windows and the small terminal, all held in signals so the
//! UI updates only where something changed.

use std::{
    fs,
    path::{Path, PathBuf},
};

use nsh_core::prelude::*;

const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".cargo-home",
    ".tools",
    ".sysroot",
    "dist",
    "test-results",
    ".nsh",
    ".cache",
    ".idea",
    "__pycache__",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftTool {
    Project,
    Commit,
    Structure,
    Bookmarks,
    Search,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightTool {
    Cmake,
    Notifications,
    Agents,
    Database,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BottomTool {
    Debug,
    Terminal,
    Problems,
    Run,
    Services,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tab {
    pub path: PathBuf,
    pub title: String,
    pub text: String,
    pub language: String,
    pub modified: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FsNode {
    pub path: PathBuf,
    pub name: String,
    pub dir: bool,
    pub children: Vec<FsNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    pub root: FsNode,
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub path: String,
    pub name: String,
    pub dir: bool,
    pub depth: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub path: PathBuf,
    pub display: String,
    pub line: usize,
    pub text: String,
}

#[derive(Clone, Copy)]
pub struct State {
    pub dark: Signal<bool>,
    pub root: Signal<PathBuf>,
    pub tree: Stored<Tree>,
    pub expanded: Signal<Vec<String>>,
    pub selected: Signal<Option<String>>,
    pub tabs: Signal<Vec<Tab>>,
    pub active: Signal<usize>,
    pub active_path: Signal<PathBuf>,
    pub editor_seed: Signal<String>,
    pub left_tool: Signal<LeftTool>,
    pub right_tool: Signal<RightTool>,
    pub bottom_tool: Signal<Option<BottomTool>>,
    pub quick_open: Signal<bool>,
    pub quick_query: Signal<String>,
    pub project_menu: Signal<bool>,
    pub cursor: Signal<(u32, u32)>,
    pub status: Signal<String>,
    pub search_query: Signal<String>,
    pub search_results: Signal<Vec<SearchHit>>,
    pub searching: Signal<bool>,
    pub cmake: Signal<Option<ide_cmake::CmakeModel>>,
    pub vcs: Signal<Vec<crate::backend::VcsRow>>,
    pub vcs_branch: Signal<Option<String>>,
    pub vcs_loading: Signal<bool>,
    pub editor: nsh_core::element::ComponentRef<nsh_base::editor::Editor>,
    pub lsp: Stored<Option<crate::lsp::Session>>,
    pub diagnostics: Signal<Vec<nsh_base::editor::EditorDiagnostic>>,
    pub diagnostics_cache: Stored<crate::lsp::DiagnosticsCache>,
    pub diagnostics_rev: Signal<u32>,
    pub lsp_status: Signal<String>,
    pub debug_bridge: Stored<Option<crate::debug::DebugBridge>>,
    pub debug_active: Signal<bool>,
    pub debug_stopped: Signal<bool>,
    pub debug_session: Signal<Option<ide_core::DebugSessionId>>,
    pub debug_thread: Signal<i64>,
    pub debug_frames: Signal<Vec<crate::debug::DebugFrame>>,
    pub debug_vars: Signal<Vec<crate::debug::DebugVar>>,
    pub debug_status: Signal<String>,
    pub breakpoints: Stored<std::collections::BTreeMap<PathBuf, Vec<u32>>>,
    pub breakpoints_rev: Signal<u32>,
    pub debug_line: Signal<Option<(PathBuf, u32)>>,
    pub terminal_ref: nsh_core::element::ComponentRef<nsh_base::Terminal>,
    pub terminal_session: Stored<Option<crate::terminal::Session>>,
    pub terminal_started: Signal<bool>,
    pub terminal_buffer: Stored<String>,
    pub terminal_rev: Signal<u32>,
    pub settings: Signal<bool>,
    pub toast: Signal<Option<String>>,
    pub left_width: Signal<f32>,
    pub right_width: Signal<f32>,
    pub bottom_height: Signal<f32>,
    pub show_left: Signal<bool>,
    pub show_right: Signal<bool>,
    pub show_bottom: Signal<bool>,
}

impl State {
    pub fn new() -> Self {
        let root = default_workspace();
        let tree = Tree::load(&root);
        let main = root.join("src/main.cpp");
        let initial = if main.exists() {
            main
        } else {
            tree.files
                .iter()
                .find(|p| {
                    matches!(
                        p.extension().and_then(|e| e.to_str()),
                        Some("md") | Some("txt") | Some("rs") | Some("toml")
                    )
                })
                .cloned()
                .unwrap_or_else(|| root.join("README.md"))
        };
        let text = fs::read_to_string(&initial).unwrap_or_default();
        let tab = Tab {
            title: file_name(&initial),
            language: language_of(&initial),
            path: initial.clone(),
            text,
            modified: false,
        };
        let expanded = vec![
            initial.parent().unwrap_or(&root).to_string_lossy().to_string(),
            root.join("build").to_string_lossy().to_string(),
            root.join("src").to_string_lossy().to_string(),
            root.join("src/util").to_string_lossy().to_string(),
        ];
        Self {
            dark: signal(false),
            root: signal(root.clone()),
            tree: stored(tree),
            expanded: signal(expanded),
            selected: signal(Some(initial.to_string_lossy().to_string())),
            tabs: signal(vec![tab]),
            active: signal(0),
            active_path: signal(initial.clone()),
            editor_seed: signal(fs::read_to_string(&initial).unwrap_or_default()),
            left_tool: signal(LeftTool::Project),
            right_tool: signal(RightTool::Cmake),
            bottom_tool: signal(None),
            quick_open: signal(false),
            quick_query: signal(String::new()),
            project_menu: signal(false),
            cursor: signal((1, 1)),
            status: signal(String::new()),
            search_query: signal(String::new()),
            search_results: signal(Vec::new()),
            searching: signal(false),
            cmake: signal(None),
            vcs: signal(Vec::new()),
            vcs_branch: signal(None),
            vcs_loading: signal(false),
            editor: component_ref::<nsh_base::editor::Editor>(),
            lsp: stored(None),
            diagnostics: signal(Vec::new()),
            diagnostics_cache: stored(std::collections::HashMap::new()),
            diagnostics_rev: signal(0),
            lsp_status: signal(String::new()),
            debug_bridge: stored(None),
            debug_active: signal(false),
            debug_stopped: signal(false),
            debug_session: signal(None),
            debug_thread: signal(0),
            debug_frames: signal(Vec::new()),
            debug_vars: signal(Vec::new()),
            debug_status: signal(String::new()),
            breakpoints: stored(std::collections::BTreeMap::new()),
            breakpoints_rev: signal(0),
            debug_line: signal(None),
            terminal_ref: component_ref::<nsh_base::Terminal>(),
            terminal_session: stored(None),
            terminal_started: signal(false),
            terminal_buffer: stored(String::new()),
            terminal_rev: signal(0),
            settings: signal(false),
            toast: signal(None),
            left_width: signal(234.0),
            right_width: signal(294.0),
            bottom_height: signal(220.0),
            show_left: signal(true),
            show_right: signal(true),
            show_bottom: signal(false),
        }
    }

    pub fn rows(&self) -> Vec<Row> {
        let expanded = self.expanded.get();
        let mut rows = Vec::new();
        self.tree.with(|tree| collect_rows(&tree.root, 0, &expanded, &mut rows));
        rows
    }

    pub fn files(&self) -> Vec<PathBuf> {
        self.tree.with(|tree| tree.files.clone())
    }

    pub fn root_name(&self) -> String {
        file_name(&self.root.get())
    }

    pub fn active_tab(&self) -> Option<Tab> {
        let tabs = self.tabs.get();
        tabs.get(self.active.get()).cloned()
    }

    pub fn toggle_dir(&self, path: &str) {
        let mut expanded = self.expanded.get();
        if let Some(index) = expanded.iter().position(|p| p == path) {
            expanded.remove(index);
        } else {
            expanded.push(path.to_owned());
        }
        self.expanded.set(expanded);
    }

    pub fn is_expanded(&self, path: &str) -> bool {
        self.expanded.get().iter().any(|p| p == path)
    }

    pub fn open_path(&self, path: PathBuf) {
        if path.is_dir() {
            self.toggle_dir(&path.to_string_lossy());
            return;
        }
        let path_str = path.to_string_lossy().to_string();
        self.selected.set(Some(path_str));
        let tabs = self.tabs.get();
        if let Some(index) = tabs.iter().position(|t| t.path == path) {
            let text = tabs[index].text.clone();
            self.active.set(index);
            self.active_path.set(path.clone());
            self.editor_seed.set(text);
            self.status.set(format!("{}", path.display()));
            return;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            self.status.set(format!("Cannot read {}", path.display()));
            return;
        };
        let mut tabs = tabs;
        tabs.push(Tab {
            title: file_name(&path),
            language: language_of(&path),
            path: path.clone(),
            text,
            modified: false,
        });
        let active = tabs.len() - 1;
        self.active_path.set(path.clone());
        self.editor_seed.set(tabs[active].text.clone());
        self.tabs.set(tabs);
        self.active.set(active);
    }

    pub fn activate_tab(&self, index: usize) {
        if index < self.tabs.get().len() {
            self.active.set(index);
            if let Some(tab) = self.active_tab() {
                self.selected.set(Some(tab.path.to_string_lossy().to_string()));
                self.active_path.set(tab.path.clone());
                self.editor_seed.set(tab.text.clone());
            }
        }
    }

    pub fn close_tab(&self, index: usize) {
        let mut tabs = self.tabs.get();
        if index >= tabs.len() {
            return;
        }
        tabs.remove(index);
        let active = self.active.get();
        let next = if tabs.is_empty() {
            0
        } else if active > index || active >= tabs.len() {
            active.saturating_sub(1).min(tabs.len() - 1)
        } else {
            active.min(tabs.len() - 1)
        };
        self.tabs.set(tabs);
        self.active.set(next);
        if let Some(tab) = self.active_tab() {
            self.active_path.set(tab.path.clone());
            self.editor_seed.set(tab.text.clone());
        }
    }

    pub fn update_active_text(&self, text: String) {
        let active = self.active.get();
        let mut tabs = self.tabs.get();
        if let Some(tab) = tabs.get_mut(active) {
            if tab.text != text {
                tab.text = text;
                tab.modified = true;
                self.tabs.set(tabs);
            }
        }
    }

    pub fn save_active(&self) {
        let active = self.active.get();
        let mut tabs = self.tabs.get();
        if let Some(tab) = tabs.get_mut(active) {
            match fs::write(&tab.path, &tab.text) {
                Ok(()) => {
                    tab.modified = false;
                    self.status.set(format!("Saved {}", tab.path.display()));
                    self.toast.set(Some(format!("Saved {}", tab.title)));
                }
                Err(error) => {
                    self.status.set(format!("Save failed: {error}"));
                    self.toast.set(Some(format!("Save failed: {error}")));
                }
            }
        }
        self.tabs.set(tabs);
    }

    pub fn reload_workspace(&self, root: PathBuf) {
        let tree = Tree::load(&root);
        self.root.set(root);
        self.tree.replace(tree);
        self.expanded.set(Vec::new());
        self.tabs.set(Vec::new());
        self.active.set(0);
        self.active_path.set(PathBuf::new());
        self.editor_seed.set(String::new());
        self.selected.set(None);
        self.status.set("Workspace opened".to_owned());
        self.diagnostics_cache.with(|cache| cache.clear());
        self.diagnostics_rev.update(|rev| *rev += 1);
        self.lsp.replace(None);
        crate::lsp::start(*self);
        self.refresh_cmake();
        self.refresh_vcs();
    }
}

impl Tree {
    pub fn load(root: &Path) -> Self {
        let mut files = Vec::new();
        let node = read_node(root, 0, &mut files);
        Self { root: node, files }
    }
}

fn read_node(path: &Path, depth: usize, files: &mut Vec<PathBuf>) -> FsNode {
    let name = file_name(path);
    if !path.is_dir() {
        files.push(path.to_path_buf());
    }
    let mut children = Vec::new();
    if path.is_dir() && depth < 12 {
        // A configured CMake build directory is generated output: IDEA keeps it
        // out of the project tree, and it can hold hundreds of thousands of files.
        let generated = path.join("CMakeCache.txt").is_file();
        if let Ok(entries) = fs::read_dir(path) {
            let mut dirs = Vec::new();
            let mut rest = Vec::new();
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if SKIP_DIRS.contains(&name.as_str()) || name == "compile_commands.json" {
                    continue;
                }
                if generated && name != "CMakeCache.txt" {
                    continue;
                }
                let child = read_node(&entry.path(), depth + 1, files);
                if child.dir {
                    dirs.push(child);
                } else {
                    rest.push(child);
                }
            }
            dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            rest.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            children.extend(dirs);
            children.extend(rest);
        }
    }
    FsNode { path: path.to_path_buf(), name, dir: path.is_dir(), children }
}

fn collect_rows(node: &FsNode, depth: usize, expanded: &[String], rows: &mut Vec<Row>) {
    for child in &node.children {
        let path = child.path.to_string_lossy().to_string();
        rows.push(Row { path: path.clone(), name: child.name.clone(), dir: child.dir, depth: depth + 1 });
        if child.dir && expanded.iter().any(|p| p == &path) {
            collect_rows(child, depth + 1, expanded, rows);
        }
    }
}

pub fn default_workspace() -> PathBuf {
    if let Ok(path) = std::env::var("OPENCLION_WORKSPACE") {
        return PathBuf::from(path);
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for candidate in [cwd.join("native-shell/demo/acme-api"), cwd.join("demo/acme-api"), cwd.join("../demo/acme-api")] {
        if candidate.exists() {
            return candidate;
        }
    }
    cwd
}

pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

pub fn language_of(path: &Path) -> String {
    let name = file_name(path).to_lowercase();
    let ext = name.rsplit('.').next().unwrap_or("");
    match ext {
        "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => "cpp",
        "rs" => "rust",
        "go" => "go",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "json" | "jsonc" => "json",
        "toml" => "toml",
        "md" | "markdown" => "markdown",
        "html" => "html",
        "css" => "css",
        "sh" | "bash" | "zsh" => "bash",
        "yaml" | "yml" => "yaml",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "gradle" => "groovy",
        "sql" => "sql",
        "xml" => "xml",
        _ => "text",
    }
    .to_owned()
}
