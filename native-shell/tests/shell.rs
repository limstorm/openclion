//! Headless tests of the native shell: the workspace tree, opening files and
//! switching tabs run on nsh's real engine without a window.

use nsh_engine::{EngineConfig, Root};
use nsh_testing::Headless;

fn start() -> Headless {
    Headless::start(EngineConfig::new(Root::of::<openclion_native::App>())).unwrap()
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
    app.wait_for("Run a shell command").unwrap();
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
