//! CMake project model for the CMake tool window: build directories, the
//! File API codemodel (targets, sources, artifacts) and `CMakePresets.json`.
//!
//! CMake writes the codemodel only when the build directory holds a
//! `.cmake/api/v1/query/client-openclion/codemodel-v2` query at configure
//! time; [`request`] writes it before a configure run, [`load`] reads the
//! reply afterwards. Nothing here runs CMake: the run configurations of
//! `ide-run` own that.

use ide_core::settings::CMakeProfile;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Client name CMake records the reply under; also the query directory name.
pub const CLIENT: &str = "client-openclion";

/// The kind of a target, as far as the CMake tool window cares.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CmakeTargetKind {
    Executable,
    StaticLibrary,
    SharedLibrary,
    ModuleLibrary,
    ObjectLibrary,
    InterfaceLibrary,
    Utility,
    #[default]
    Unknown,
}

impl CmakeTargetKind {
    fn parse(kind: &str) -> Self {
        match kind {
            "EXECUTABLE" => Self::Executable,
            "STATIC_LIBRARY" => Self::StaticLibrary,
            "SHARED_LIBRARY" => Self::SharedLibrary,
            "MODULE_LIBRARY" => Self::ModuleLibrary,
            "OBJECT_LIBRARY" => Self::ObjectLibrary,
            "INTERFACE_LIBRARY" => Self::InterfaceLibrary,
            "UTILITY" | "CUSTOM" => Self::Utility,
            _ => Self::Unknown,
        }
    }

    /// Whether the target produces a file one can run and debug.
    pub fn is_executable(self) -> bool {
        self == Self::Executable
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CmakeTarget {
    pub name: String,
    pub kind: CmakeTargetKind,
    /// Absolute path of the linked artifact, when CMake reported one.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub artifact: Option<String>,
    /// Absolute source files of the target, sorted, deduplicated.
    pub sources: Vec<String>,
    /// The IDE folder CMake assigned (`set_target_properties(... FOLDER ...)`).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub folder: Option<String>,
    /// The codemodel configuration the target was read from (`Debug`, `Release`).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub configuration: Option<String>,
}

/// One configured (or configure-able) build directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CmakeBuild {
    /// Directory relative to the project root, with `/` separators (`build`, `out/Debug`).
    pub dir: String,
    /// Absolute source directory CMake recorded, when configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub source_dir: Option<String>,
    /// A `CMakeCache.txt` exists: configure already ran.
    pub configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub generator: Option<String>,
    /// `CMAKE_BUILD_TYPE` of a single-config generator, or the CMake profile's build type.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub build_type: Option<String>,
    /// `CMAKE_CONFIGURATION_TYPES` of a multi-config generator (empty for single-config).
    pub configuration_types: Vec<String>,
    /// The compile database clangd reads, when the configure exported one.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub compile_commands: Option<String>,
    /// Name of the CMake profile that owns the directory, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub profile: Option<String>,
    pub targets: Vec<CmakeTarget>,
}

impl CmakeBuild {
    /// The configuration to build / debug: the build type, else the first of the
    /// listed configuration types, else `Debug` (CMake's own fallback).
    pub fn configuration(&self) -> String {
        self.build_type
            .clone()
            .or_else(|| self.configuration_types.first().cloned())
            .unwrap_or_else(|| "Debug".to_owned())
    }
}

/// A `CMakePresets.json` entry the tool window lists next to the profiles.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CmakePreset {
    pub name: String,
    /// `configure` or `build`.
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub generator: Option<String>,
    /// Build directory the preset configures into, relative to the project root.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub binary_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub build_type: Option<String>,
    /// Build presets: the configure preset they build.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub configure_preset: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CmakeModel {
    pub source_dir: String,
    /// A `CMakeLists.txt` sits in the project root.
    pub has_project: bool,
    /// A `CMakePresets.json` sits in the project root.
    pub has_presets: bool,
    pub presets: Vec<CmakePreset>,
    pub builds: Vec<CmakeBuild>,
}

impl CmakeModel {
    /// Every executable target with the build directory that produces it.
    pub fn executables(&self) -> impl Iterator<Item = (&CmakeBuild, &CmakeTarget)> {
        self.builds.iter().flat_map(|b| b.targets.iter().filter(|t| t.kind.is_executable()).map(move |t| (b, t)))
    }
}

/// Build directories of the project: the selected `CMakePresets.json` presets,
/// else the settings profiles, else every directory holding a `CMakeCache.txt`,
/// else `<root>/build`.
fn build_dirs(
    root: &Path,
    profiles: Option<&[CMakeProfile]>,
    preset_names: Option<&[String]>,
) -> Vec<(String, Option<String>)> {
    if let Some(names) = preset_names {
        let available = presets(root);
        return names
            .iter()
            .map(|name| {
                let preset = available.iter().find(|p| p.kind == "configure" && &p.name == name);
                match preset {
                    Some(preset) => (preset_build_dir(root, preset), Some(name.clone())),
                    None => (format!("cmake-build-{name}"), Some(name.clone())),
                }
            })
            .collect();
    }
    if let Some(profiles) = profiles {
        return profiles.iter().map(|p| (profile_dir(p), (!p.name.is_empty()).then(|| p.name.clone()))).collect();
    }
    let mut found: Vec<PathBuf> = fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            (path.is_dir() && path.join("CMakeCache.txt").is_file()).then_some(path)
        })
        .collect();
    found.sort();
    if found.is_empty() {
        return vec![("build".to_owned(), None)];
    }
    found
        .into_iter()
        .map(|p| {
            let dir = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            (dir, None)
        })
        .collect()
}

/// The build directory of a CMake profile; empty `build_dir` means
/// `cmake-build-<lowercase build type>`, like CLion's default.
pub fn profile_dir(profile: &CMakeProfile) -> String {
    if profile.build_dir.trim().is_empty() {
        format!("cmake-build-{}", profile.build_type.to_lowercase())
    } else {
        profile.build_dir.clone()
    }
}

/// The model of the project at `root`, reading what the last configure wrote.
pub fn load(root: &Path, profiles: Option<&[CMakeProfile]>) -> CmakeModel {
    load_with(root, profiles, None)
}

/// The model with the project's CMake selection: presets win over profiles.
pub fn load_with(root: &Path, profiles: Option<&[CMakeProfile]>, preset_names: Option<&[String]>) -> CmakeModel {
    let has_project = root.join("CMakeLists.txt").is_file();
    let mut builds = Vec::new();
    if has_project {
        for (dir, profile) in build_dirs(root, profiles, preset_names) {
            builds.push(load_build(root, &dir, profile));
        }
    }
    let presets = presets(root);
    CmakeModel {
        source_dir: root.to_string_lossy().replace('\\', "/"),
        has_project,
        has_presets: root.join("CMakePresets.json").is_file(),
        presets,
        builds,
    }
}

fn load_build(root: &Path, dir: &str, profile: Option<String>) -> CmakeBuild {
    let build = root.join(dir);
    let cache = fs::read_to_string(build.join("CMakeCache.txt")).ok();
    let configured = cache.is_some();
    let cache_value = |key: &str| {
        cache.as_deref().and_then(|c| {
            let prefix = format!("{key}:");
            c.lines()
                .find_map(|l| l.strip_prefix(&prefix))
                .and_then(|rest| rest.split_once('='))
                .map(|(_, v)| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        })
    };
    let compile_commands = build.join("compile_commands.json");
    CmakeBuild {
        dir: dir.to_owned(),
        source_dir: cache_value("CMAKE_HOME_DIRECTORY").or_else(|| Some(abs(root))),
        configured,
        generator: cache_value("CMAKE_GENERATOR"),
        build_type: cache_value("CMAKE_BUILD_TYPE"),
        configuration_types: cache_value("CMAKE_CONFIGURATION_TYPES")
            .map(|v| v.split(';').map(str::to_owned).collect())
            .unwrap_or_default(),
        compile_commands: compile_commands.is_file().then(|| abs(&compile_commands)),
        profile,
        targets: targets(&build),
    }
}

fn abs(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn read_json(path: &Path) -> Option<serde_json::Value> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

/// Resolves a reply reference: the File API only names files next to the index.
fn reply_file(reply: &Path, name: &str) -> Option<serde_json::Value> {
    (Path::new(name).components().count() == 1).then(|| read_json(&reply.join(name))).flatten()
}

/// Current model of `build`, live updates included: only the newest index counts.
pub fn index_path(build: &Path) -> Option<PathBuf> {
    let reply = build.join(".cmake/api/v1/reply");
    fs::read_dir(reply).ok()?.flatten().map(|e| e.path()).filter(|p| is_index(p)).max()
}

fn is_index(path: &Path) -> bool {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    name.starts_with("index-") && name.ends_with(".json")
}

/// Targets of the newest codemodel in `build`, empty when CMake never ran.
pub fn targets(build: &Path) -> Vec<CmakeTarget> {
    targets_in(build, None, None)
}

/// Targets of the newest codemodel of `build`, restricted to `configuration`
/// when that is a multi-config generator, and only when the codemodel describes
/// `source_root` (a reply lingers after the project moves).
pub fn targets_for(build: &Path, source_root: &Path, configuration: Option<&str>) -> Vec<CmakeTarget> {
    let same = source_root.canonicalize().ok();
    targets_in(build, same.as_deref(), configuration)
}

fn targets_in(build: &Path, source_root: Option<&Path>, configuration: Option<&str>) -> Vec<CmakeTarget> {
    let Some((reply, model)) = codemodel(build) else {
        return Vec::new();
    };
    if let Some(expected) = source_root {
        let recorded = model["paths"]["source"].as_str().map(PathBuf::from);
        if recorded.as_deref().and_then(|p| p.canonicalize().ok()).as_deref() != Some(expected) {
            return Vec::new();
        }
    }
    let mut targets = Vec::new();
    for entry in model["configurations"].as_array().into_iter().flatten() {
        let name = entry["name"].as_str().unwrap_or_default();
        if let Some(wanted) = configuration
            && !name.is_empty()
            && name != wanted
        {
            continue;
        }
        for target in entry["targets"].as_array().into_iter().flatten() {
            let Some(json) = target["jsonFile"].as_str().and_then(|n| reply_file(&reply, n)) else {
                continue;
            };
            if let Some(target) = parse_target(&json, (!name.is_empty()).then_some(name)) {
                targets.push(target);
            }
        }
    }
    targets.sort_by(|a, b| a.name.cmp(&b.name));
    targets.dedup_by(|a, b| a.name == b.name);
    targets
}

/// The source directory the newest codemodel records, exactly as CMake wrote
/// it (not canonicalized: `/tmp` vs `/private/tmp` is what a debugger source
/// map has to translate).
pub fn codemodel_source(build: &Path) -> Option<String> {
    codemodel(build).and_then(|(_, model)| model["paths"]["source"].as_str().map(str::to_owned))
}

/// The codemodel of the newest index in `build`, with the reply directory.
fn codemodel(build: &Path) -> Option<(PathBuf, serde_json::Value)> {
    let reply = build.join(".cmake/api/v1/reply");
    let index = read_json(&index_path(build)?)?;
    let reference = index["objects"].as_array().and_then(|objects| {
        // Only codemodel v2 carries targets, artifacts and sources.
        objects.iter().find(|o| o["kind"] == "codemodel" && o["version"]["major"] == 2)
    })?;
    let model = reference["jsonFile"].as_str().and_then(|n| reply_file(&reply, n))?;
    Some((reply, model))
}

fn parse_target(json: &serde_json::Value, configuration: Option<&str>) -> Option<CmakeTarget> {
    let name = json["name"].as_str()?.to_owned();
    let kind = CmakeTargetKind::parse(json["type"].as_str().unwrap_or_default());
    let source_root = json["paths"]["source"].as_str().unwrap_or_default();
    let build_root = json["paths"]["build"].as_str().unwrap_or_default();
    let mut sources: Vec<String> = json["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s["path"].as_str())
        .map(|p| abs(&Path::new(source_root).join(p)))
        .collect();
    sources.sort();
    sources.dedup();
    let artifact = json["artifacts"].as_array().and_then(|a| a.first()).and_then(|a| a["path"].as_str());
    let folder = match &json["folder"] {
        serde_json::Value::Object(o) => o.get("name").and_then(|n| n.as_str()).map(str::to_owned),
        serde_json::Value::String(s) => Some(s.clone()),
        _ => None,
    }
    .filter(|f| !f.is_empty());
    Some(CmakeTarget {
        name,
        kind,
        artifact: artifact.map(|p| abs(&Path::new(build_root).join(p))),
        sources,
        folder,
        configuration: configuration.map(str::to_owned),
    })
}

/// `CMakePresets.json` entries: configure presets, then build presets that name one.
pub fn presets(root: &Path) -> Vec<CmakePreset> {
    let Some(json) = read_json(&root.join("CMakePresets.json")) else {
        return Vec::new();
    };
    let source = abs(root);
    let expand = |value: &str, preset: &str, build_type: &str| {
        value
            .replace("${sourceDir}", &source)
            .replace("${fileDir}", &source)
            .replace("${presetName}", preset)
            .replace("${buildType}", build_type)
    };
    let mut presets = Vec::new();
    for (kind, key) in [("configure", "configurePresets"), ("build", "buildPresets")] {
        for preset in json[key].as_array().into_iter().flatten() {
            let Some(name) = preset["name"].as_str() else { continue };
            if preset["hidden"].as_bool().unwrap_or(false) || preset["hidden"].is_string() {
                continue;
            }
            // `inherits` chains are how real projects share a base preset.
            let resolved = resolve_inherited(preset, &json, key, 0);
            let build_type = resolved["cacheVariables"]["CMAKE_BUILD_TYPE"].as_str().unwrap_or_default();
            let binary_dir = resolved["binaryDir"].as_str().map(|d| expand(d, name, build_type));
            let generator = resolved["generator"].as_str().map(|g| expand(g, name, build_type));
            presets.push(CmakePreset {
                name: name.to_owned(),
                kind: kind.to_owned(),
                generator,
                binary_dir: binary_dir.map(|d| d.replace('\\', "/")),
                build_type: (!build_type.is_empty()).then(|| build_type.to_owned()),
                configure_preset: resolved["configurePreset"].as_str().map(str::to_owned),
            });
        }
    }
    presets
}

/// A preset with the fields of its `inherits` bases applied underneath it.
fn resolve_inherited(
    preset: &serde_json::Value,
    json: &serde_json::Value,
    key: &str,
    depth: usize,
) -> serde_json::Value {
    let mut merged = serde_json::Map::new();
    if depth < 10 {
        let bases: Vec<&str> = match &preset["inherits"] {
            serde_json::Value::String(s) => vec![s.as_str()],
            serde_json::Value::Array(a) => a.iter().filter_map(|v| v.as_str()).collect(),
            _ => Vec::new(),
        };
        for base in bases {
            let Some(parent) = json[key].as_array().and_then(|a| a.iter().find(|p| p["name"] == base)) else {
                continue;
            };
            let parent = resolve_inherited(parent, json, key, depth + 1);
            merge_preset(&mut merged, &parent);
        }
    }
    // The `name` and the kind-specific link always come from the preset itself.
    merge_preset(&mut merged, preset);
    serde_json::Value::Object(merged)
}

fn merge_preset(into: &mut serde_json::Map<String, serde_json::Value>, from: &serde_json::Value) {
    for (key, value) in from.as_object().into_iter().flatten() {
        if key == "inherits" || key == "hidden" || key == "name" {
            continue;
        }
        if key == "cacheVariables" {
            let target = into.entry(key).or_insert_with(|| serde_json::json!({}));
            for (k, v) in value.as_object().into_iter().flatten() {
                target[k] = v.clone();
            }
        } else {
            into.insert(key.clone(), value.clone());
        }
    }
}

/// The configure preset named `name`.
pub fn configure_preset(root: &Path, name: &str) -> Option<CmakePreset> {
    presets(root).into_iter().find(|p| p.kind == "configure" && p.name == name)
}

/// The first build preset that builds the configure preset `name`.
pub fn build_preset_for(root: &Path, name: &str) -> Option<CmakePreset> {
    presets(root).into_iter().find(|p| p.kind == "build" && p.configure_preset.as_deref() == Some(name))
}

/// Absolute build directory of a configure preset; `cmake-build-<name>` when the
/// preset names none (CMake's own default would be `build`, but the IDE keeps
/// the source tree clean like the CMake profiles do).
pub fn preset_build_dir(root: &Path, preset: &CmakePreset) -> String {
    let raw = preset.binary_dir.clone().unwrap_or_else(|| format!("cmake-build-{}", preset.name));
    let path = Path::new(&raw);
    if path.is_absolute() { path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/") } else { raw }
}

/// Where CMake should drop the codemodel reply for `build`.
pub fn query_dir(build: &Path) -> PathBuf {
    build.join(".cmake/api/v1/query").join(CLIENT)
}

/// Asks CMake for the codemodel on the next configure of `build` (idempotent).
pub fn request(build: &Path) -> io::Result<()> {
    let query = query_dir(build);
    fs::create_dir_all(&query)?;
    let file = query.join("codemodel-v2");
    if !file.exists() {
        fs::write(file, "")?;
    }
    Ok(())
}

/// The compile database clangd should read: the first configured build directory
/// that exported one, in profile order.
pub fn compile_commands(root: &Path, profiles: Option<&[CMakeProfile]>) -> Option<PathBuf> {
    for (dir, _) in build_dirs(root, profiles, None) {
        let file = root.join(dir).join("compile_commands.json");
        if file.is_file() {
            return Some(file);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn write(path: &Path, value: impl AsRef<[u8]>) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, value).unwrap();
    }

    fn fixture(root: &Path, dir: &str) {
        write(&root.join("CMakeLists.txt"), "project(app CXX)");
        let reply = root.join(dir).join(".cmake/api/v1/reply");
        write(
            &reply.join("index-2026-01-01T00-00-00-0000.json"),
            json!({"objects":[{"kind":"codemodel","version":{"major":2,"minor":6},"jsonFile":"codemodel-v2-app.json"}]})
                .to_string(),
        );
        write(
            &reply.join("codemodel-v2-app.json"),
            json!({"kind":"codemodel","paths":{"source":"","build":""},
                "configurations":[{"name":"Debug","targets":[
                    {"jsonFile":"target-app.json"},{"jsonFile":"target-lib.json"}]}]})
            .to_string(),
        );
        write(
            &reply.join("target-app.json"),
            json!({"name":"app","type":"EXECUTABLE","paths":{"source":root.to_string_lossy(),"build":root.join(dir).to_string_lossy()},
                "sources":[{"path":"src/main.cpp"},{"path":"src/greeter.cpp"}],"artifacts":[{"path":"app"}],"folder":{"name":"Apps"}})
            .to_string(),
        );
        write(
            &reply.join("target-lib.json"),
            json!({"name":"greeter","type":"STATIC_LIBRARY","paths":{"source":root.to_string_lossy(),"build":root.join(dir).to_string_lossy()},
                "sources":[{"path":"src/greeter.cpp"}],"artifacts":[{"path":"libgreeter.a"}]})
            .to_string(),
        );
        write(
            &root.join(dir).join("CMakeCache.txt"),
            "CMAKE_GENERATOR:INTERNAL=Ninja\nCMAKE_BUILD_TYPE:STRING=Debug\nCMAKE_HOME_DIRECTORY:INTERNAL=/src\n",
        );
        write(&root.join(dir).join("compile_commands.json"), "[]");
    }

    #[test]
    fn reads_targets_artifacts_and_build_settings() {
        let tmp = tempfile::tempdir().unwrap();
        fixture(tmp.path(), "cmake-build-debug");
        let model = load(tmp.path(), None);
        assert!(model.has_project);
        assert_eq!(model.builds.len(), 1);
        let build = &model.builds[0];
        assert_eq!(build.dir, "cmake-build-debug");
        assert!(build.configured);
        assert_eq!(build.generator.as_deref(), Some("Ninja"));
        assert_eq!(build.build_type.as_deref(), Some("Debug"));
        assert_eq!(build.configuration(), "Debug");
        assert!(build.compile_commands.is_some());
        assert_eq!(build.targets.len(), 2);
        let app = build.targets.iter().find(|t| t.name == "app").unwrap();
        assert_eq!(app.kind, CmakeTargetKind::Executable);
        assert!(app.artifact.as_deref().unwrap().ends_with("cmake-build-debug/app"));
        assert_eq!(app.sources.len(), 2);
        assert_eq!(app.folder.as_deref(), Some("Apps"));
        assert_eq!(model.executables().count(), 1);
    }

    #[test]
    fn profiles_pick_build_directories_even_before_configure() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("CMakeLists.txt"), "project(app CXX)");
        let profiles = [
            CMakeProfile { name: "Debug".into(), ..Default::default() },
            CMakeProfile {
                name: "Bench".into(),
                build_type: "Release".into(),
                build_dir: "out/bench".into(),
                ..Default::default()
            },
        ];
        let model = load(tmp.path(), Some(&profiles));
        assert_eq!(model.builds.len(), 2);
        assert_eq!(model.builds[0].dir, "cmake-build-debug");
        assert_eq!(model.builds[0].profile.as_deref(), Some("Debug"));
        assert_eq!(model.builds[1].dir, "out/bench");
        assert!(!model.builds[1].configured);
    }

    #[test]
    fn presets_expand_source_dir_macros() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("CMakeLists.txt"), "project(app CXX)");
        write(
            &tmp.path().join("CMakePresets.json"),
            json!({"version":6,
                "configurePresets":[
                    {"name":"debug","generator":"Ninja","binaryDir":"${sourceDir}/build/debug",
                     "cacheVariables":{"CMAKE_BUILD_TYPE":"Debug"}},
                    {"name":"base","hidden":true}],
                "buildPresets":[{"name":"debug-build","configurePreset":"debug"}]})
            .to_string(),
        );
        let model = load(tmp.path(), None);
        assert!(model.has_presets);
        assert_eq!(model.presets.len(), 2);
        let configure = model.presets.iter().find(|p| p.kind == "configure").unwrap();
        assert_eq!(configure.name, "debug");
        assert_eq!(configure.build_type.as_deref(), Some("Debug"));
        assert!(configure.binary_dir.as_deref().unwrap().ends_with("/build/debug"));
        assert_eq!(model.presets.iter().filter(|p| p.kind == "build").count(), 1);
    }

    #[test]
    fn presets_inherit_bases_and_link_build_presets() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("CMakeLists.txt"), "project(app CXX)");
        write(
            &tmp.path().join("CMakePresets.json"),
            json!({"version":6,
                "configurePresets":[
                    {"name":"base","hidden":true,"generator":"Ninja",
                     "cacheVariables":{"CMAKE_BUILD_TYPE":"Debug","EXTRA":"1"}},
                    {"name":"debug","inherits":"base","binaryDir":"${sourceDir}/out/debug"},
                    {"name":"release","inherits":["base"],
                     "cacheVariables":{"CMAKE_BUILD_TYPE":"Release"}}],
                "buildPresets":[
                    {"name":"debug-build","configurePreset":"debug"},
                    {"name":"release-build","configurePreset":"release"}]})
            .to_string(),
        );
        let presets = presets(tmp.path());
        let debug = presets.iter().find(|p| p.kind == "configure" && p.name == "debug").unwrap();
        // Inherited generator, own binaryDir, inherited build type.
        assert_eq!(debug.generator.as_deref(), Some("Ninja"));
        assert_eq!(debug.build_type.as_deref(), Some("Debug"));
        assert!(debug.binary_dir.as_deref().unwrap().ends_with("/out/debug"));
        let release = presets.iter().find(|p| p.name == "release").unwrap();
        assert_eq!(release.build_type.as_deref(), Some("Release"));
        // The base is hidden: it is available for inheritance, not as a profile.
        assert!(!presets.iter().any(|p| p.name == "base"));
        let build = presets.iter().find(|p| p.name == "debug-build").unwrap();
        assert_eq!(build.configure_preset.as_deref(), Some("debug"));
        assert_eq!(build_preset_for(tmp.path(), "debug").unwrap().name, "debug-build");
        assert_eq!(configure_preset(tmp.path(), "release").unwrap().name, "release");
        assert!(preset_build_dir(tmp.path(), debug).ends_with("out/debug"));
        // A preset without binaryDir gets a build directory of its own.
        let plain = CmakePreset { name: "plain".into(), kind: "configure".into(), ..Default::default() };
        assert_eq!(preset_build_dir(tmp.path(), &plain), "cmake-build-plain");
    }

    #[test]
    fn selected_presets_are_the_build_directories() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("CMakeLists.txt"), "project(app CXX)");
        write(
            &tmp.path().join("CMakePresets.json"),
            json!({"version":6,"configurePresets":[
                {"name":"debug","binaryDir":"${sourceDir}/out/debug"},
                {"name":"release","binaryDir":"${sourceDir}/out/release"}]})
            .to_string(),
        );
        let names = ["release".to_string()];
        let model = load_with(tmp.path(), None, Some(&names));
        assert_eq!(model.builds.len(), 1);
        assert_eq!(model.builds[0].dir, "out/release");
        // The label is the preset's name, so the tree reads like CLion's.
        assert_eq!(model.builds[0].profile.as_deref(), Some("release"));
        // A selected preset the file no longer declares still gets a directory.
        let gone = ["ghost".to_string()];
        assert_eq!(load_with(tmp.path(), None, Some(&gone)).builds[0].dir, "cmake-build-ghost");
    }

    #[test]
    fn request_writes_the_query_once() {
        let tmp = tempfile::tempdir().unwrap();
        let build = tmp.path().join("build");
        request(&build).unwrap();
        request(&build).unwrap();
        let query = query_dir(&build);
        assert_eq!(fs::read_dir(&query).unwrap().count(), 1);
        assert!(query.join("codemodel-v2").exists());
    }

    #[test]
    fn a_project_without_cmake_has_no_builds() {
        let tmp = tempfile::tempdir().unwrap();
        let model = load(tmp.path(), None);
        assert!(!model.has_project);
        assert!(model.builds.is_empty());
    }
}
