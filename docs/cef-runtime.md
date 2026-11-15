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
start-up (`The SUID sandbox helper binary was found, but is not configured correctly`):

```bash
sudo chown root:root target/debug/chrome-sandbox && sudo chmod 4755 target/debug/chrome-sandbox
```

Chromium copies its runtime (~1.5 GiB: `libcef.so`, `icudtl.dat`, `locales/`, …) next to the
executable on every build, and writes a profile under `~/.cache/<identifier>/cef/`.

## What works

- The app starts, loads the frontend from the Tauri asset protocol, and talks to Rust over IPC.
- The window renders the full workbench; opening a project, indexing and `git` state work.
- Mouse: hover, clicks and wheel reach the page.
- `cargo test -p openclion --lib` (73) and the frontend suite (477 files, 5125 tests) pass.

## Known blocker: keyboard and IME

**Keyboard input does not reach the page.** Typing produces no `keydown` on the page even though
`document.hasFocus()` is `true` and `Input.dispatchKeyEvent` over DevTools works.

Why: `tauri-runtime-cef` runs CEF in windowed mode and reparents Chromium's browser into an
override-redirect X11 child window of the GTK4 toplevel. X delivers key events only to the window
that holds the input focus; the window manager cannot focus an override-redirect child, and the
runtime contains no `SendKeyEvent` forwarding (`grep send_key_event` finds nothing in
3.0.0-alpha.7). Pointer events work because X routes them to the window under the cursor.

Evidence gathered on this machine (X11/xrdp, GTK 4.14):

- A minimal Tauri 3 + CEF app with a focused `<textarea>` shows no typed text.
- The same keystrokes type into the WebKit build of the same app (`main` branch binary).
- Over DevTools: `document.hasFocus() === true`, `document.activeElement` is the search input, and
  a `keydown` listener records nothing while `xdotool key a` runs.
- `XSetInputFocus` on Chromium's child window succeeds and is accepted, but no key event reaches
  the renderer.

Until upstream forwards key events (or switches Linux to offscreen rendering with input
forwarding), the "typing and Chinese IME work" acceptance criterion cannot be met on this branch.
The two ways forward:

1. Wait for a `tauri-runtime-cef` release that forwards `KeyboardInput` (winit → `SendKeyEvent`),
   which is also what IBus/fcitx integration needs for `compositionstart/update/end`.
2. Vendor the runtime and add that forwarding here — key mapping tables (winit → CEF key codes,
   modifiers, character text) plus the IBus path, which is a substantial change.
