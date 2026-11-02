//! The model against a real CMake run: the File API reply depends on the CMake
//! version, so a hand-written fixture cannot prove the parser. Skipped when no
//! `cmake` is installed (CI images that only run `cargo test` without a toolchain).

use std::fs;
use std::path::Path;
use std::process::Command;

fn cmake_available() -> bool {
    Command::new("cmake").arg("--version").output().is_ok_and(|o| o.status.success())
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

#[test]
fn a_real_configure_produces_the_targets_the_tool_window_lists() {
    if !cmake_available() {
        eprintln!("skipping: cmake is not installed");
        return;
    }
    let project = tempfile::tempdir().unwrap();
    let root = project.path();
    write(
        &root.join("CMakeLists.txt"),
        "\
cmake_minimum_required(VERSION 3.20)
project(smoke LANGUAGES CXX)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
add_library(greeter STATIC src/greeter.cpp)
target_include_directories(greeter PUBLIC include)
add_executable(app src/main.cpp)
target_link_libraries(app PRIVATE greeter)
",
    );
    write(&root.join("src/main.cpp"), "int main() { return 0; }\n");
    write(&root.join("src/greeter.cpp"), "#include \"greeter.h\"\n");
    write(&root.join("include/greeter.h"), "#pragma once\n");

    // What the tool window's Reload does: ask for the codemodel, then configure.
    let build = root.join("cmake-build-debug");
    ide_cmake::request(&build).unwrap();
    let status = Command::new("cmake")
        .args(["-S", ".", "-B", "cmake-build-debug", "-DCMAKE_BUILD_TYPE=Debug", "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON"])
        .current_dir(root)
        .status()
        .unwrap();
    assert!(status.success(), "cmake configure failed");

    let model = ide_cmake::load(root, None);
    assert!(model.has_project);
    let build = model.builds.iter().find(|b| b.dir == "cmake-build-debug").expect("the configured build dir");
    assert!(build.configured);
    assert_eq!(build.build_type.as_deref(), Some("Debug"));
    assert!(build.compile_commands.is_some(), "CMake exports the compile database");

    let names: Vec<&str> = build.targets.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"app"), "{names:?}");
    assert!(names.contains(&"greeter"), "{names:?}");
    let app = build.targets.iter().find(|t| t.name == "app").unwrap();
    assert_eq!(app.kind, ide_cmake::CmakeTargetKind::Executable);
    let artifact = app.artifact.as_deref().unwrap_or_default();
    assert!(artifact.ends_with("/app") || artifact.ends_with("app.exe"), "{artifact}");
    assert!(app.sources.iter().any(|s| s.ends_with("src/main.cpp")), "{:?}", app.sources);
    let greeter = build.targets.iter().find(|t| t.name == "greeter").unwrap();
    assert_eq!(greeter.kind, ide_cmake::CmakeTargetKind::StaticLibrary);
    assert!(greeter.sources.iter().any(|s| s.ends_with("src/greeter.cpp")));
    assert_eq!(model.executables().count(), 1);
}
