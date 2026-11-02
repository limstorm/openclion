//! CMake targets come from the shared CMake project model (`ide-cmake`), which
//! reads the File API codemodel including evaluated subdirectories.

use super::{ConfigExt, config};
use crate::{BeforeLaunchTask, RunConfig};
use ide_cmake::CmakePreset;
use ide_core::settings::CMakeProfile;
use std::fs;
use std::path::Path;

fn before(config: &mut RunConfig, id: &str) {
    config.extras.before_launch.push(BeforeLaunchTask { kind: "run".into(), target: id.into() });
}

pub(super) fn detect(
    root: &Path,
    profiles: Option<&[CMakeProfile]>,
    presets: Option<&[String]>,
    out: &mut Vec<RunConfig>,
) {
    if !root.join("CMakeLists.txt").is_file() {
        return;
    }
    // Selected `CMakePresets.json` presets replace both profiles and discovery.
    if let Some(presets) = presets {
        let available = ide_cmake::presets(root);
        for (index, name) in presets.iter().enumerate() {
            detect_preset(root, name, index, presets.len() > 1, &available, out);
        }
        return;
    }
    if let Some(profiles) = profiles {
        for (index, profile) in profiles.iter().enumerate() {
            detect_profile(root, profile, index, profiles.len() > 1, out);
        }
        return;
    }
    let mut builds = Vec::new();
    if let Ok(entries) = fs::read_dir(root) {
        builds.extend(entries.flatten().filter_map(|e| {
            let p = e.path();
            (p.is_dir() && p.join("CMakeCache.txt").is_file()).then_some(p)
        }));
    }
    if builds.is_empty() {
        builds.push(root.join("build"));
    }
    builds.sort();
    builds.dedup();
    for build in builds {
        let dir = build.strip_prefix(root).unwrap_or(&build).to_string_lossy().replace('\\', "/");
        let configuration = fs::read_to_string(build.join("CMakeCache.txt"))
            .ok()
            .and_then(|cache| {
                let values = cache.lines().find_map(|line| line.strip_prefix("CMAKE_CONFIGURATION_TYPES:STRING="))?;
                Some(if values.split(';').any(|c| c == "Debug") {
                    "Debug".to_owned()
                } else {
                    values.split(';').find(|s| !s.is_empty())?.to_owned()
                })
            })
            .unwrap_or_else(|| "Debug".into());
        let prefix = format!("cmake:{dir}");
        let configure_id = format!("{prefix}:configure");
        let build_id = format!("{prefix}:build");
        let mut args = vec!["-S", ".", "-B", &dir, "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON"];
        if !build.join("CMakeCache.txt").is_file() {
            args.push("-DCMAKE_BUILD_TYPE=Debug");
        }
        let mut configure = config(&configure_id, format!("CMake Configure ({dir})"), "cmake", "cmake", &args);
        configure.extras.params.insert("buildDir".into(), dir.clone());
        out.push(configure);
        let mut all = config(
            &build_id,
            format!("CMake Build ({dir})"),
            "cmake",
            "cmake",
            &["--build", &dir, "--config", &configuration],
        );
        // `cmake --build` regenerates by itself once a build directory is configured.
        if !build.join("CMakeCache.txt").is_file() {
            before(&mut all, &configure_id);
        }
        out.push(all);
        let report = format!("{dir}/ctest-junit.xml");
        let mut test = config(
            format!("{prefix}:test"),
            format!("CTest ({dir})"),
            "cmake",
            "ctest",
            &["--test-dir", &dir, "-C", &configuration, "--output-junit", &report, "--output-on-failure"],
        )
        .test();
        test.extras.params.insert("ctestReport".into(), report.clone());
        before(&mut test, &build_id);
        out.push(test);
        targets(root, &build, &dir, &prefix, Some(&configuration), &[], out);
    }
}

fn detect_profile(root: &Path, profile: &CMakeProfile, index: usize, multiple: bool, out: &mut Vec<RunConfig>) {
    let dir = if profile.build_dir.trim().is_empty() {
        format!("cmake-build-{}", profile.build_type.to_lowercase())
    } else {
        profile.build_dir.clone()
    };
    let build = root.join(&dir);
    let prefix = format!("cmake:profile:{index}");
    let configure_id = format!("{prefix}:configure");
    let build_id = format!("{prefix}:build");
    let start = out.len();
    let mut configure =
        config(&configure_id, format!("CMake Configure ({dir})"), "cmake", "cmake", &["-S", ".", "-B", &dir]);
    if !profile.generator.trim().is_empty() {
        configure.args.extend(["-G".into(), profile.generator.clone()]);
    }
    configure.args.push(format!("-DCMAKE_BUILD_TYPE={}", profile.build_type));
    configure.args.push("-DCMAKE_EXPORT_COMPILE_COMMANDS=ON".into());
    configure.args.extend_from_slice(&profile.cmake_args);
    configure.args.extend(profile.toolchain_definitions());
    configure.extras.params.insert("buildDir".into(), dir.clone());
    out.push(configure);
    let mut all = config(
        &build_id,
        format!("CMake Build ({dir})"),
        "cmake",
        "cmake",
        &["--build", &dir, "--config", &profile.build_type],
    );
    all.args.extend_from_slice(&profile.build_args);
    if !build.join("CMakeCache.txt").is_file() {
        before(&mut all, &configure_id);
    }
    out.push(all);
    let report = format!("{dir}/ctest-junit.xml");
    let mut test = config(
        format!("{prefix}:test"),
        format!("CTest ({dir})"),
        "cmake",
        "ctest",
        &["--test-dir", &dir, "-C", &profile.build_type, "--output-junit", &report, "--output-on-failure"],
    )
    .test();
    test.extras.params.insert("ctestReport".into(), report.clone());
    before(&mut test, &build_id);
    out.push(test);
    targets(root, &build, &dir, &prefix, Some(&profile.build_type), &profile.build_args, out);
    for config in &mut out[start..] {
        config.env = profile.env.clone();
        if multiple {
            config.name = format!("{} [{}]", config.name, profile.name);
        }
    }
}

/// A selected `CMakePresets.json` preset: `cmake --preset <name>`, the build
/// preset that goes with it (else `cmake --build <dir>`), CTest and the targets
/// of the preset's build directory.
fn detect_preset(
    root: &Path,
    name: &str,
    index: usize,
    multiple: bool,
    presets: &[CmakePreset],
    out: &mut Vec<RunConfig>,
) {
    let Some(preset) = presets.iter().find(|p| p.kind == "configure" && p.name == name) else {
        return;
    };
    let dir = ide_cmake::preset_build_dir(root, preset);
    let build = root.join(&dir);
    let prefix = format!("cmake:preset:{index}");
    let configure_id = format!("{prefix}:configure");
    let build_id = format!("{prefix}:build");
    let configuration = preset.build_type.clone().unwrap_or_else(|| "Debug".into());
    let start = out.len();

    let mut configure =
        config(&configure_id, format!("CMake Configure ({name})"), "cmake", "cmake", &["--preset", name]);
    configure.extras.params.insert("buildDir".into(), dir.clone());
    configure.extras.params.insert("preset".into(), name.to_owned());
    out.push(configure);

    let build_preset = presets.iter().find(|p| p.kind == "build" && p.configure_preset.as_deref() == Some(name));
    let mut all = match &build_preset {
        Some(p) => {
            config(&build_id, format!("CMake Build ({})", p.name), "cmake", "cmake", &["--build", "--preset", &p.name])
        }
        None => config(
            &build_id,
            format!("CMake Build ({dir} {configuration})"),
            "cmake",
            "cmake",
            &["--build", &dir, "--config", &configuration],
        ),
    };
    // `cmake --preset` regenerates by itself once the build directory is configured.
    if !build.join("CMakeCache.txt").is_file() {
        before(&mut all, &configure_id);
    }
    out.push(all);

    let report = format!("{dir}/ctest-junit.xml");
    let mut test = config(
        format!("{prefix}:test"),
        format!("CTest ({dir})"),
        "cmake",
        "ctest",
        &["--test-dir", &dir, "-C", &configuration, "--output-junit", &report, "--output-on-failure"],
    )
    .test();
    test.extras.params.insert("ctestReport".into(), report.clone());
    before(&mut test, &build_id);
    out.push(test);

    targets(root, &build, &dir, &prefix, Some(&configuration), &[], out);
    if multiple {
        for config in &mut out[start..] {
            config.name = format!("{} [{}]", config.name, name);
        }
    }
}

fn targets(
    root: &Path,
    build: &Path,
    dir: &str,
    prefix: &str,
    configuration: Option<&str>,
    build_args: &[String],
    out: &mut Vec<RunConfig>,
) {
    // A profile or preset selects the codemodel configuration; discovery takes the first.
    let configuration = configuration.filter(|c| !c.is_empty());
    for target in ide_cmake::targets_for(build, root, configuration) {
        if !target.kind.is_executable() {
            continue;
        }
        let Some(artifact) = target.artifact else { continue };
        let target_name = target.name.as_str();
        let name = target.configuration.unwrap_or_default();
        let id = format!("{prefix}:{name}:{target_name}");
        let target_build_id = format!("{id}:build");
        let mut args = vec!["--build", dir, "--target", target_name];
        if !name.is_empty() {
            args.extend(["--config", name.as_str()]);
        }
        let mut build_config =
            config(&target_build_id, format!("Build {target_name} ({dir} {name})"), "cmake", "cmake", &args);
        if let Some(chosen) = configuration {
            if name.is_empty() {
                build_config.args.extend(["--config".into(), chosen.to_owned()]);
            }
            build_config.args.extend_from_slice(build_args);
            if !build.join("CMakeCache.txt").is_file() {
                before(&mut build_config, &format!("{prefix}:configure"));
            }
        }
        out.push(build_config);
        let mut run = config(&id, format!("{target_name} ({dir} {name})"), "native", &artifact, &[]).debug("lldb-dap");
        before(&mut run, &target_build_id);
        run.extras.params.insert("cmakeSources".into(), target.sources.join("\n"));
        // lldb-dap reports paths as CMake configured them; translate to the real tree.
        let recorded = ide_cmake::codemodel_source(build).unwrap_or_else(|| root.to_string_lossy().into_owned());
        let canonical = dunce::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        run.extras.params.insert("cmakeSourceRoot".into(), recorded.replace('\\', "/"));
        run.extras.params.insert("cmakeCanonicalSourceRoot".into(), canonical.to_string_lossy().replace('\\', "/"));
        out.push(run);
    }
}

/// Request the codemodel only when the user runs a configure configuration.
pub(crate) fn prepare(config: &RunConfig, cwd: &Path) -> crate::IdeResult<()> {
    if config.kind == "cmake"
        && config.args.first().is_some_and(|a| a == "-S")
        && let Some(dir) = config.extras.params.get("buildDir")
    {
        let build = cwd.join(dir);
        let generator = config.args.windows(2).find(|a| a[0] == "-G").map(|a| a[1].as_str());
        if let Some(generator) = generator
            && let Ok(cache) = fs::read_to_string(build.join("CMakeCache.txt"))
            && let Some(cached) = cache.lines().find_map(|l| l.strip_prefix("CMAKE_GENERATOR:INTERNAL="))
            && cached != generator
        {
            fs::remove_file(build.join("CMakeCache.txt"))?;
            let files = build.join("CMakeFiles");
            if files.is_dir() {
                fs::remove_dir_all(files)?;
            }
        }
        ide_cmake::request(&build)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn profiles_select_file_api_configuration_and_apply_target_options() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("CMakeLists.txt"), "project(app)").unwrap();
        let reply = root.path().join("output/.cmake/api/v1/reply");
        fs::create_dir_all(&reply).unwrap();
        let write = |name: &str, value: Value| fs::write(reply.join(name), value.to_string()).unwrap();
        write(
            "index-1.json",
            serde_json::json!({"objects":[{"kind":"codemodel","version":{"major":2},"jsonFile":"model.json"}]}),
        );
        write(
            "model.json",
            serde_json::json!({"paths":{"source":root.path()},"configurations":[
                {"name":"Debug","targets":[{"jsonFile":"debug.json"}]},
                {"name":"Release","targets":[{"jsonFile":"release.json"}]}
            ]}),
        );
        for name in ["debug", "release"] {
            write(
                &format!("{name}.json"),
                serde_json::json!({"name":"app","type":"EXECUTABLE","artifacts":[{"path":format!("{name}/app")}]}),
            );
        }
        let profile = CMakeProfile {
            name: "Production".into(),
            build_type: "Release".into(),
            build_dir: "output".into(),
            build_args: ["-j", "8", "--", "verbose=1"].map(String::from).into(),
            env: [("FOO".into(), "bar".into())].into(),
            ..Default::default()
        };
        let mut configs = Vec::new();
        detect(root.path(), Some(std::slice::from_ref(&profile)), None, &mut configs);
        let run = configs.iter().find(|c| c.kind == "native").unwrap();
        assert!(run.command.ends_with("release/app"));
        assert_eq!(run.env, profile.env);
        assert_eq!(configs.iter().filter(|c| c.kind == "native").count(), 1);
        let build = configs.iter().find(|c| c.id == run.extras.before_launch[0].target).unwrap();
        assert_eq!(
            build.args,
            ["--build", "output", "--target", "app", "--config", "Release", "-j", "8", "--", "verbose=1"]
        );
        assert_eq!(build.env, profile.env);
        assert_eq!(build.extras.before_launch[0].target, "cmake:profile:0:configure");
        fs::write(root.path().join("output/CMakeCache.txt"), "CMAKE_GENERATOR:INTERNAL=Ninja\n").unwrap();
        let mut configured = Vec::new();
        detect(root.path(), Some(std::slice::from_ref(&profile)), None, &mut configured);
        for id in ["cmake:profile:0:build", build.id.as_str()] {
            let build = configured.iter().find(|c| c.id == id).unwrap();
            assert!(build.extras.before_launch.is_empty(), "{id} should use CMake's automatic regeneration");
        }
    }

    #[test]
    fn generator_change_clears_only_cmake_state_at_launch() {
        let root = tempfile::tempdir().unwrap();
        let build = root.path().join("cmake-build-debug");
        fs::create_dir_all(build.join("CMakeFiles")).unwrap();
        fs::write(build.join("CMakeCache.txt"), "CMAKE_GENERATOR:INTERNAL=Unix Makefiles\n").unwrap();
        fs::write(build.join("keep.txt"), "artifact").unwrap();
        fs::write(root.path().join("CMakeLists.txt"), "project(app)").unwrap();
        let mut configs = Vec::new();
        detect(
            root.path(),
            Some(&[CMakeProfile { generator: "Ninja".into(), ..Default::default() }]),
            None,
            &mut configs,
        );
        assert!(build.join("CMakeCache.txt").exists());
        prepare(&configs[0], root.path()).unwrap();
        assert!(!build.join("CMakeCache.txt").exists());
        assert!(!build.join("CMakeFiles").exists());
        assert!(build.join("keep.txt").exists());
        assert!(build.join(".cmake/api/v1/query/client-openclion/codemodel-v2").exists());
    }
}
