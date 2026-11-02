<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="src-tauri/icons/source/openclion-wordmark-dark.svg">
    <img src="src-tauri/icons/source/openclion-wordmark.svg" alt="OpenCLion" height="56">
  </picture>
</p>

<p align="center">
  A CLion-style C/C++ IDE, rebuilt in Rust.
</p>

OpenCLion is a fork of [OpenIDEA](https://github.com/l0ng-ai/openidea) — the
IntelliJ IDEA experience rewritten as a native Tauri app — re-targeted from
Java/Kotlin to C and C++: CMake projects, clangd code insight and LLDB
debugging.

The workbench is the original one, unchanged: the same tool windows, keymaps,
themes, search-everything and editor. What changed is what the IDE _knows about_
— the project model, build, tests, debugger and inspections speak CMake and
Clang.

## What makes it a C/C++ IDE

| Area                  |                                                                                                                                                                                             |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **CMake tool window** | Build directories (one per CMake profile), codemodel targets read through the CMake File API, `compile_commands.json` and `CMakePresets.json`; Reload / Build / Run / CTest from the window |
| **Project model**     | `CMakeLists.txt` and `CMakePresets.json` define source roots; `cmake-build-*`, `build/`, `out/` stay out of the index                                                                       |
| **Build**             | Build Project runs `cmake -S . -B <dir> -DCMAKE_EXPORT_COMPILE_COMMANDS=ON`, then `cmake --build <dir>`; GCC/Clang diagnostics land in the Problems view                                    |
| **Code insight**      | clangd pointed at the project's compile database (`--compile-commands-dir=${compileCommands}`), with clang-tidy inspections                                                                 |
| **Run / Debug**       | One configuration per CMake executable target, built as a before-launch task and debugged with `lldb-dap`; CTest configurations report into the test tree                                   |
| **Settings**          | Build, Execution, Deployment › CMake (profiles) and › Toolchains (CMake, Ninja/Make, lldb-dap, clangd, clang-tidy)                                                                          |
| **New projects**      | C++ executable and C++ library CMake templates                                                                                                                                              |

## Build

```bash
pnpm install
pnpm dev:app          # desktop app (Tauri)
pnpm dev              # UI only, in a browser, with a sample C++/CMake project
pnpm test             # frontend unit tests
pnpm e2e              # Playwright, plain-browser mode
cargo test --workspace
```

Linux build dependencies (Debian/Ubuntu):

```bash
sudo apt install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
  libxdo-dev libssl-dev patchelf build-essential cmake ninja-build clang clangd lldb
```

## Where the C/C++ work lives

- `crates/ide-cmake` — the CMake project model: build directories, File API
  codemodel, presets, compile database.
- `crates/ide-run/src/detect/cmake.rs` — CMake run configurations (configure,
  build, CTest, per-target build and run).
- `crates/ide-host/src/ops/cmake.rs`, `src-tauri/src/commands/cmake.rs` — the
  `cmake_model` command the tool window reads.
- `crates/ide-build/src/compile` — Build Project for CMake and the GCC/Clang
  diagnostic parser.
- `src/features/cmake` — the CMake tool window, the CMake Application run
  configuration type and the Toolchains settings page.

[`docs/porting-idea-to-clion.md`](docs/porting-idea-to-clion.md) describes the
architecture and maps the upstream IDEA-oriented code onto CLion features.

## License

[Apache 2.0](LICENSE), as the upstream project. Bundled third-party assets are
listed in [`THIRD_PARTY_LICENSES.md`](THIRD_PARTY_LICENSES.md). OpenCLion is a
fork of OpenIDEA; see [`NOTICE`](NOTICE) for attribution.

<sub>Not affiliated with JetBrains. CLion, IntelliJ and IntelliJ IDEA are
trademarks of JetBrains s.r.o.</sub>
