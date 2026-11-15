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

`tauri build --bundles deb` cannot finish a CEF bundle with the 3.0.0-alpha.6 CLI yet: its
bundler expects `libEGL.so`/`libGLESv2.so` beside `libcef.so` (CEF 154 for Linux ships ANGLE
inside `libcef.so` and only `libvk_swiftshader.so` + `libvulkan.so.1` next to it), so it fails with
`No such file or directory` while copying the runtime. `scripts/cef-deb.sh` assembles the same
package layout by hand:

```bash
CEF_PATH=/opt/cef cargo build --release -p openclion   # ~21 min, LTO
scripts/cef-deb.sh /opt/cefnew/cef_linux_x86_64        # -> target/release/bundle/deb/OpenCLion_0.1.33_amd64.deb
```

What it produces, and why:

| Piece                   | Note                                                                                                                                    |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| `/usr/share/OpenCLion/` | the binary and the whole CEF runtime; the binary is linked with `-rpath,$ORIGIN`, so `libcef.so` loads from its own directory           |
| `/usr/bin/openclion`    | symlink into that directory                                                                                                             |
| `chrome-sandbox`        | installed `4755 root:root` in the archive itself, and `postinst` re-applies it (dpkg does keep the bit here, but a build tree does not) |
| `libcef.so`             | `strip --strip-unneeded`: 1.45 GiB → 272 MiB installed (`CEF_DEB_KEEP_SYMBOLS=1` keeps the symbols)                                     |
| `Depends:`              | GTK4 + the Chromium runtime libraries; the bundler's defaults still name WebKitGTK                                                      |

Result: **142 MB** compressed, **502 MB** installed. Installing it and launching `openclion` starts
the app, opens the project, and typing works — including the IBus path (`nihao` + space commits
`你好` in the editor).

## Remaining work on this branch

- The `--mode perf` harness keeps the eager feature path (see `perf(startup)`), unchanged here.
- `tauri build` itself should work again once the bundler's CEF file list matches what CEF ships;
  the script above is the stand-in until then.
