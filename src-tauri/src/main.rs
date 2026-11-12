// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Started by `ssh` / `git` as their askpass program: answer and exit, no UI.
    if let Some(code) = ide_ssh::askpass::run_helper() {
        std::process::exit(code);
    }
    // Before GTK and WebKit start: pick the WebView rendering path this host can carry.
    #[cfg(target_os = "linux")]
    openclion_lib::linux_rendering::tune();
    openclion_lib::run()
}
