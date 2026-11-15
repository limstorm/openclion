# The CEF runtime branch (`cef`)

This branch replaces WebKitGTK with Chromium: Tauri 2 (wry) → Tauri 3 with
[`tauri-runtime-cef`](https://crates.io/crates/tauri-runtime-cef) 3.0.0-alpha.7. The goal is an
engine whose scrolling, caret and text rendering do not depend on WebKitGTK's compositor, which
on a GPU-less Linux host costs ~30 ms per frame (see `docs/perf/webkit-wayland.md`).

## What changed

| Area                    | Change                                                                                                                                                                                                     |
| ----------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/Cargo.toml`  | `tauri` 3.0.0-alpha.6 (`gtk4`, `specta`, `unstable`), `tauri-build` 3.0.0-alpha.4, plugins 3.0.0-alpha.x, `tauri-runtime-cef` 3.0.0-alpha.7                                                                |
| runtime type            | Tauri 3's `Builder` is type-erased (`DynRuntime`), so command signatures and `AppHandle` types only needed `tauri::Wry` → `tauri::DynRuntime`                                                              |
| `src-tauri/src/main.rs` | `#[tauri_runtime_cef::cef_entry_point]`: Chromium starts this same executable for its renderer/GPU/utility processes                                                                                       |
| `src-tauri/src/lib.rs`  | `Builder::default().runtime(Cef::default())`; window focus is forwarded to the webview (Chromium's window is a separate X11 child)                                                                         |
| dialogs                 | `tauri-plugin-dialog` with `default-features = false, features = ["xdg-portal"]`: its default `gtk3` feature loads GTK3, which then wins symbol resolution over GTK4 and crashes the process at `gtk_init` |
| `tauri-specta`          | vendored in `vendor/tauri-specta` (2.0.0-rc.25) with only its `tauri` dependency moved to 3.0.0-alpha.6, wired with `[patch.crates-io]` — upstream has no Tauri 3 release yet                              |
| automation bridge       | `tauri-plugin-mcp-bridge` is Tauri 2 only, so it is gone; debug builds expose the Chrome DevTools protocol on `127.0.0.1:9222` instead (`Cef::remote_debugging`)                                           |
| WebKit tuning           | `linux_rendering::tune()` is no longer called: `WEBKIT_DISABLE_COMPOSITING_MODE` has no meaning under Chromium                                                                                             |

## Building

The `cef` crate links against a CEF binary distribution that it would normally download, but its
index (`https://cef-builds.spotifycdn.com/index.json`) has grown past the 10 MiB limit its HTTP
client allows, so the download step fails with `response body is larger than request limit`.
Point `CEF_PATH` at an unpacked distribution instead:

```bash
# 154.0.34 is what cef-dll-sys 154.5.0+154.0.34 expects
curl -L -O "https://cef-builds.spotifycdn.com/cef_binary_154.0.34%2Bg14c5a08%2Bchromium-154.0.8037.98_linux64_minimal.tar.bz2"
sudo mkdir -p /opt/cef/154.0.34
sudo tar xjf cef_binary_154.0.34*.tar.bz2 -C /opt/cef/154.0.34
cd /opt/cef/154.0.34
sudo mv cef_binary_*_linux64_minimal cef_linux_x86_64     # the name the crate resolves
cd cef_linux_x86_64
sudo mv Release/* . && rmdir Release                       # the layout the crate expects
sudo mv Resources/* . && rmdir Resources
printf '{"type":"minimal","name":"cef_binary_154.0.34","sha1":"15c91ad767097217b2e82ba8b933fe30b134be69"}\n' | sudo tee archive.json

export CEF_PATH=/opt/cef
cargo build -p openclion
```

GTK4 development headers are now required (`libgtk-4-dev`); WebKitGTK is not.

Chromium's setuid sandbox helper must be owned by root with mode 4755 or the process aborts at
start-up (`The SUID sandbox helper binary was found, but is not configured correctly`). Every
build copies a fresh helper without the setuid bit, so re-run this after each build:

```bash
scripts/cef-sandbox.sh            # chown root:root + chmod 4755 on target/debug/chrome-sandbox
```

Chromium copies its runtime (~1.5 GiB: `libcef.so`, `icudtl.dat`, `locales/`, …) next to the
executable on every build, and writes a profile under `~/.cache/<identifier>/cef/`.

## What works

- The app starts, loads the frontend from the Tauri asset protocol, and talks to Rust over IPC.
- The window renders the full workbench; opening a project, indexing and `git` state work.
- Mouse: hover, clicks and wheel reach the page.
- `cargo test -p openclion --lib` (73) and the frontend suite (477 files, 5125 tests) pass.

## Window identity (taskbar icon)

CEF's window is created outside GTK's own window tracking and ends up with an empty
`WM_CLASS` (`"", ""`), so a shell cannot match it to the `.desktop` entry and shows a generic
icon. `x11_focus::set_wm_class()` sets the hint to `openclion`/`OpenCLion` on this process's
top-level windows (re-applied together with the focus, because GTK rewrites it while the surface
is realised), which matches `StartupWMClass=openclion` in the packaged desktop entry.

## Keyboard and IME

Chromium's browser lives in an **override-redirect X11 child window** of the GTK window
(`CefBrowserHost` in windowed mode), and it handles pointer and key input itself. X delivers key
events only to the window that holds the input focus; a window manager never focuses an
override-redirect child, so the focus stays on the GTK toplevel (or on GTK's own 1×1 focus
window) and **every keystroke is lost** — typing, IME composition and all shortcuts.

`Webview::set_focus()` is not enough: it calls `CefBrowserHost::SetFocus`, which makes Chromium
believe it is focused (`document.hasFocus()` reports `true`) but does not move the X focus. The
runtime has no `SendKeyEvent` forwarding either, so nothing else delivers the keys.

`src-tauri/src/x11_focus.rs` closes the gap:

- it finds this process's own top-level windows through `_NET_WM_PID`, follows the largest mapped
  child down to Chromium's window, and calls `XSetInputFocus` on it;
- `focus_browser_soon()` retries for six seconds after a window is created (the browser window
  appears about a second later);
- `focus_browser_watchdog()` re-applies it while this process owns `_NET_ACTIVE_WINDOW`, because
  GTK takes the X focus back on every activation — a change Tauri's own focus events do not
  report.

Verified on this machine (X11/xrdp, GTK 4.14, IBus):

| Check                                            | Result                                                                     |
| ------------------------------------------------ | -------------------------------------------------------------------------- |
| ASCII typing in the editor                       | `Building protobuf` → `Building protobXYZuf`, caret where the click put it |
| Chinese IME (IBus libpinyin, `nihao` + space)    | commits `你好` at the caret; the status bar column advances by two         |
| `document.hasFocus()` / `document.activeElement` | `true` / the focused `cm-content`                                          |
| Keyboard shortcuts (arrows, Enter, Ctrl+Z)       | work                                                                       |

Without the fix the same keystrokes produce no `keydown` at all while `document.hasFocus()` is
already `true` — that asymmetry is what identified the missing X focus rather than a CEF input
bug.

## Packaging

`tauri build --bundles deb|appimage` does not bundle a CEF app out of the box with the
3.0.0-alpha.6 CLI:

- its CEF copy list still names `libEGL.so`/`libGLESv2.so`, which CEF stopped shipping for Linux
  by 154 (ANGLE lives inside `libcef.so`; only SwiftShader and the Vulkan loader sit beside it),
  so the copy loop stops with `No such file or directory`;
- its AppImage path downloads `linuxdeploy` at build time, which needs network access the build
  host may not have;
- it always `strip`s `libcef.so`, which throws the symbol table of a 1.45 GiB library away.

### The official bundler, made to work

`scripts/cef-bundle-prep.sh` drops two empty files named `libEGL.so`/`libGLESv2.so` into the CEF
distribution: presence is all the copy loop checks, Chromium never opens them on Linux, and an
empty file is as good as absent to `dlopen`. After that:

```bash
scripts/cef-bundle-prep.sh
node scripts/tauri.ts build --bundles deb     # -> target/release/bundle/deb/OpenCLion_0.1.33_amd64.deb (186 MB)
```

That package installs and runs, but it carries the _stripped_ `libcef.so` (272 MiB) and the two
placeholder files.

### The package we ship

`scripts/cef-package.sh` assembles the layout by hand so both sizes and the AppImage are under
control:

```bash
CEF_PATH=/opt/cef cargo build --release -p openclion    # ~21 min, LTO
scripts/cef-package.sh --bundle deb                     # -> 133 MB deb (439 MiB installed)
scripts/cef-package.sh --bundle appimage                # -> 183 MB AppImage
scripts/cef-package.sh --bundle deb --keep-symbols      # debugging variant -> 275 MB (1.6 GiB installed)
```

Everything is stripped by default — the build tree strips `libcef.so` in `build.rs`, the package
strips the application binary on its way in, and `locales/` keeps `en-US`, `zh-CN` and `zh-TW`
instead of the 220 `.pak` files CEF ships (49 MiB). `CEF_LOCALES=all` ships every locale,
`--keep-symbols` (or `CEF_KEEP_SYMBOLS=1` for a build) is the only way to get symbols.

| Bundle                            | Contents                                                                                                                                                                             |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `OpenCLion_<v>_amd64.deb`         | `/usr/share/OpenCLion/` (binary + runtime), `/usr/bin/openclion` symlink, `4500 root:root` sandbox + `postinst`, GTK4/Chromium `Depends:`                                            |
| `OpenCLion_<v>_amd64-symbols.deb` | the same with the unstripped `libcef.so`, for builds that must symbolize Chromium crashes                                                                                            |
| `OpenCLion_<v>_amd64.AppImage`    | `AppDir/usr/bin/` holds the binary and the runtime next to it (`-rpath,$ORIGIN`), `AppRun` execs it; the runtime disables the Chromium sandbox inside an AppImage and warns about it |

All three were installed/started on this machine: the app opens the project, edits files, and
commits `你好` from IBus at the caret.

### Running from a build tree

Two things make `./target/release/openclion` awkward before packaging, and both are handled in
the build now:

- **The sandbox helper.** Chromium requires `chrome-sandbox` to be `root:root 4755`; a build copies
  a fresh one `0755`, and Chromium aborts instead of falling back (`The SUID sandbox helper binary
  was found, but is not configured correctly`). `src-tauri/src/cef_sandbox.rs` checks the helper
  and, when it is not root-owned 4755, asks the runtime for `SandboxPolicy::Disabled` and says so:

  ```
  openclion: …/target/release/chrome-sandbox is not root-owned 4755, so Chromium's sandbox cannot
  start: running without it (an installed package sets this up, see scripts/cef-sandbox.sh)
  ```

  Packages are unaffected: `postinst` installs the helper correctly, so the sandbox stays on.
  `scripts/cef-sandbox.sh` still exists for anyone who wants the sandbox in a dev tree.

- **The size of `libcef.so`.** `cef-dll-sys` copies the distribution as published, symbol table
  included: 1.45 GiB per build profile. `src-tauri/build.rs` now strips it after the copy
  (1.45 GiB → 272 MiB) and prints a warning saying how to keep the symbols:

  ```
  warning: openclion@0.1.33: stripping …/target/release/libcef.so (set CEF_KEEP_SYMBOLS=1 to keep its symbols)
  ```

  `CEF_KEEP_SYMBOLS=1 cargo build …` keeps them; the `--keep-symbols` package build is unaffected
  because `scripts/cef-package.sh` copies from `CEF_PATH`, not from the build tree.

### A note on killing the app

Chromium starts zygote processes that outlive their parent. Killing the main process leaves them
holding the browser singleton, and the next launch fails with
`Runtime(WebviewRuntimeNotInstalled)` and Chromium's `Opening in existing browser session.` line
in the log. Stop everything the process spawned before relaunching:

```bash
ps -eo pid,comm,args | awk '$2=="openclion" || $2=="chrome-sandbox" {print $1}' | xargs -r sudo kill -9
```

Stripping `libcef.so` is _not_ the problem there — the 136 MiB package starts exactly like the
1.6 GiB one once those processes are gone.

## Remaining work on this branch

- The `--mode perf` harness keeps the eager feature path (see `perf(startup)`), unchanged here.
- `tauri build` itself should work again once the bundler's CEF file list matches what CEF ships;
  the script above is the stand-in until then.
