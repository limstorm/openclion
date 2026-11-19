//! The OpenCLion backend, reused directly: CMake's project model, git status
//! and the ripgrep search engine from `crates/ide-*`. Everything here runs off
//! the engine thread and reports through nsh signals.

use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
};

use ide_core::CancellationToken;
use ide_search::{FileMatches, SearchQuery, SearchScope};
use ide_vcs::{ChangeKind, Repo};
use nsh_core::prelude::*;

use crate::model::{SearchHit, State};

#[derive(Debug, Clone, PartialEq)]
pub struct VcsRow {
    pub path: PathBuf,
    /// Relative path, as git reports it.
    pub rel: String,
    /// `M`, `A`, `D`, `R`, `U`, `C`.
    pub letter: &'static str,
    /// Staged changes are green, unstaged blue, conflicts red.
    pub staged: bool,
    pub conflicted: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VcsSnapshot {
    pub rows: Vec<VcsRow>,
    pub branch: Option<String>,
    pub operation: Option<String>,
}

impl VcsSnapshot {
    pub fn load(root: &Path) -> Self {
        let Ok(Some(repo)) = Repo::discover(root) else {
            return Self { rows: Vec::new(), branch: None, operation: None };
        };
        let branch = repo.branch_info().ok().and_then(|info| info.head);
        let operation = repo.operation().ok().flatten();
        let rows = repo
            .status()
            .unwrap_or_default()
            .into_iter()
            .map(|status| {
                let kind = status.staged.or(status.unstaged);
                let conflicted = kind == Some(ChangeKind::Conflicted);
                VcsRow {
                    path: status.path.into_path_buf(),
                    rel: status.rel_path,
                    letter: letter_of(kind),
                    staged: status.staged.is_some(),
                    conflicted,
                }
            })
            .collect();
        Self { rows, branch, operation }
    }
}

fn letter_of(kind: Option<ChangeKind>) -> &'static str {
    match kind {
        Some(ChangeKind::Added) => "A",
        Some(ChangeKind::Modified) => "M",
        Some(ChangeKind::Deleted) => "D",
        Some(ChangeKind::Renamed) => "R",
        Some(ChangeKind::Untracked) => "U",
        Some(ChangeKind::Conflicted) => "C",
        Some(ChangeKind::Ignored) | None => " ",
    }
}

/// The executable the Run/Debug buttons use: the selected target, else the
/// first one of the CMake model.
pub fn select_executable<'a>(
    model: &'a ide_cmake::CmakeModel,
    wanted: Option<&str>,
) -> Option<(&'a ide_cmake::CmakeBuild, &'a ide_cmake::CmakeTarget)> {
    let fallback = model.executables().next();
    match wanted {
        Some(name) => model.executables().find(|(_, target)| target.name == name).or(fallback),
        None => fallback,
    }
}

/// The CMake model of `root`, read from the File API reply of the last
/// configure and from `CMakePresets.json`.
pub fn cmake_model(root: &Path) -> Option<ide_cmake::CmakeModel> {
    let mut model = ide_cmake::load(root, None);
    // A project that was configured but has no File API reply (an older CMake
    // or a build directory made by hand) still has targets in its
    // CMakeLists.txt: list them so the window is useful before a reload.
    if model.builds.iter().all(|build| build.targets.is_empty()) {
        let declarations = cmakelists_targets(&root.join("CMakeLists.txt"));
        if !declarations.is_empty() {
            if model.builds.is_empty() {
                model.builds.push(ide_cmake::CmakeBuild {
                    dir: "build".to_owned(),
                    configured: false,
                    configuration_types: Vec::new(),
                    build_type: None,
                    generator: None,
                    source_dir: Some(root.to_string_lossy().replace('\\', "/")),
                    compile_commands: None,
                    profile: None,
                    targets: Vec::new(),
                });
            }
            let build = &mut model.builds[0];
            for (name, kind) in declarations {
                if build.targets.iter().any(|target| target.name == name) {
                    continue;
                }
                build.targets.push(ide_cmake::CmakeTarget {
                    name,
                    kind,
                    artifact: None,
                    sources: Vec::new(),
                    folder: None,
                    configuration: Some(build.configuration()),
                });
            }
        }
    }
    Some(model)
}

/// `add_executable` / `add_library` names from a CMakeLists.txt, in order.
fn cmakelists_targets(path: &Path) -> Vec<(String, ide_cmake::CmakeTargetKind)> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let (rest, kind) = if let Some(rest) = line.strip_prefix("add_executable(") {
            (rest, ide_cmake::CmakeTargetKind::Executable)
        } else if let Some(rest) = line.strip_prefix("add_library(") {
            (rest, ide_cmake::CmakeTargetKind::Unknown)
        } else {
            continue;
        };
        let name = rest.split(|c: char| c.is_whitespace() || c == ')').next().unwrap_or_default().trim();
        if name.is_empty() || name.starts_with('$') {
            continue;
        }
        let kind = if kind == ide_cmake::CmakeTargetKind::Unknown {
            let upper = rest.to_uppercase();
            if upper.contains("STATIC") {
                ide_cmake::CmakeTargetKind::StaticLibrary
            } else if upper.contains("SHARED") {
                ide_cmake::CmakeTargetKind::SharedLibrary
            } else if upper.contains("INTERFACE") {
                ide_cmake::CmakeTargetKind::InterfaceLibrary
            } else {
                ide_cmake::CmakeTargetKind::Unknown
            }
        } else {
            kind
        };
        out.push((name.to_owned(), kind));
    }
    out
}

/// Runs the ripgrep-backed project search and returns flat, UI-ready hits.
pub fn search_project(root: &Path, pattern: &str) -> Vec<SearchHit> {
    if pattern.trim().is_empty() {
        return Vec::new();
    }
    let query = SearchQuery {
        pattern: pattern.to_owned(),
        is_regex: false,
        case_sensitive: false,
        whole_word: false,
        include: Vec::new(),
        exclude: Vec::new(),
        scope: SearchScope::Project,
        max_matches: 500,
        context: ide_search::SearchContext::Any,
    };
    let collected: Mutex<Vec<SearchHit>> = Mutex::new(Vec::new());
    let on_file = |file: FileMatches| {
        let path = file.path.clone().into_path_buf();
        let mut out = collected.lock().unwrap_or_else(|p| p.into_inner());
        for line in file.matches {
            out.push(SearchHit {
                path: path.clone(),
                display: file.rel_path.clone(),
                line: line.line as usize + 1,
                text: line.text.trim().to_owned(),
            });
        }
    };
    let overlay = |_path: &Path| None;
    let cancel = CancellationToken::new();
    let _ = ide_search::search(root, &query, &overlay, &cancel, &on_file);
    collected.into_inner().unwrap_or_default()
}

impl State {
    /// Rebuilds the CMake tool window from the File API reply in the
    /// background; the panel shows its previous model until the new one lands.
    pub fn refresh_cmake(&self) {
        let state = *self;
        let root = self.root.get();
        self.status.set("Reloading CMake project…".to_owned());
        spawn(async move {
            let model = blocking(move || cmake_model(&root)).await.ok().flatten();
            let summary = model
                .as_ref()
                .map(|m| {
                    let targets: usize = m.builds.iter().map(|b| b.targets.len()).sum();
                    format!("CMake: {} build(s), {targets} target(s)", m.builds.len())
                })
                .unwrap_or_else(|| "CMake: no project".to_owned());
            state.cmake.set(model);
            state.status.set(summary);
        });
    }

    /// Reads git status and the current branch in the background.
    pub fn refresh_vcs(&self) {
        let state = *self;
        let root = self.root.get();
        spawn(async move {
            state.vcs_loading.set(true);
            let snapshot = blocking(move || VcsSnapshot::load(&root)).await.ok().unwrap_or(VcsSnapshot {
                rows: Vec::new(),
                branch: None,
                operation: None,
            });
            state.vcs_branch.set(snapshot.branch);
            state.vcs_loading.set(false);
            state.vcs.set(snapshot.rows);
        });
    }

    /// Finds in files with `ide-search` instead of the fallback line walk.
    pub fn search_backend(&self) {
        let state = *self;
        let root = self.root.get();
        let pattern = self.search_query.get();
        if pattern.trim().is_empty() {
            self.search_results.set(Vec::new());
            return;
        }
        self.searching.set(true);
        spawn(async move {
            let hits = blocking(move || search_project(&root, &pattern)).await.unwrap_or_default();
            state.search_results.set(hits);
            state.searching.set(false);
        });
    }
}

impl State {
    /// Builds the first executable target of the first build directory with
    /// CMake, then runs it — the title bar's green Run button.
    pub fn run_active_configuration(&self) {
        let state = *self;
        let root = self.root.get();
        let model = self.cmake.get();
        let Some(model) = model else {
            self.status.set("No CMake model — reload the CMake project".to_owned());
            return;
        };
        let Some((build, target)) = select_executable(&model, self.run_target.get().as_deref()) else {
            self.status.set("No executable target in the CMake model".to_owned());
            return;
        };
        let build_dir = root.join(&build.dir);
        let target_name = target.name.clone();
        let artifact = target.artifact.clone().map(PathBuf::from);
        let configuration = build.configuration();
        let display = format!("{target_name} ({configuration})");
        self.status.set(format!("Building {display}…"));
        crate::terminal::ensure_started(*self);
        crate::terminal::write_output(
            *self,
            &format!("\r\n$ cmake --build {} --target {target_name} -j\r\n", build_dir.display()),
        );
        self.show_bottom.set(true);
        self.bottom_tool.set(Some(crate::model::BottomTool::Terminal));
        spawn(async move {
            let result = blocking(move || {
                let build = Command::new("cmake")
                    .arg("--build")
                    .arg(&build_dir)
                    .arg("--target")
                    .arg(&target_name)
                    .arg("-j")
                    .current_dir(&root)
                    .output();
                let mut lines = Vec::new();
                match build {
                    Ok(output) => {
                        for line in String::from_utf8_lossy(&output.stdout).lines() {
                            lines.push(line.to_owned());
                        }
                        for line in String::from_utf8_lossy(&output.stderr).lines() {
                            lines.push(line.to_owned());
                        }
                        if !output.status.success() {
                            lines.push(format!("[build failed: exit {}]", output.status.code().unwrap_or(-1)));
                            return (lines, None);
                        }
                    }
                    Err(error) => {
                        lines.push(format!("error: {error}"));
                        return (lines, None);
                    }
                }
                let exe = artifact
                    .filter(|path| path.is_file())
                    .or_else(|| {
                        let candidate = build_dir.join(&target_name);
                        candidate.is_file().then_some(candidate)
                    })
                    .or_else(|| {
                        let candidate = build_dir.join(format!("{target_name}.exe"));
                        candidate.is_file().then_some(candidate)
                    });
                match exe {
                    Some(exe) => {
                        lines.push(format!("$ {}", exe.display()));
                        match Command::new(&exe).current_dir(&root).output() {
                            Ok(output) => {
                                for line in String::from_utf8_lossy(&output.stdout).lines() {
                                    lines.push(line.to_owned());
                                }
                                for line in String::from_utf8_lossy(&output.stderr).lines() {
                                    lines.push(line.to_owned());
                                }
                                lines.push(format!("[process exited {}]", output.status.code().unwrap_or(-1)));
                            }
                            Err(error) => lines.push(format!("run error: {error}")),
                        }
                        (lines, Some(exe))
                    }
                    None => {
                        lines.push("[the target produced no executable]".to_owned());
                        (lines, None)
                    }
                }
            })
            .await
            .ok();
            let (lines, exe) = result.unwrap_or_default();
            crate::terminal::write_output(state, &(lines.join("\r\n") + "\r\n"));
            state.status.set(match exe {
                Some(exe) => format!("Ran {}", exe.display()),
                None => format!("Build finished (no run)"),
            });
            state.refresh_cmake();
        });
    }

    /// Configures the first build directory with CMake and exports the compile
    /// database clangd reads.
    pub fn configure_cmake(&self) {
        let state = *self;
        let root = self.root.get();
        let build_dir = self
            .cmake
            .get()
            .and_then(|model| model.builds.first().map(|build| build.dir.clone()))
            .unwrap_or_else(|| "build".to_owned());
        let dir = root.join(&build_dir);
        self.status.set("Configuring CMake…".to_owned());
        crate::terminal::ensure_started(*self);
        crate::terminal::write_output(
            *self,
            &format!("\r\n$ cmake -S {} -B {} -DCMAKE_EXPORT_COMPILE_COMMANDS=ON\r\n", root.display(), dir.display()),
        );
        self.show_bottom.set(true);
        self.bottom_tool.set(Some(crate::model::BottomTool::Terminal));
        spawn(async move {
            let result = blocking(move || {
                let _ = ide_cmake::request(&dir);
                let output = Command::new("cmake")
                    .arg("-S")
                    .arg(&root)
                    .arg("-B")
                    .arg(&dir)
                    .arg("-DCMAKE_EXPORT_COMPILE_COMMANDS=ON")
                    .arg("-DCMAKE_BUILD_TYPE=Debug")
                    .current_dir(&root)
                    .output();
                let mut lines = Vec::new();
                match output {
                    Ok(output) => {
                        for line in String::from_utf8_lossy(&output.stdout).lines() {
                            lines.push(line.to_owned());
                        }
                        for line in String::from_utf8_lossy(&output.stderr).lines() {
                            lines.push(line.to_owned());
                        }
                        lines.push(format!("[configure exited {}]", output.status.code().unwrap_or(-1)));
                    }
                    Err(error) => lines.push(format!("error: {error}")),
                }
                lines
            })
            .await
            .unwrap_or_default();
            crate::terminal::write_output(state, &(result.join("\r\n") + "\r\n"));
            state.status.set("CMake configured".to_owned());
            state.refresh_cmake();
        });
    }
}
