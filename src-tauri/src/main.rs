// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Chromium (CEF) starts helper processes from this same executable: the attribute
/// sends them to CEF's own entry point instead of running the IDE twice.
#[tauri_runtime_cef::cef_entry_point]
fn main() {
    // Started by `ssh` / `git` as their askpass program: answer and exit, no UI.
    if let Some(code) = ide_ssh::askpass::run_helper() {
        std::process::exit(code);
    }
    // The rendering path is Chromium's own business now: `linux_rendering` tuned
    // WebKitGTK's compositor, which this shell no longer uses.
    openclion_lib::run()
}
