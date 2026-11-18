# OpenCLion Native Shell

The Tauri/React workbench replaced by a **pure-Rust IDE** built with
[Native Shell (nsh)](https://nsh.dev/zh-CN/docs/). Every pixel of the
workbench — title bar, tool-window stripes, project tree, editor tabs, code
editor, CMake tool window, terminal and status bar — is a native view made by
Rust code. The app links no WebView and no JavaScript engine.

```
native-shell/
├── nsh.json                 app id, window (1400×900), Islands Light tokens
├── Cargo.toml               nsh-core / nsh-base / nsh-platform-* only
├── src/
│   ├── lib.rs               #[component] App, window chrome, shortcuts
│   ├── main.rs              cargo run on the desktop backend
│   ├── model.rs             workspace tree, tabs, search, terminal, signals
│   ├── theme.rs             Islands Light/Dark palette + bundled fonts
│   ├── icons.rs             expui SVG icons inlined as SVG elements
│   └── ui/
│       ├── mod.rs           workbench frame (frame → islands → status bar)
│       ├── titlebar.rs      project widget, configuration, run/debug/search
│       ├── rail.rs          left and right tool-window stripes
│       ├── tree_panel.rs    Project / Commit / Structure / Bookmarks / Search
│       ├── editor_panel.rs  editor tabs + nsh Editor
│       ├── panels.rs        CMake, Notifications, AI, Database, Terminal…
│       ├── status.rs        breadcrumbs and editor details
│       └── overlay.rs       Quick Open, project menu, Settings, toast
├── assets/                  Inter, JetBrains Mono, expui icons, nsh themes
├── demo/acme-api/           the workspace in the screenshots
└── tests/shell.rs           headless nsh tests (5)
```

## What was ported

| Web frontend (`main`)                                | Native shell                   |
| ---------------------------------------------------- | ------------------------------ |
| `src/workbench/TitleBar.tsx` + `TitleBar.module.css` | `ui/titlebar.rs`               |
| `src/workbench/toolwindows/ToolWindows.tsx`          | `ui/rail.rs`, `ui/panels.rs`   |
| `src/features/project-tree/ProjectView.tsx`          | `ui/tree_panel.rs`             |
| `src/workbench/EditorArea.tsx`, `src/ui/Tabs.tsx`    | `ui/editor_panel.rs`           |
| `src/workbench/StatusBar.tsx`                        | `ui/status.rs`                 |
| `src/ui/Menu.tsx`, `Popup`, `Dialog`                 | `ui/overlay.rs`                |
| `src/styles/tokens.css` Islands Light                | `src/theme.rs::Palette::light` |
| `src/ui/expui/**` icons                              | `assets/icons/*.svg` (inlined) |

The layout follows the CSS constants exactly: 40 pt title bar, 6 pt island
gaps, 10 pt island radius, 240 pt left tool window, 296 pt CMake tool window,
38 pt tab strip, 22 pt editor rows, 34 pt status bar.

## Run

Requirements: Rust 1.97+, GTK 4.14 development headers, and the `nsh` CLI
0.6 (`curl -fsSL https://nsh.dev/install.sh | sh`).

```sh
cd native-shell
nsh run                 # builds the nsh runner and opens the app
# or, directly on this machine's backend:
cargo run
```

Open another workspace with `OPENCLION_WORKSPACE=/path/to/project`.

On a machine without system GTK headers (this checkout), the app was built
with a local sysroot and `nsh` unpacked under `.tools/`:

```sh
export PATH="$PWD/../.tools/nsh/bin:$PATH"
export CARGO_HOME="$PWD/../.cargo-home"
export PKG_CONFIG_PATH="$PWD/../.sysroot/usr/lib/x86_64-linux-gnu/pkgconfig"
export LIBRARY_PATH="$PWD/../.sysroot/usr/lib/x86_64-linux-gnu"
cargo run
```

## Interactions

| Action              | How                                                  |
| ------------------- | ---------------------------------------------------- |
| Open a file         | click it in Project, or Ctrl+P then type/Enter/click |
| Edit                | type in the NSH editor; the tab shows a modified dot |
| Save                | Ctrl+S                                               |
| Close tab           | Ctrl+W or the tab's ×                                |
| Find in Files       | Ctrl+Shift+F (or the rail magnifier)                 |
| Terminal            | Ctrl+` or the rail terminal icon                     |
| Toggle tool windows | rail buttons, Ctrl+B (left), Ctrl+J (bottom)         |
| Settings            | title-bar gear or the status-bar gear                |
| Open project        | project name → Open… (system folder picker)          |

## Tests

```sh
cd native-shell
cargo test
```

Five headless tests run the real nsh engine and press real views: the
workbench mounts, a tree file opens in the editor, a folder expands, Find in
Files opens, and the terminal panel opens.

## Theme

Islands Light is the target and the default. `theme.rs` also carries the
Islands Dark palette and the workbench re-colours from it, but the _native
editor and GTK scroll views_ keep their light appearance when the theme is
switched at run time (GTK scroll widgets are created light). Dark mode is
therefore marked "on the roadmap" in Settings rather than shipping a
half-coloured editor.

## Screenshots

|                                             |                                             |
| ------------------------------------------- | ------------------------------------------- |
| ![Workbench](docs/native-shell-light.png)   | ![Terminal](docs/native-shell-terminal.png) |
| ![Settings](docs/native-shell-settings.png) |                                             |
