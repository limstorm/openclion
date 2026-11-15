fn main() {
    let root =
        dunce::canonicalize(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()).expect("worktree root");
    println!("cargo:rustc-env=OPENCLION_WORKTREE_ROOT={}", root.display());
    // Windows (MSVC): link the Common-Controls v6 manifest into every binary of
    // this package, not only the app's resources, so test executables such as
    // `export_bindings` can load comctl32 and don't exit with
    // STATUS_ENTRYPOINT_NOT_FOUND.
    let mut windows = tauri_build::WindowsAttributes::new();
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        windows = tauri_build::WindowsAttributes::new_without_app_manifest();
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows)).expect("tauri build");
    strip_cef_runtime();
}

/// Strips the CEF runtime the `cef-dll-sys` build script copied next to the binaries.
///
/// That script installs the distribution as published, and `libcef.so` ships with its
/// symbol table: 1.45 GiB against 272 MiB for the same library stripped. Chromium does not
/// need the symbols, and a build tree of 1.45 GiB per profile is not usable. Building with
/// `CEF_KEEP_SYMBOLS=1` keeps them (for symbolizing Chromium crashes).
///
/// Runs after `cef-dll-sys`: it is a dependency of `tauri-runtime-cef`, so its build script
/// has already copied the library by the time this one runs.
fn strip_cef_runtime() {
    if std::env::var_os("CEF_KEEP_SYMBOLS").is_some() {
        return;
    }
    let Ok(out_dir) = std::env::var("OUT_DIR") else { return };
    // target/<profile>/build/<package>-<hash>/out -> target/<profile>
    let Some(profile_dir) = std::path::Path::new(&out_dir).ancestors().nth(3) else { return };
    let library = profile_dir.join("libcef.so");
    let Ok(metadata) = std::fs::metadata(&library) else { return };
    // Already stripped (or a distribution that never carried symbols): nothing to do.
    if metadata.len() < 400 * 1024 * 1024 {
        return;
    }
    println!("cargo:warning=stripping {} (set CEF_KEEP_SYMBOLS=1 to keep its symbols)", library.display());
    match std::process::Command::new("strip").arg("--strip-unneeded").arg(&library).status() {
        Ok(status) if status.success() => {}
        Ok(status) => println!("cargo:warning=`strip` failed ({status}), {library:?} keeps its symbols"),
        Err(e) => println!("cargo:warning=cannot run strip ({e}), {library:?} keeps its symbols"),
    }
}
