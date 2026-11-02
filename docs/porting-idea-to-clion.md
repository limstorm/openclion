# Porting OpenIDEA (IntelliJ-style) to OpenCLion (CLion-style)

This document explains how the upstream project is put together, what a fork has
to touch to turn it into a C/C++ IDE, and what OpenCLion changed. It is written
for the next person who forks this code base.

## 1. How the upstream IDE is put together

The app is two halves plus a shared command vocabulary.

```
React + CodeMirror (src/)          ← UI, contribution points, no disk access
        │  commands.* / ide://event (tauri-specta generated src/bindings.ts)
src-tauri/                         ← thin Tauri adapter: #[tauri::command] wrappers
crates/ide-host                    ← Host: window → project, terminals, processes,
        │                            debug sessions, remote server (openclion-server)
crates/ide-*                       ← feature crates (one per subsystem)
```

Three rules keep it tractable:

1. **The UI shell is language agnostic.** `Workbench`, `ToolWindows`, `Settings`,
   `keymap`, `inspections` all assemble themselves from _contribution points_
   (`registerToolWindow`, `registerSettingsPage`, `registerRunConfigType`,
   `registerEditorContribution`, `registerAction`, …). Nothing in the shell knows
   what Java is, so a C/C++ fork changes feature _registrations_, not the shell.
2. **The backend owns everything that touches disk.** The frontend asks for
   directories, documents, run configurations, the CMake model; it never reads
   files itself. IPC types live in `ide-core` and are exported to TypeScript by
   `cargo test -p openclion --test export_bindings` (`pnpm gen:bindings`).
3. **Features are declarative where possible.** Run configurations, language
   definitions, debug adapters and tool installers are data (`languages/*.toml`,
   `RunConfig` records, `ToolSpec` entries), so a fork can re-target them without
   writing a feature.

### Command path

```
commands.cmakeModel()            // src/bindings.ts
  → invoke("cmake_model")        // src-tauri/src/lib.rs (trust check, remote routing)
  → commands::cmake::cmake_model // src-tauri/src/commands/cmake.rs
  → ide_host::ops::cmake::model  // shared with openclion-server (remote dev)
  → ide_cmake::load(root, profiles)
```

Adding a command means: implement it in `crates/ide-host/src/ops/<area>.rs`
(+ `register` for remote development), wrap it in
`src-tauri/src/commands/<area>.rs`, add it to `collect_commands!` in
`src-tauri/src/commands/mod.rs`, regenerate bindings, call it from `src/`.
Read-only commands must also be allowed in `crates/ide-host/src/trust.rs`
(Restricted Mode).

### Project model

`Host::open_project` resolves roots (`crates/ide-workspace/src/roots`), builds
the language registry, the document store, the background index and the LSP
manager. `ide-run::detect` then emits the run configurations from what it finds
on disk (`Cargo.toml`, `pom.xml`, `package.json`, `CMakeLists.txt`, …), and
`ide-lsp` starts language servers according to `languages/*.toml`.

## 2. Forking: the mechanical part

| Item                                  | Where                                                                                                                                                                                             |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Product name, window title, bundle id | `src-tauri/tauri.conf.json` (`productName`, `identifier`, `mainBinaryName`), `src-tauri/tauri.linux.conf.json`                                                                                    |
| Rust package/binary names             | `src-tauri/Cargo.toml` (`openclion`, `openclion_lib`), `crates/ide-host/Cargo.toml` (`openclion-server`), `Cargo.toml` workspace                                                                  |
| Update feed                           | remove `plugins.updater` — a fork must never install upstream binaries                                                                                                                            |
| Config/data directories               | `<config_dir>/settings.toml`, project dir `.openclion/`, localStorage keys (`openclion.layout.v1:`)                                                                                               |
| Strings and i18n                      | `src/platform/locales/*.json`; every new English literal in UI code needs an entry in `src/platform/locales/text/zh.json` (the `textCatalog` test enforces it)                                    |
| Icons, wordmark, About                | `src-tauri/icons/source/*`, `src/ui/icons.tsx`, `src/features/app/AboutDialog.tsx`                                                                                                                |
| JVM-only payloads                     | `plugins/{java-usages,kotlin-bridge,lombok}` and the paths baked into `crates/ide-sdk/src/java.rs` use `ai.openidea.*` class names — do not rename those strings unless you also rebuild the jars |

`NOTICE` must keep the upstream attribution; `LICENSE` stays Apache 2.0.

## 3. IDEA → CLion: what has to change

| IDEA concept (upstream)                             | CLion equivalent                                                                          | Where                                                    |
| --------------------------------------------------- | ----------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| Maven / Gradle tool windows, build model            | CMake tool window, CMake project model                                                    | new `crates/ide-cmake`, `src/features/cmake`             |
| Project JDK, `LanguageLevel`, Maven/Gradle settings | Toolchains: CMake, Ninja/Make, lldb-dap, clangd                                           | `tools.<command>.path` settings + Toolchains page        |
| Java roots (`src/main/java`, `pom.xml`)             | CMake roots (`CMakeLists.txt`, `src/`, `include/`, `tests/`) and excluded `cmake-build-*` | `crates/ide-workspace/src/roots/detect.rs`               |
| Build Project: Maven/Gradle/Cargo/Go/TS             | Build Project: CMake (`cmake -S … -B`, `cmake --build`) with GCC/Clang output parsing     | `crates/ide-build/src/compile`                           |
| jdtls + class files, Java inspections               | clangd + compile database + clang-tidy                                                    | `languages/{c,cpp}.toml`, `crates/ide-lsp/src/launch.rs` |
| JUnit / Gradle test tree                            | CTest configurations (`ctest --test-dir`) in the same test tree                           | `crates/ide-run/src/detect/cmake.rs`                     |
| Java debugger (JDWP via jdtls)                      | lldb-dap (native targets)                                                                 | `languages/cpp.toml` `[debugger]`, `ide-dap`             |
| Spring Boot / Tomcat / JVM run types                | CMake Application run type + per-target native configs                                    | `src/features/cmake/runTypes.ts`                         |
| Java code style / Lombok / Kotlin bridge            | (no equivalent yet)                                                                       | roadmap                                                  |

## 4. What OpenCLion changed

**New: the CMake project model** (`crates/ide-cmake`)

- `load(root, profiles)` → `CmakeModel { sourceDir, hasProject, hasPresets,
  presets, builds }`, where each `CmakeBuild` carries the build directory, the
  generator/build type read from `CMakeCache.txt`, `configurationTypes`, the
  compile database and the targets.
- Targets come from the CMake **File API** codemodel (`index-*.json` →
  codemodel v2 → per-target JSON), with kind, artifact, absolute sources and
  folder; `targets_for(build, source_root, configuration)` filters a multi-config
  generator and rejects a stale reply from another source tree.
- `presets()` reads `CMakePresets.json` (`${sourceDir}`, `${presetName}`,
  `${buildType}` expanded); `request(build)` writes the
  `.cmake/api/v1/query/client-openclion/codemodel-v2` query before a configure.

**Run configurations** (`crates/ide-run/src/detect/cmake.rs`)

- Now consumes `ide-cmake` instead of its own File API parser, so the CMake tool
  window and the Run tool window can never disagree.
- Still emits: configure (`cmake -S . -B <dir>`), build (`cmake --build <dir>`),
  CTest (`ctest --test-dir <dir>`), and per executable target a build config plus
  a `native` run config (`debugAdapter: "lldb-dap"`, `before_launch` → its build,
  `params.cmakeSources` for "Run file in target", source map for lldb-dap).

**Tool window** (`src/features/cmake`)

- `index.ts` registers the CMake tool window on the right stripe only while the
  project has a `CMakeLists.txt` (same pattern as Maven/Gradle), plus two actions
  (`cmake.reloadProject`, `cmake.buildProject`) and the `build.cmake` context.
- `CMakeView.tsx` renders build directories → targets / `compile_commands.json`
  and presets; double-click runs an executable (building first), builds other
  targets, opens files; the header reloads (configure) and builds; the context
  menu offers Run / Debug / Build Target / Run CTest.
- `configs.ts` maps a build directory or target to the run configuration that
  performs the action, so the window never invents a command line.
- `modes`: the model is reloaded on `fsChanged` (`CMakeCache.txt`,
  `compile_commands.json`, `CMakeLists.txt`), on `runExited` for CMake sessions,
  and on `cmake.*` settings changes.

**IPC** — `cmake_model` (`crates/ide-host/src/ops/cmake.rs`,
`src-tauri/src/commands/cmake.rs`), registered for remote development too.

**Build Project** (`crates/ide-build/src/compile`)

- `BuildSystem::CMake` with the same "configure when the cache is missing, then
  build" ordering, `--clean-first` for Rebuild.
- `Format::GccClang` parses `path:line[:col]: error|warning|note: fatal error:
  message` into the Problems view / editor squiggles.

**Roots** (`crates/ide-workspace/src/roots/detect.rs`)

- `CMakeLists.txt` / `CMakePresets.json` are build files (they trigger a root
  recompute); a flat CMake project is one source root, otherwise `src`,
  `include` and `tests`; `cmake-build-*` and `build/` are excluded.

**clangd wiring** (`crates/ide-lsp/src/launch.rs`, `languages/{c,cpp}.toml`)

- Server arguments now expand `${root}` and `${compileCommands}` — the latter is
  the directory of the project's `compile_commands.json` (CMake writes it into
  the build directory, where clangd does not look by itself).
- `clangd` is launched with `--clang-tidy` and
  `--compile-commands-dir=${compileCommands}`, giving CLion-style clang-tidy
  warnings as inspections.

**Tool paths** (`crates/ide-sdk/src/apply.rs`) — `tools.<command>.path` now
applies to any bare run command (CMake, `ctest`, Ninja, Make), not just Godot,
which is what the Toolchains page edits.

**UI**

- `src/features/cmake/runTypes.ts` — "CMake Application" in
  Run › Edit Configurations › Add, and the edit form of detected native target
  configurations (`kind: "native"`), with lldb-dap as the adapter.
- `src/features/cmake/ToolchainSettings.tsx` — Build, Execution, Deployment ›
  Toolchains (CMake, Ninja, Make, lldb-dap, clangd, clang-tidy toggle).
- `src/features/app/NewProject.tsx` — C++ executable / C++ library CMake
  templates (CMakeLists.txt with `CMAKE_EXPORT_COMPILE_COMMANDS`, `src/`,
  `include/`, `.gitignore`).
- `src/features/settings/index.ts` — "Settings for New Projects" opens Toolchains
  instead of the Java page.

## 5. Second milestone: what was added after the first port

- **Debugger Memory view** — `DebugManager::read_memory` (DAP `readMemory`,
  base64 payload, 64 KiB cap) behind `debug_read_memory`; a Memory tab in the
  Debug tool window taking an address or an expression evaluated to one,
  512-byte pages, 16 bytes per row with the ASCII column, unreadable bytes
  shown as `??`, refreshed on every stop.
- **CTest in the test tree** — CTest configurations are test configurations and
  run with `--output-junit <build>/ctest-junit.xml`; the run reads that report
  through `run_test_report_file` (freshness checked, so an older report is
  ignored), the console parser gives live progress, and "Rerun Failed Tests"
  filters with `ctest -R '^(a|b)$'`. Covers `add_test`, `gtest_discover_tests`,
  `catch_discover_tests` and doctest.
- **Compiler kits** — `ide_sdk::cpp::detect` finds the clang/GCC pairs on the
  `PATH` (with Settings › Tools overrides, `--version` probed), pairs each with
  a debugger (`lldb-dap`/`lldb`/`gdb`) and the CMake and Ninja/Make that go with
  it, and the Toolchains page lists them. A CMake profile can select a kit:
  `cCompiler`, `cxxCompiler` and `toolchainFile` become
  `-DCMAKE_C_COMPILER` / `-DCMAKE_CXX_COMPILER` / `-DCMAKE_TOOLCHAIN_FILE`.

* **CMakePresets as profiles** — `CMakeSettings.presets` selects configure
  presets by name; `ide-run` then emits `cmake --preset <name>`, the build preset
  that names it (else `cmake --build <dir> --config <type>`), CTest and the
  targets of its build directory. `CMakePresets.json` `inherits` chains are
  resolved for generator/binaryDir/cacheVariables. Settings › CMake has a preset
  mode with a selection list; the CMake tool window shows the preset build
  directories labelled by preset name.

* **Generate Definition** — the C/C++ declaration at the caret in a header
  (`ide_syntax::cpp_declaration_at`, tree-sitter; `.h` files fall back to the C++
  grammar) is turned into a stub and appended to the matching source file
  through the same `src/` ↔ `include/` pairing as Go to Related Symbol.
  `cpp_generate_definition` in the IPC, `Generate Definition` in the Code menu,
  and `code.generate` (⌘N) falls back to it when clangd has nothing to generate.

* **Valgrind Memcheck** — `Run with Valgrind Memcheck` runs a native target under
  `valgrind --tool=memcheck --xml=yes`, then `src/features/valgrind/report.ts`
  parses the XML and publishes one diagnostic per finding at its innermost
  project frame (Problems view), with a summary notification. The report lives
  in a temporary directory; nothing is written into the project.
* **Sanitizers** — a CMake profile can be built with AddressSanitizer and UBSan
  (a toggle on Settings › CMake that adds `-fsanitize=address,undefined` to the
  profile's compiler flags), and `Run with Sanitizers` runs the binary with
  symbolized ASan/LSan/UBSan runtime options, reads the run console and turns
  each report into a Problems entry. `src/features/sanitizers/report.ts` knows
  ASan, LeakSanitizer, ThreadSanitizer and UBSan banners.

## 6. Deliberate gaps (roadmap)

Ordered by how much they would be missed by a CLion user:

1. **Debugger views beyond memory** — Disassembly, registers and the "inline
   variables" gutter; the Memory view is in, the rest is frontend work in
   `src/features/debugger`.
2. **C++ editor features** — the source ↔ header switch is in (`Go to Related
   Symbol`, `navigate.relatedSymbol`, `ide_host::ops::cpp`); still missing are
   generate definitions and the include fixer. (Formatting already works:
   with no external formatter configured, `Reformat Code` goes through clangd,
   which runs clang-format and honours `.clang-format`.)
3. **Sanitizers / Valgrind** — CLion's Valgrind and sanitizer integrations have
   no counterpart here yet.
4. **Java/Kotlin leftovers** — jdtls, Lombok, the Kotlin bridge and JDWP are
   gone; the Maven/Gradle build-tool model and the Java _run_ configurations are
   kept (they launch `java` without a language server or debugger).

## 7. Verifying a change

```bash
cargo check --workspace --all-targets
cargo test -p ide-cmake -p ide-run -p ide-build -p ide-workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm typecheck && pnpm test && pnpm lint
pnpm dev            # browser preview with a sample C++/CMake project
pnpm e2e            # Playwright against the same preview
```

The desktop shell (`pnpm dev:app`) additionally needs the Tauri system
dependencies (webkit2gtk on Linux); everything above runs without them.
