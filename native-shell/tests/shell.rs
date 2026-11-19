//! Headless tests of the native shell: the workspace tree, opening files and
//! switching tabs run on nsh's real engine without a window.

use nsh_core::geometry::{Size, px};
use nsh_engine::{EngineConfig, Root};
use nsh_testing::Headless;

fn start() -> Headless {
    // Headless runs must not leave clangd processes behind.
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        std::env::set_var("OPENCLION_NO_LSP", "1");
        std::env::set_var("OPENCLION_NO_TERMINAL", "1");
        std::env::set_var("OPENCLION_NO_DEBUG", "1");
    });
    let mut config = EngineConfig::new(Root::of::<openclion_native::App>());
    config.viewport = Size::new(px(1400.0), px(900.0));
    Headless::start(config).unwrap()
}

#[test]
fn the_workbench_mounts_the_project() {
    let mut app = start();
    app.wait_for("Project").unwrap();
    app.wait_for("acme-api").unwrap();
    app.wait_for("main.cpp").unwrap();
    app.wait_for("Select Configuration").unwrap();
}

#[test]
fn opening_a_tree_file_shows_it_in_the_editor() {
    let mut app = start();
    app.wait_for("greeter.cpp").unwrap();
    app.press("greeter.cpp").unwrap();
    app.wait_for("Greeter::Greeter(std::string name) : name_(std::move(name)) {}").unwrap();
    app.wait_for("text:std::string Greeter::greet() const {").unwrap();
}

#[test]
fn the_bottom_panel_opens_and_shows_the_terminal() {
    let mut app = start();
    app.wait_for("Project").unwrap();
    app.press("id:rail-terminal").unwrap();
    app.wait_for("Terminal").unwrap();
}

#[test]
fn expanding_a_folder_reveals_its_files() {
    let mut app = start();
    app.wait_for("include").unwrap();
    app.press("include").unwrap();
    app.wait_for("greeter.h").unwrap();
}

#[test]
fn find_in_files_opens_from_the_rail() {
    let mut app = start();
    app.wait_for("Project").unwrap();
    app.press("id:rail-search").unwrap();
    app.wait_for("Find in Files").unwrap();
    app.wait_for("Search in files").unwrap();
}

#[test]
fn the_cmake_panel_lists_the_backends_targets() {
    let mut app = start();
    app.wait_for("Project").unwrap();
    app.press("id:rail-cmake").unwrap();
    app.wait_for("CMake").unwrap();
    app.wait_for("Targets").unwrap();
    app.wait_for("acme_tests").unwrap();
    app.wait_for("executable").unwrap();
}

#[test]
fn the_commit_panel_reads_git_status() {
    let mut app = start();
    app.wait_for("Project").unwrap();
    app.press("id:rail-commit").unwrap();
    app.wait_for("Commit").unwrap();
}

#[test]
fn the_project_header_opens_the_new_file_dialog() {
    let mut app = start();
    app.wait_for("Project").unwrap();
    app.press("id:project-new-file").unwrap();
    app.wait_for("New File").unwrap();
}

#[test]
fn the_run_widget_opens_the_configuration_chooser() {
    let mut app = start();
    app.wait_for("Select Configuration").unwrap();
    app.press("id:run-config-widget").unwrap();
    app.wait_for("Run Configurations").unwrap();
}

#[test]
fn choosing_a_run_target_updates_the_title_bar_widget() {
    let mut app = start();
    app.wait_for("Select Configuration").unwrap();
    app.press("id:run-config-widget").unwrap();
    app.wait_for("Run Configurations").unwrap();
    app.press("id:run-target-acme_tests").unwrap();
    app.wait_for("acme_tests (Debug)").unwrap();
}

#[test]
fn the_menu_button_opens_the_application_menu() {
    let mut app = start();
    app.wait_for("Project").unwrap();
    app.press("id:app-menu-button").unwrap();
    app.wait_for("New File…").unwrap();
}

#[test]
fn the_git_rail_button_opens_the_git_panel() {
    let mut app = start();
    app.wait_for("Project").unwrap();
    app.press("id:rail-git").unwrap();
    app.wait_for("Worktrees").unwrap();
}

#[test]
fn the_right_rail_opens_ssh_and_notebooks() {
    let mut app = start();
    app.wait_for("Project").unwrap();
    app.press("id:rail-ssh").unwrap();
    app.wait_for("SSH").unwrap();
    app.press("id:rail-notebook").unwrap();
    app.wait_for("Notebooks").unwrap();
}

#[test]
fn selecting_a_cmake_target_updates_the_selection() {
    let mut app = start();
    app.wait_for("Project").unwrap();
    app.press("id:rail-cmake").unwrap();
    app.wait_for("acme_tests").unwrap();
    app.press("id:cmake-target-acme_tests").unwrap();
    app.wait_for("Selected CMake target acme_tests").unwrap();
}
