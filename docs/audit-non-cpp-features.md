# 非 C/C++/CMake 功能审计（只读盘点）

> 审计对象：`/home/limstorm/Projects/coder/openclion`（OpenIDEA 的 CLion 化 fork）。
> 目标：把「一个 C/C++ IDE 永远用不到」的语言 / 工具 / 产品功能逐项列清，给出**精确路径与删除缝（removal seam）**，同时保留编辑器语法高亮。
> 方法：只读代码检索（`languages/*.toml`、`crates/*/src`、`src/features/*`、`src-tauri/src/commands/mod.rs`、`src/bindings.ts`），未运行构建、未修改任何代码。

> **审计基线**：提交 `ae257a2`（"refactor: remove the JVM subsystems …"）的已检出状态，审计时间 2026-11-02 15:26–15:50。
> 审计期间工作区有**并发进行中的改动**（非本报告作者所为）：截至 15:52，`languages/*.toml` 中 52 个非 C++ 语言的 `[[language-servers]]` / `[debugger]` 段**已被删除**，`crates/ide-language/src/registry.rs` 的测试改为断言「只有 C 家族保留 server」，`crates/ide-tools` 的 `registry_names_match_language_configs` 从「双向一致」放宽为「TOML → registry 单向」。因此下文表格描述的是**基线状态**；§8 的 Stage 1 前半段（TOML 剥离）已在飞行中，但 **`TOOLS` 的 44 个非 C++ `ToolSpec`、`ide-run::detect`、`ide-dap` builtin、后端 crate、前端 feature 均尚未删除**。

---

## 0. 摘要

### 0.1 关键数字

| 指标                                    | 数量                                                | 证据                                                               |
| --------------------------------------- | --------------------------------------------------- | ------------------------------------------------------------------ |
| `languages/*.toml` 语言定义             | 73                                                  | `ls languages/*.toml`                                              |
| 仍声明 `[[language-servers]]` 的语言    | 57（共 71 条 server 定义，11 条 `fallback = true`） | `grep -l '^\[\[language-servers\]\]' languages/*.toml`             |
| 仍声明 `[debugger]` 的语言              | 21                                                  | `grep -l '^\[debugger\]' languages/*.toml`                         |
| 使用 `tree-sitter` 的语言               | 41（共享 `grammars.rs` 的 35 个 grammar）           | `languages/*.toml`、`crates/ide-language/src/syntax/grammars.rs`   |
| 使用 `ui-mode`（CodeMirror 模式）的语言 | 58                                                  | 同上                                                               |
| `ide-tools` 托管安装器 `ToolSpec`       | 47（37 LS / 9 debugger / 1 formatter）              | `crates/ide-tools/src/registry.rs:117` `TOOLS`                     |
| `SdkKind` 变体                          | 8，**全部**是其他语言                               | `crates/ide-sdk/src/lib.rs:32-44`                                  |
| `src/features/index.ts` 注册的 feature  | 43                                                  | `src/features/index.ts:49-92`                                      |
| 与 C/C++ 无关、可整体下线的 feature     | 12                                                  | 见 §5                                                              |
| Tauri 命令总数 / 其中非 C++ 域          | **451** / 约 **137**                                | `src-tauri/src/commands/mod.rs:68-521`（`collect_commands!` 计数） |

### 0.2 五条结论

1. **数据面是最便宜的删除面。** `languages/*.toml`、`ide-tools::registry`、`ide-run::detect`、`ide-dap` 的 builtin adapter 表都是纯数据；删数据不改结构。`crates/ide-tools/src/registry.rs:864` 的测试 `registry_names_match_language_configs` 在基线时强制 `TOOLS` 与 `languages/*.toml` **双向一致**——两侧必须同一次提交里删，否则 `cargo test -p ide-tools` 红。（并发改动已把它放宽为「TOML → registry」单向，于是 registry 侧可以晚一步删；代价是 44 个非 C++ 安装器条目会以「惰性元数据」形式留在设置页的探测列表里。）
2. **最容易被忽略的成本是三处"自动发生"的**：项目打开时的 `sdk_project_libraries`（6 线程 + `python`/`rustc`/`cargo`/`go` 子进程，`crates/ide-sdk/src/libraries.rs:71`）、`agent-registry` 维护线程（可能 HTTPS 拉 ACP registry，`crates/ide-agent/src/registry.rs:637`）、以及**每次启动**就构建全部 35 个 tree-sitter grammar（41 个语言定义共享这 35 个 grammar；`src-tauri/src/state.rs:61`）。其余（LSP、Jupyter kernel、`docker events`、DB pool、agent 子进程）都是**按需**启动。
3. **有 3 条硬依赖必须"搬走"而不是"删掉"**：
   - `ide_http::client::{with_proxy,build,shared}` —— Tauri updater 用它注入代理（`src-tauri/src/commands/window.rs:326`）；
   - `ide_build::compile` 的 `BuildSystem::CMake` 分支 —— Build Project 的 C++ 路径（`crates/ide-build/src/compile/mod.rs:208`）；
   - `ide_run::process::{ProcessManager, StartOptions}` —— 所有运行/终端/构建进程（`crates/ide-run/src/lib.rs:17`）。
4. **Java/Kotlin 只删了"语言"，没删"JVM 平台"。** 上一次提交 `ae257a2` 移除了 jdtls / JDWP / Lombok / Kotlin bridge，但 Maven/Gradle 模型、Tomcat、JUnit、JDK 下载、`jvm_file_presentations`、`kotlin.toml` 的 LSP 仍在（详见 §3.4、§4.6）。本次清理应把它们一并纳入。
5. **删除顺序必须"前端先下线、后端后删命令"**，否则 `src/bindings.ts`（由 `cargo test -p openclion --test export_bindings` 生成）会让 `pnpm typecheck` 直接红。§8 的 5 个阶段按这个约束排。

---

## 1. `languages/*.toml`：谁还在声明 language server / debugger

### 1.1 判定规则

一个 `languages/<id>.toml` 去掉 `[[language-servers]]`、`[debugger]`、`formatter` 之后还剩什么：

- `tree-sitter = "<grammar>"` → 仍有**结构化高亮 + outline + folding + 选择范围**（`crates/ide-language/src/syntax/grammars.rs` 的 `SPECS`）；
- `ui-mode = "<CM name>"` → 仍有 **CodeMirror 词法高亮**（`src/editor/core/cm/languages.ts` 的 `findLanguageDescription`）；
- 两者都没有 → 只剩 **扩展名关联 + 行注释 / 块注释 token + 缩进设置**（仍有用：注释切换、自动缩进、File Types 归组、状态栏语言名）。

`ui-mode` 的一个坑：`languages/http.toml:7` 的 `ui-mode = "HTTP"` 由 **http-client feature** 通过 `registerLanguageMode` 提供（`src/features/http-client/index.ts:118`）；删掉该 feature 后回退到 `@codemirror/legacy-modes` 的 `HTTP`（toml 注释里写明了这个 fallback）。

### 1.2 保留（C/C++/CMake 核心，★）

| 语言          | 文件                           | LS                                                | debugger   | 语法                | 说明                                        |
| ------------- | ------------------------------ | ------------------------------------------------- | ---------- | ------------------- | ------------------------------------------- |
| C             | `languages/c.toml`             | `clangd`                                          | `lldb-dap` | tree-sitter `c`     | ★ 主语言                                    |
| C++           | `languages/cpp.toml`           | `clangd`                                          | `lldb-dap` | tree-sitter `cpp`   | ★ 主语言                                    |
| CMake         | `languages/cmake.toml`         | `neocmakelsp` + `cmake-language-server`(fallback) | —          | tree-sitter `cmake` | ★ 构建系统；两个 server 是主/备关系，都要留 |
| Objective-C   | `languages/objective-c.toml`   | `clangd --background-index`                       | `lldb-dap` | ui-mode only        | ★ 同一 clangd 家族                          |
| Objective-C++ | `languages/objective-cpp.toml` | `clangd --background-index`                       | `lldb-dap` | ui-mode only        | ★ 同一 clangd 家族                          |
| Makefile      | `languages/makefile.toml`      | —                                                 | —          | tree-sitter `make`  | ★ CMake 的 Makefile generator / 老项目      |

### 1.3 其余 67 个语言：哪个该删 server、哪个可整体删

「删后」= 移除 `[[language-servers]]` + `[debugger]` + `formatter` 后仍具备的能力；「建议」= 对 C/C++ IDE 的处理建议（`语法` = 留 TOML 只保高亮，`整删` = 连 TOML 一起删）。

| id                             | LS（name）                                                   | debugger           | 语法来源                       | 删后     | 建议                                                            |
| ------------------------------ | ------------------------------------------------------------ | ------------------ | ------------------------------ | -------- | --------------------------------------------------------------- |
| `bat`                          | —                                                            | —                  | 无（前端 `batchLanguage`）     | 仅扩展名 | 整删                                                            |
| `clojure`                      | clojure-lsp                                                  | —                  | ui-mode                        | 语法     | 整删（无 grammar，只有 legacy mode）                            |
| `csharp`                       | csharp-ls                                                    | netcoredbg         | tree-sitter                    | 语法     | 语法                                                            |
| `css`                          | vscode-css-language-server                                   | —                  | tree-sitter                    | 语法     | 语法                                                            |
| `dart`                         | dart                                                         | dart               | tree-sitter                    | 语法     | 语法                                                            |
| `diff`                         | —                                                            | —                  | ui-mode                        | 语法     | **留**（patch / `git diff` 查看）                               |
| `dockerfile`                   | dockerfile-language-server                                   | —                  | ui-mode                        | 语法     | 整删（除非要编辑 Dockerfile）                                   |
| `dotenv`                       | —                                                            | —                  | ui-mode                        | 语法     | 留（`.env` 常见于调试环境）                                     |
| `elixir`                       | elixir-ls                                                    | elixir-ls-debugger | tree-sitter                    | 语法     | 语法                                                            |
| `erlang`                       | elp                                                          | —                  | ui-mode                        | 语法     | 语法                                                            |
| `fsharp`                       | fsautocomplete                                               | netcoredbg         | 无                             | 仅扩展名 | 整删                                                            |
| `gitattributes`                | —                                                            | —                  | 无                             | 仅扩展名 | 留（git 相关文本）                                              |
| `gdscript`                     | godot（TCP 连接，会**启动 headless Godot 进程**）            | godot（DAP TCP）   | tree-sitter                    | 语法     | 整删                                                            |
| `gdshader`                     | —                                                            | —                  | ui-mode `C`                    | 语法     | 整删                                                            |
| `go`                           | gopls                                                        | delve              | tree-sitter                    | 语法     | 语法                                                            |
| `go.mod`                       | gopls                                                        | —                  | 无                             | 仅扩展名 | 整删                                                            |
| `go.work`                      | gopls                                                        | —                  | 无                             | 仅扩展名 | 整删                                                            |
| `godot-resource`               | —                                                            | —                  | ui-mode                        | 语法     | 整删                                                            |
| `graphql`                      | graphql-lsp                                                  | —                  | tree-sitter                    | 语法     | 语法                                                            |
| `groovy`                       | groovy-language-server                                       | —                  | ui-mode                        | 语法     | 整删（Gradle 脚本；若保留 Gradle 则留语法）                     |
| `haskell`                      | haskell-language-server                                      | —                  | tree-sitter                    | 语法     | 语法                                                            |
| `hcl`                          | —                                                            | —                  | tree-sitter                    | 语法     | 语法                                                            |
| `html`                         | angular-language-server + vscode-html-language-server        | —                  | tree-sitter                    | 语法     | 语法（`[[framework-ui-modes]]` 也要删，见 §1.4）                |
| `http`                         | —                                                            | —                  | ui-mode（由 http-client 提供） | 仅扩展名 | 整删（连 `.http` 文件类型）                                     |
| `ignore`                       | —                                                            | —                  | 无                             | 仅扩展名 | 留（`.gitignore`）                                              |
| `ini`                          | —                                                            | —                  | ui-mode                        | 语法     | 留（`.editorconfig` 等）                                        |
| `java`                         | —（jdtls 已随 JVM 清理删除）                                 | —                  | tree-sitter                    | 语法     | 语法                                                            |
| `javascript`                   | vtsls + typescript-language-server                           | js-debug           | tree-sitter                    | 语法     | 语法                                                            |
| `javascriptreact`              | 同上                                                         | js-debug           | tree-sitter `javascript`       | 语法     | 语法                                                            |
| `json`                         | vscode-json-language-server                                  | —                  | tree-sitter                    | 语法     | **留**（`compile_commands.json`、`CMakePresets`）               |
| `jsonc`                        | vscode-json-language-server                                  | —                  | tree-sitter `json`             | 语法     | **留**（`launch.json`/`settings.json`/`compile_commands` 关联） |
| `julia`                        | julia `-e using LanguageServer`                              | —                  | 无                             | 仅扩展名 | 整删                                                            |
| `kotlin`                       | kotlin-lsp + kotlin-language-server                          | —                  | tree-sitter                    | 语法     | 语法（或整删，见 §3.4）                                         |
| `latex`                        | texlab                                                       | —                  | 无                             | 仅扩展名 | 整删                                                            |
| `less`                         | vscode-css-language-server                                   | —                  | 无                             | 仅扩展名 | 整删                                                            |
| `log`                          | —                                                            | —                  | 无                             | 仅扩展名 | 留（构建日志）                                                  |
| `lua`                          | lua-language-server                                          | —                  | tree-sitter                    | 语法     | 语法                                                            |
| `markdown`                     | marksman                                                     | —                  | tree-sitter                    | 语法     | **留**（README/文档；`marked`/`mermaid` 预览也依赖）            |
| `nix`                          | nil + nixd                                                   | —                  | tree-sitter                    | 语法     | 语法                                                            |
| `ocaml`                        | ocamllsp                                                     | ocamlearlybird     | tree-sitter                    | 语法     | 语法                                                            |
| `ocaml.interface`              | ocamllsp                                                     | —                  | tree-sitter `ocaml_interface`  | 语法     | 语法                                                            |
| `perl`                         | perlnavigator                                                | —                  | ui-mode                        | 语法     | 整删                                                            |
| `php`                          | intelephense + phpactor                                      | php-debug          | tree-sitter                    | 语法     | 语法                                                            |
| `plaintext`                    | —                                                            | —                  | 无                             | 仅扩展名 | **留**                                                          |
| `powershell`                   | powershell-editor-services（`pwsh` 长命令行）                | —                  | ui-mode                        | 语法     | 整删                                                            |
| `properties`                   | —                                                            | —                  | ui-mode                        | 语法     | 留                                                              |
| `proto`                        | buf                                                          | —                  | tree-sitter                    | 语法     | 语法                                                            |
| `python`                       | basedpyright + pyright + ty                                  | debugpy            | tree-sitter                    | 语法     | 语法                                                            |
| `r`                            | r-languageserver                                             | —                  | ui-mode                        | 语法     | 整删                                                            |
| `ruby`                         | ruby-lsp + solargraph                                        | —                  | tree-sitter                    | 语法     | 语法                                                            |
| `rust`                         | rust-analyzer                                                | codelldb           | tree-sitter                    | 语法     | 语法（IDE 自身是 Rust，建议留）                                 |
| `sass`                         | —                                                            | —                  | ui-mode                        | 语法     | 整删                                                            |
| `scala`                        | metals                                                       | —                  | tree-sitter                    | 语法     | 语法                                                            |
| `scss`                         | vscode-css-language-server                                   | —                  | 无                             | 仅扩展名 | 整删                                                            |
| `shellscript`                  | bash-language-server                                         | bash-debug         | tree-sitter `bash`             | 语法     | **留**（构建脚本 / `run.sh`）                                   |
| `sql`                          | sqls                                                         | —                  | ui-mode                        | 语法     | 整删（连带 database feature）                                   |
| `svelte`                       | svelte-language-server                                       | —                  | ui-mode `HTML`                 | 语法     | 整删                                                            |
| `swift`                        | sourcekit-lsp                                                | lldb-dap           | tree-sitter                    | 语法     | 语法（lldb 家族）                                               |
| `terraform` / `terraform-vars` | terraform-ls                                                 | —                  | tree-sitter `hcl`              | 语法     | 整删                                                            |
| `toml`                         | taplo                                                        | —                  | tree-sitter                    | 语法     | 留（`Cargo.toml`/`vcpkg` 周边）                                 |
| `typescript`                   | vtsls + typescript-language-server + angular-language-server | js-debug           | tree-sitter                    | 语法     | 语法                                                            |
| `typescriptreact`              | vtsls + typescript-language-server                           | js-debug           | tree-sitter `tsx`              | 语法     | 语法                                                            |
| `vue`                          | vue-language-server + vtsls                                  | —                  | ui-mode                        | 语法     | 整删                                                            |
| `xml`                          | lemminx                                                      | —                  | tree-sitter                    | 语法     | 语法                                                            |
| `yaml`                         | yaml-language-server                                         | —                  | tree-sitter                    | 语法     | **留**（`.clang-format` / `.clang-tidy` / `.clangd`）           |
| `zig`                          | zls                                                          | codelldb           | tree-sitter                    | 语法     | 语法                                                            |

> `yaml.toml:5` 的 `filenames` 已包含 `.clang-format` / `.clang-tidy` / `.clangd`——这条 C++ 关联必须保留。

### 1.4 隐藏的"语言外挂"字段

| 字段                                     | 位置                                                                                                                           | 说明                                                                                      | 处理                                                                                                                               |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `[[framework-ui-modes]]`                 | `languages/html.toml:13-15`（`ui-mode = "Angular Template"`，`root-markers = ["angular.json", "node_modules/@angular/core"]`） | 项目里出现 Angular 标记时把 HTML 模板切到 Angular 模式                                    | 删；同时删 `src/editor/core/cm/angular/`（68 K + 生成的 `parser.ts`）与 `src/editor/core/cm/languages.ts:93` 的 `Angular Template` |
| `tsserver-project = true`                | `javascript*.toml`、`typescript*.toml`（9 处）                                                                                 | 让 `ide-lsp` 给 tsserver 造"外部项目"（`crates/ide-lsp/src/external_project.rs`）         | 删 server 后这些字段随之消失；`external_project.rs` / `tsserver_bridge.rs` 变成死代码                                              |
| `max-java = 21`                          | `languages/kotlin.toml:23`                                                                                                     | 限制 kotlin-language-server 的 JDK                                                        | 随 server 删除                                                                                                                     |
| `connect = "127.0.0.1:6005"` / `6006`    | `languages/gdscript.toml:15,25`                                                                                                | **TCP 连接型** LSP/DAP：没有已开编辑器时会 `godot --headless --editor` 起进程             | 随语言删除                                                                                                                         |
| `initialization-options`（basedpyright） | `languages/python.toml:21-24`                                                                                                  | 嵌套 TOML 子表，删 server 时注意是 `[language-servers.initialization-options…]`，要整段删 | 删                                                                                                                                 |

---

## 2. 托管工具安装器：`ide-tools` 与 `ide-sdk`

### 2.1 位置与结构

| 符号                            | 路径                                                                                                         | 作用                                                                                                        |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| `TOOLS: &[ToolSpec]`            | `crates/ide-tools/src/registry.rs:117`                                                                       | 47 个可安装工具                                                                                             |
| `enum ToolKind`                 | `crates/ide-tools/src/registry.rs:10`                                                                        | `LanguageServer` / `Debugger` / `Formatter`                                                                 |
| `enum Method`                   | `crates/ide-tools/src/registry.rs:28`                                                                        | `Rustup` / `Npm` / `Go` / `PipVenv` / `PipUser` / `PipTarget` / `Gem` / `DotnetTool` / `Release` / `Manual` |
| `enum ReleaseSource`            | `crates/ide-tools/src/registry.rs:18`                                                                        | `GitHub` / `HashiCorp` / `OpenVsx`                                                                          |
| `pub fn find(name)`             | `crates/ide-tools/src/registry.rs:846`                                                                       | 按 `languages/*.toml` 里的 `name` 查表                                                                      |
| `Tools::install/uninstall/list` | `crates/ide-tools/src/lib.rs:740 / 1153 / 703`                                                               | 安装、卸载、状态探测                                                                                        |
| `ide_tools::jdk`                | `crates/ide-tools/src/jdk.rs`（+ `src-tauri/src/commands/tools.rs:95-140`）                                  | **Download JDK**：vendor 列表、版本列表、下载（Java only）                                                  |
| 安装根目录                      | `ide_tools::tools_dir` / `tools_bin_dirs` / `extend_process_path`，`crates/ide-tools/src/lib.rs:114/125/159` | `<app-data>/tools` 加进子进程 `PATH`                                                                        |

### 2.2 C/C++/CMake 真正用到的只有 3 个

| ToolSpec      | kind           | Method                                      | 被谁引用                                                                      |
| ------------- | -------------- | ------------------------------------------- | ----------------------------------------------------------------------------- |
| `clangd`      | LanguageServer | `Release(GitHub clangd/clangd)`             | `languages/c.toml:11`、`cpp.toml:11`、`objective-c*.toml:10`                  |
| `lldb-dap`    | Debugger       | `Manual`（无下载，仅提示）                  | `languages/c.toml:19`、`cpp.toml:19`、`objective-c*.toml:17`、`swift.toml:17` |
| `neocmakelsp` | LanguageServer | `Release(GitHub Decodetalkers/neocmakelsp)` | `languages/cmake.toml:12`                                                     |

> `codelldb`（`registry.rs:499`）虽然是通用 LLDB 前端，但只被 `rust.toml` / `zig.toml` 引用；若 Rust/Zig 语法保留但不需要调试，可一并删除。

### 2.3 其余 44 个可删项（按 Method 分组，便于批量删）

| Method                                   | 工具                                                                                                                                                                                                                                                                | 依赖的运行时                                                |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| `Npm`（15）                              | prettier, vtsls, typescript-language-server, pyright, bash-language-server, yaml-language-server, vscode-json/css/html-language-server, dockerfile-language-server, svelte-language-server, vue-language-server, angular-language-server, graphql-lsp, intelephense | `node`（`registry.rs:105`）                                 |
| `Go`（3）                                | gopls, sqls, delve                                                                                                                                                                                                                                                  | `go`                                                        |
| `PipVenv`/`PipTarget`（3）               | basedpyright, ty（PipVenv）, debugpy（PipTarget）                                                                                                                                                                                                                   | `python3` / `uv`                                            |
| `Gem`（2）                               | ruby-lsp, solargraph                                                                                                                                                                                                                                                | `gem`                                                       |
| `DotnetTool`（2）                        | csharp-ls, fsautocomplete                                                                                                                                                                                                                                           | `dotnet`                                                    |
| `Rustup`（1）                            | rust-analyzer                                                                                                                                                                                                                                                       | `rustup`                                                    |
| `Release` GitHub/HashiCorp/OpenVsx（18） | lua-language-server, marksman, taplo, zls, terraform-ls, codelldb, js-debug, kotlin-lsp, elixir-ls, elixir-ls-debugger, buf, clojure-lsp, texlab, lemminx, perlnavigator, netcoredbg, bash-debug, php-debug                                                         | 无 / `node`（js-debug、bash-debug、php-debug 走 node 包装） |

15+3+3+2+2+1+18 = **44**，与「47 − 3 个保留」一致。`Manual` 只有 `lldb-dap` 一个（无安装器、仅提示），属于保留项，不在上表内。删除后 `Method` 的 `Rustup` / `Npm` / `Go` / `PipVenv` / `PipUser` / `PipTarget` / `Gem` / `DotnetTool` 八个变体全部无引用，可以只留 `Release` + `Manual`；`ReleaseSource::HashiCorp` 与 `OpenVsx` 也随之无引用。

### 2.4 `SdkKind`：8 个变体全部是其他语言

| 变体             | 定义                           | 检测函数                                | 谁在用                                                                                            |
| ---------------- | ------------------------------ | --------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `Python`         | `crates/ide-sdk/src/lib.rs:33` | `discover.rs:16` → `python(root)`       | `src-tauri/src/commands/sdk.rs`、`ide-host/src/ops/notebook.rs:105`、`ide-host/src/ops/python.rs` |
| `Node`           | `:34`                          | `discover.rs:17`                        | Python/JS 工具链、`run/runtimes.ts:34`                                                            |
| `PackageManager` | `:36`                          | `discover.rs:18` → `package_managers()` | npm/pnpm/yarn/bun                                                                                 |
| `Rust`           | `:37`                          | `discover.rs:19`                        | rust-analyzer、cargo 运行配置                                                                     |
| `Go`             | `:38`                          | `discover.rs:20`                        | gopls、Go 模块                                                                                    |
| `TypeScript`     | `:39`                          | `discover.rs:21`                        | tsdk                                                                                              |
| `Jdk`            | `:41`                          | `discover.rs:22` → `jdk()`              | Maven/Gradle/Tomcat                                                                               |
| `Maven`          | `:43`                          | `discover.rs:23`                        | Maven 同步                                                                                        |

**C/C++ 不经过 `SdkKind`**：编译器套件走 `ide_sdk::cpp::CompilerKit`（`crates/ide-sdk/src/cpp.rs:148`，命令 `sdk::toolchain_kits`，`src-tauri/src/commands/sdk.rs` → `ide-host/src/ops/sdk.rs:32`）。因此 `SdkKind` 整个枚举可以删（连带 `discover.rs` 的 8 个 detect 函数、`version_managers.rs` 测试、`Sdk` 结构体的 `source` 值 `venv/pyenv/conda/nvm/rustup/…`）。

### 2.5 `ide-sdk` 里只服务其他语言的模块

| 文件                                                                  | 行数            | 内容                                                                                                                                                                                                                                                                            | 使用者                                                               |
| --------------------------------------------------------------------- | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| `crates/ide-sdk/src/python.rs`                                        | 145             | 创建 venv、pip 安装/卸载（`sdk_python_create_venv`、`sdk_python_packages`、`sdk_python_pip`）                                                                                                                                                                                   | Python Interpreter 设置页、notebook kernel 选择                      |
| `crates/ide-sdk/src/npm.rs`                                           | 227             | npm/pnpm/yarn 探测                                                                                                                                                                                                                                                              | Node 设置页                                                          |
| `crates/ide-sdk/src/go.rs`                                            | 329             | GOROOT/GOPATH/`go list`、`work_modules`                                                                                                                                                                                                                                         | Go 设置页、`libraries` 的 `goModules`                                |
| `crates/ide-sdk/src/java.rs`                                          | 342             | JDK 发现、`pom.xml` 的 `localRepository`、`JAVA_HOME`                                                                                                                                                                                                                           | Maven/Gradle、`jvm_icons`                                            |
| `crates/ide-sdk/src/libraries.rs`                                     | 538             | External Libraries：python/jdk/node/rust/cargo/go/maven/gradle 六类根目录                                                                                                                                                                                                       | `sdk_project_libraries`                                              |
| `crates/ide-sdk/src/angular.rs`                                       | 93              | Angular LSP 包定位、TS 版本参数                                                                                                                                                                                                                                                 | `apply.rs:363-490`                                                   |
| `crates/ide-sdk/src/vue.rs`                                           | 94              | Vue LSP + `@vue/typescript-plugin`                                                                                                                                                                                                                                              | `apply.rs:340-457`                                                   |
| `crates/ide-sdk/src/godot.rs`                                         | 109             | Godot 可执行文件定位                                                                                                                                                                                                                                                            | `ops/tools.rs:26`、`apply.rs:233`                                    |
| `crates/ide-sdk/src/apply.rs`                                         | 675             | 把 settings 套到 server/run config；**其中** `python_interpreter`(33)、`project_jdk`(79)、`maven_command`(89)、`gradle_command`(107)、`gradle_env`(129)、`maven_options`(145)、`maven_local_repository`(159)、`tsdk`(183)、godot/angular/vue 分支(233/340/363/470) 都是其他语言 | `ide-host/src/project.rs:127-146`（每个 project 的 LSP config hook） |
| `crates/ide-sdk/src/pinned.rs`、`exec.rs`、`locate.rs`、`discover.rs` | 233/210/198/556 | 版本管理器（mise/asdf）、子进程执行、`which`、SDK 探测                                                                                                                                                                                                                          | 混用；`locate.rs` 服务于「运行当前文件」的通用路径，需保留           |

`ide-sdk` 的 C++ 部分是 `cpp.rs`（148 行，compiler kits）+ `apply.rs::setting_path` + `locate.rs`。**建议保留 `ide-sdk` crate，只删模块**（比整体删除安全，因为 `ide-host` 与 `src-tauri` 到处 `use ide_sdk::apply`）。

---

## 3. 运行 / 调试集成（按语言）

### 3.1 `crates/ide-run/src/detect/*`：项目打开时的运行配置探测

入口 `crates/ide-run/src/detect.rs:31` `detect_with_cmake()`，依次调用 11 个探测器（**纯文件解析，无子进程**）：

| 探测器                                  | 行                           | 识别                           | 产出的 `kind`     | 归属                   |
| --------------------------------------- | ---------------------------- | ------------------------------ | ----------------- | ---------------------- |
| `detect_node`                           | `detect.rs:229`              | `package.json` scripts         | `npm`             | JS/TS                  |
| `detect_deno`                           | `detect.rs:256`              | `deno.json` tasks              | `deno`            | JS/TS                  |
| `detect_cargo`                          | `detect.rs:325`              | `Cargo.toml` bins/tests        | `cargo`           | Rust                   |
| `detect_gradle` / `detect_gradle_build` | `detect.rs:475/482`          | `build.gradle*` tasks          | `gradle`          | JVM                    |
| `detect_maven` / `detect_maven_build`   | `detect.rs:517/527`          | `pom.xml` 插件/goals           | `maven`           | JVM                    |
| `detect_go` / `detect_go_module`        | `detect.rs:558/586`          | `go.mod` main/test             | `go`              | Go                     |
| `detect_python`                         | `detect.rs:715`              | `pyproject.toml`/`*.py`/pytest | `python`/`pytest` | Python                 |
| `cmake::detect`                         | `detect/cmake.rs`（377 行）  | `CMakeLists.txt`、CTest        | `cmake`           | ★ **保留**             |
| `dotnet::detect`                        | `detect/dotnet.rs`（121 行） | `*.csproj`                     | `dotnet`          | .NET                   |
| `detect_make`                           | `detect.rs:798`              | `Makefile` targets             | `make`            | **通用**（C/C++ 也用） |
| `detect_just`                           | `detect.rs:869`              | `justfile`                     | `just`            | 通用                   |
| `detect_godot`                          | `detect.rs:855`              | `project.godot`                | `godot`           | Godot                  |

删除缝：`detect_with_cmake()` 里逐行删调用；`detect/` 目录删 `dotnet.rs`。`detect_make` / `detect_just` 语言无关，建议保留（很多 C++ 项目用 Makefile / just）。

### 3.2 `crates/ide-run` 里按语言分家的模块

| 文件                  | 行数 | 内容                                                                            | 谁用                                                                                              | 处理                                                                                                                               |
| --------------------- | ---- | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `src/tomcat.rs`       | 531  | Tomcat home 校验、artifact 构建、update、`base_dir`（Spring Boot / WAR 部署）   | `ide-host/src/ops/run.rs:125`、`ops/run/commands.rs:158/197/236`、`src-tauri/src/commands/run.rs` | 删（连带 5 个 `run_tomcat_*` 命令）                                                                                                |
| `src/junit.rs`        | 364  | 读 JUnit XML（surefire / Gradle test report）                                   | `ops/run/commands.rs:132-153`、`src-tauri/src/commands/run.rs:151`                                | 删（连带 `run_test_reports` / `run_test_report_file`）                                                                             |
| `src/java_cmdline.rs` | 537  | 长 java 命令行缩短、classpath 处理                                              | **仅被注释引用**（`crates/ide-vcs/src/cli.rs:132` 提到"同样的限制"）                              | 删（死代码）                                                                                                                       |
| `src/gradle_jvm.rs`   | 97   | Gradle init script 注入 `jvm_args`                                              | `ops/run/commands.rs:117`、`run_gradle_jvm_args_script` 命令                                      | 删                                                                                                                                 |
| `src/dotnet.rs`       | 257  | .NET 解析 / target path                                                         | `ops/run/commands.rs:16-19`、`run_dotnet_resolve`                                                 | 删                                                                                                                                 |
| `src/go.rs`           | 292  | Go 包源码、`work_modules`、测试文件准备                                         | `ide-sdk/src/go.rs:174`、`ide-sdk/src/libraries.rs:422`、`ops/run.rs:135`                         | 删（连带 `libraries` 的 go 分支）                                                                                                  |
| `src/python_path.rs`  | 44   | PYTHONPATH 注入                                                                 | `ops/python.rs:40-44`                                                                             | 删                                                                                                                                 |
| `src/wrap.rs`         | 310  | 运行包装（run dir、`program_name`、`python_command`、`go_module`、`find_tool`） | `ide-coverage`、`ide-profiler`、`ops/coverage.rs`                                                 | **部分保留**：`new_output_dir` 被 coverage/profiler 用；删 coverage/profiler 后整个 `wrap` 只剩 `run_dir`/`program_name`，可一并删 |
| `src/process.rs`      | 1161 | `ProcessManager`                                                                | 全局                                                                                              | ★ **保留**                                                                                                                         |

### 3.3 `crates/ide-dap` 的 adapter

| 位置                                              | 内容                                                                                                           | 归属                                                             |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| `crates/ide-dap/src/adapter.rs:50-92` `builtin()` | 6 个内建 adapter：`codelldb`(60)、`debugpy`(66)、`js-debug`(67)、`delve`(74)、`netcoredbg`(80)、`lldb-dap`(86) | ★ 只留 `lldb-dap`（+ 可选 `codelldb`）                           |
| `adapter.rs:101-107` `default_type()`             | `codelldb→lldb`、`debugpy→debugpy`、`js-debug→pwa-node`、`delve→go`、`netcoredbg→coreclr`                      | 随 adapter 删                                                    |
| `crates/ide-dap/src/cargo.rs`                     | 113                                                                                                            | 「cargo 构建后再调试」：`extra_json` 的 `"cargo"` 分支           | 删（Rust only）                     |
| `crates/ide-dap/src/attach.rs`                    | 289                                                                                                            | attach 到已运行进程；`python: "Python (debugpy)"` 等标签         | 部分删（`debug_attach` 通用能力留） |
| `crates/ide-dap/src/session.rs`                   | 484                                                                                                            | 会话；含 `skipLibraryCode`（Python `justMyCode` / node_modules） | 保留，删语言分支                    |

> `godot` 的 debugger 不在 `builtin()` 里——它来自 `languages/gdscript.toml:20-26`（TCP + `--dap-port`），删语言即删。

### 3.4 前端 run / debug 的语言分支

| 文件                                                                                         | 行数                                   | 内容                                                                                                                                                            | 删除方式                                                                                |
| -------------------------------------------------------------------------------------------- | -------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| `src/features/run/types.ts:10-17`                                                            | —                                      | `jvmTypes`、`tomcat`、`goTypes`、`dotnetTypes` 的 import；`BUILTIN_TYPES`（:385-399）含 `npm`、`gradle`、`maven`、`godot`、`pytest`、`dotnet*`、`go*`、`tomcat` | 删 import + 数组项，只留 `shell`、`program`、`compound`                                 |
| `src/features/run/jvmTypes.ts`                                                               | —                                      | `gradleForkedJvm` / `mavenForkedJvm` / `isWindows`（**`isWindows` 被 `context.ts:8`、`producers.ts:19` 共用，需保留一个通用版本**）                             | 拆出 `isWindows` 到 `platform/`，删其余                                                 |
| `src/features/run/tomcat.ts`                                                                 | 511（含 `TomcatSettings.tsx`）         | Tomcat 配置类型、`tomcatBrowserUrl`、`buildTomcatArtifact`                                                                                                      | 删；`store.ts:42`、`applicationUpdate.ts:12`、`services.test.ts` 引用要清               |
| `src/features/run/junit` 相关                                                                | `src/features/run/testing/jvm.ts`(101) | JVM 测试输出解析（Gradle/Maven）                                                                                                                                | 删                                                                                      |
| `src/features/run/goTypes.ts`/`goActions.ts`/`goGenerate.ts`/`goModules.ts`/`goArguments.ts` | 4 个文件 + 3 个测试                    | Go 运行类型、`go mod tidy` 等动作、generate                                                                                                                     | 删；`index.ts:37-38`、`contextRun.ts:32`、`runGutter.ts:16`、`producers.ts:16` 引用要清 |
| `src/features/run/dotnetTypes.ts`                                                            | —                                      | .NET 项目/测试类型                                                                                                                                              | 删；`beforeLaunch.ts:5` 引用要清                                                        |
| `src/features/run/serverPort.ts`                                                             | —                                      | 从输出里嗅探启动端口（Spring/Tomcat 用）                                                                                                                        | 删；`store.ts:41` 引用                                                                  |
| `src/features/run/testing/{cargo,js,go,jvm,pytest,ctest}.ts`                                 | 82/184/187/101/140/51                  | 各测试框架输出解析                                                                                                                                              | 留 `ctest.ts`（★ CTest）；`cargo` 视 Rust 去留；删 `js/go/jvm/pytest`                   |
| `src/features/run/producers.ts:221-314` `toolsFor()` / `scriptLanguage()`                    | 984 行中约 150 行                      | 语言判定与所需工具：`python`/`go`/`js`/`ts`/`native`(clang/gcc/cc)/`shell`                                                                                      | 删 `js/ts/python/go` 分支，留 `native`（C/C++）与 `shell`                               |
| `src/features/debugger/launch.ts:305-312`                                                    | —                                      | 按 adapter 分派 `debugpyLaunch`/`jsDebugLaunch`/`delveLaunch`/`godotLaunch`/`netcoredbg`                                                                        | 只留 `codelldb`/`lldb-dap`                                                              |
| `src/features/debugger/attach.ts:14`                                                         | —                                      | attach 候选进程的标签表（python/js/go/dotnet）                                                                                                                  | 删非 C++ 项                                                                             |
| `src/features/debugger/missingAdapter.ts:9-13`                                               | —                                      | 缺 adapter 的安装提示（netcoredbg/debugpy/js-debug/delve/codelldb）                                                                                             | 只留 `lldb-dap`                                                                         |
| `src/features/run/applicationUpdate.ts`                                                      | —                                      | 「Update Application」：Tomcat / .NET publish / Spring Boot                                                                                                     | 删或只留通用部分                                                                        |
| `src/features/run/buildUpdate.ts`                                                            | —                                      | 构建触发的应用更新                                                                                                                                              | 通用，保留                                                                              |

---

## 4. 会起进程 / 占内存的后端服务

### 4.1 `ide-notebook`（Jupyter / ZeroMQ）

| 项       | 路径 / 符号                                                                                                                  | 运行时行为                                                                                                                                                                                                                                                                           |
| -------- | ---------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| crate    | `crates/ide-notebook`（2760 行）                                                                                             | 依赖 `jupyter-zmq-client 2.0.0`、`jupyter-protocol 2.0.3`、`zeromq 0.6`（`Cargo.toml` workspace deps）                                                                                                                                                                               |
| 文档模型 | `src/document.rs`、`NotebookDocument/NotebookSnapshot/NotebookEdit`                                                          | 纯内存                                                                                                                                                                                                                                                                               |
| 内核进程 | `src/kernel/process.rs:29-95` `Process::spawn`                                                                               | `Command::new(python)` 起 **一个 Python 子进程**：`python -m ipykernel_launcher -f <runtime>/kernel-*/connection.json`（临时目录、`kill_on_drop`、Unix 上独立 process group），占 5 个 ZMQ 端口（`peek_ports_with_listeners(ip, 5)`，`process.rs:37`：stdin/control/hb/shell/iopub） |
| 内核监管 | `src/kernel/transport.rs:155`、`:214` `Running::spawn` → `tokio::spawn(supervise(...))`                                      | **每个打开的 notebook 一个常驻 tokio 任务 + 5 个 socket**（shell/iopub/stdin/control/heartbeat），超时 30 s 启动、8 s 关闭                                                                                                                                                           |
| Host 侧  | `crates/ide-host/src/notebook.rs`（814 行）、`ops/notebook.rs`（157 行）                                                     | 每个 project 一个 `NotebookService`（`project.rs:285`），持有 `KernelManager`（`Mutex<HashMap<PathBuf, Arc<Slot>>>`）                                                                                                                                                                |
| 命令     | `src-tauri/src/commands/notebook.rs` 11 条（`notebook_open/get/edit/save/reload/undo/redo/close/execute/interrupt/restart`） | `mod.rs:119-129`                                                                                                                                                                                                                                                                     |
| 前端     | `src/features/notebook/`（2593 行，11 文件）+ `registerEditorKind("notebook")`                                               | 打开 `.ipynb` 才加载（`lazyComponent`）                                                                                                                                                                                                                                              |

**删除缝**：`src-tauri/src/commands/mod.rs:119-129` 删 11 条命令 → 删 `src-tauri/src/commands/notebook.rs` → 删 `crates/ide-host/src/notebook.rs` 与 `ops/notebook.rs` → `Project` 结构体删 `notebooks` 字段（`project.rs:52`、`:309`、`:422`）→ `Host::shutdown_notebooks_on_exit`（`src-tauri/src/lib.rs:213`）→ 删 crate `ide-notebook`（`src-tauri/Cargo.toml`、`crates/ide-host/Cargo.toml`）→ 删 workspace deps `jupyter-zmq-client`/`jupyter-protocol`/`zeromq`。
**风险**：中（`NotebookService` 在 `Project` 里被 `Arc` 持有，`open_light` 也构造，`project.rs:402`）。

### 4.2 `ide-db`（sqlx / tiberius 驱动）

| 项       | 路径 / 符号                                                                                                                      | 运行时行为                                                                                                                                    |
| -------- | -------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| crate    | `crates/ide-db`（4216 行）                                                                                                       | `sqlx 0.9`（sqlite/postgres/mysql + rustls）、`tiberius 0.13`（SQL Server）、`bb8 0.9`（池）                                                  |
| 管理器   | `src/lib.rs:320` `DbManager::new`                                                                                                | 构造时 **spawn 一个 `openclion-db-history` 线程**（`src/lib.rs:328`），每 project / 每 LightEdit 窗口一个（`project.rs:284`、`:335`、`:401`） |
| 连接池   | `src/conn.rs:529-696`；`const MAX: u32 = 4`（`:535`）                                                                            | 每个 data source 最多 4 条连接；`connect` 时先建 1 条验证（`:533`）                                                                           |
| 隧道     | `src/lib.rs:345` `tunnels`、`tunnel_open`                                                                                        | SSH 隧道（`ide-ssh`）                                                                                                                         |
| 命令     | `src-tauri/src/commands/db.rs` **28 条**（`db_sources` … `db_console_path`），`mod.rs:462-476` + `:521`                          | —                                                                                                                                             |
| 前端     | `src/features/database/`（12119 行，43 文件，9 个测试）+ `setInterval(...,5000)` 空闲断连（`store.ts:403-414`）                  | tool window 常驻 stripe，`registerEditorKind(TABLE_KIND)`                                                                                     |
| SQL 检查 | `src/platform/inspections.ts:258-266` `builtin:sql`（producer `sql-inspections`），实现 `src/features/database/sqlSupport.ts:26` | 每次打开 SQL 控制台跑 schema 校验                                                                                                             |

**删除缝**：前端 feature → 命令 28 条（`mod.rs:462-476`、`:521`）→ `crates/ide-host/src/project.rs` 的 `db` 字段（`:46`、`:306`、`:335`）→ 删 crate。
**风险**：中。`Project.db` 只被 `src-tauri/src/commands/db.rs` 使用（同批删除），`http_history` 只被 `commands/http.rs` 使用（同批删除），其余 crate 无引用——字段与构造点干净可删。

### 4.3 `ide-http`

| 项     | 路径                                                                                                                                   | 运行时行为                                                                                   |
| ------ | -------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| crate  | `crates/ide-http`（1301 行）：`parse`/`env`/`exec`/`history`/`client`                                                                  | `reqwest 0.13` + `rustls`                                                                    |
| 历史   | `src/history.rs:47` `History::new(dir)`                                                                                                | **纯文件**（`index.json` + `<id>.json`），无线程；每个 project 一个（`project.rs:346`）      |
| 客户端 | `src/client.rs:12/28/46` `with_proxy`/`build`/`shared`                                                                                 | ★ **被 Tauri updater 使用**（`src-tauri/src/commands/window.rs:326`），也供 `ide-tools` 下载 |
| 命令   | `src-tauri/src/commands/http.rs` 7 条（`http_requests/environments/execute/history/history_get/history_clear/proxy_check_connection`） | —                                                                                            |
| 前端   | `src/features/http-client/`（2985 行，13 文件）+ `registerLanguageMode("HTTP")` + Spring 生成（`spring/`）                             | —                                                                                            |

**删除缝**：把 `crates/ide-http/src/client.rs` 抽到 `ide-core`（或保留一个 60 行的 `ide-net` crate），删 `parse/env/exec/history`、7 条命令、`Project.http_history`、前端 feature。
**风险**：**高**——`client.rs` 是 updater 的代理链路，不能连带删。

### 4.4 `ide-docker` / `ide-github` / `ide-web`

| crate        | 行数 | 运行时行为                                                                                                                                                                                                                                                                                                                                            | 删除缝                                                                                                                 |
| ------------ | ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `ide-docker` | 1506 | 全部走 `docker` CLI（`which::which("docker")` + 常见路径探测，`src/lib.rs:31-67`）。`snapshot` 一次 `docker ps/images/volume/network --format '{{json .}}'`；`watch_events`（`:283`）起 `docker events` 长流。前端：**Services 树挂载期间一个 `docker events` 子进程 + 10 s 轮询**（`src/features/services/store.ts:227-273`，`ServicesTree.tsx:52`） | 删 crate + `src-tauri/src/commands/docker.rs`（14 条）+ `features/services`                                            |
| `ide-github` | 1243 | 全部走 `gh` CLI（`src/lib.rs:24-57`）；`availability` 一次 `gh auth status` + `gh repo view`；PR 列表/详情/评论/checkout 各一次                                                                                                                                                                                                                       | 删 crate + `commands/github.rs`（10 条）+ `features/github`（19583 行！）                                              |
| `ide-web`    | 526  | `WebServer::new()` 只在 `AppState` 里构造（`src-tauri/src/state.rs:22/103`，**不起进程**）；首次 `url_for` 才 `TcpListener::bind(("127.0.0.1", 63342))`，失败退随机端口，然后 `tokio::spawn` accept 循环（`src/server.rs:104-115`）。`open()` 调系统浏览器                                                                                            | 删 `server.rs` 与 `AppState.web`；**保留 `browsers.rs`**（`web_open` 被 `features/web` 用；若整个 feature 删则一起删） |

### 4.5 `ide-agent`（ACP / Claude / Codex / Gemini CLI）

| 项            | 路径                                                                                                                                                                                          | 运行时行为                                                                                    |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| crate         | `crates/ide-agent`（14627 行，最大）                                                                                                                                                          | `acp/`、`native/{claude,codex}`、`registry.rs`、`worktree.rs`                                 |
| 内建 provider | `src/provider.rs:40-56` `BUILTIN` 14 个：`claude`、`codex`、`claude-acp`(npx)、`codex-acp`(npx)、`gemini`、`copilot`、`cursor`、`opencode`、`qwen`、`kimi`、`goose`、`auggie`、`kiro`、`vibe` | **每个运行中的任务一个 CLI 子进程**                                                           |
| 连接器        | `src/lib.rs:238-249` `AgentManager::new` → `Peer::spawn_framed(...)` / `native::codex::connect` / `native::claude::connect`                                                                   | 进程 + ACP JSON-RPC（lines framing）                                                          |
| 注册表维护    | `src/registry.rs:637-659` `maintain()`：`Once` + 线程 `agent-registry`，`prune()` 后若缓存过期且装过 registry agent 就 `fetch()`（**HTTPS 拉 ACP Registry**）                                 | 每次进程生命周期一次；**在第一个 project 打开时触发**（`crates/ide-host/src/project.rs:289`） |
| worktree      | `src/worktree.rs`                                                                                                                                                                             | 每个任务一个 `git worktree`（`~/.openclion/worktrees/<project>`）                             |
| 命令          | `src-tauri/src/commands/agent.rs` **22 条**（`mod.rs:96-117`）                                                                                                                                | —                                                                                             |
| 前端          | `src/features/agents/`（6914 行，32 文件，10 测试）+ tool window + `registerEditorKind(SESSION_KIND/HOME_KIND)` + 设置页 `REGISTRY_PAGE`                                                      | —                                                                                             |

**删除缝**：前端 feature + `activateSettingsPages`（`src/features/index.ts:118-131`）→ 22 条命令 → `project_services` 里的 `agents`（`project.rs:348`）与 `ide_agent::registry::maintain`（`:289`）→ 删 crate + `commands/agent.rs`。
**风险**：低（除 `project.rs` 的 `Project.agents`)——但要同步删 `ide-host/Cargo.toml` 与 `src-tauri/Cargo.toml` 里的依赖。

### 4.6 `ide-build` 的 Maven / Gradle 模型与同步

| 项       | 路径                                                                                                                              | 运行时行为                                                                                              |
| -------- | --------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| 模型     | `src/maven.rs`（1149+ 行）、`src/gradle.rs`（~400 行）                                                                            | `load()` **纯文件解析**（递归读 `pom.xml`、父 POM 从 `~/.m2` 读；`build.gradle` 正则解析）              |
| 同步     | `src/sync.rs:204` `sync_maven`、`:267` `sync_gradle`                                                                              | **后台跑 `mvn`/`gradle`**（`--batch-mode` / init script），把 effective POM / `gradle tasks` 写进 cache |
| 缓存     | `sync.rs:122/145` `apply_maven_cache`/`apply_gradle_cache`                                                                        | 无进程                                                                                                  |
| 应用更新 | `src/application_update.rs`                                                                                                       | Tomcat / Spring Boot 更新                                                                               |
| POM 编辑 | `src/pom_edit.rs`、`src/xml.rs`                                                                                                   | 纯文本编辑                                                                                              |
| 构建     | `src/compile/mod.rs:206-213`                                                                                                      | `BuildSystem::Maven/Gradle/CMake/Cargo/Go/TypeScript`；★ CMake 在 `:208`、`cmake_steps` 在 `:307`       |
| 命令     | `build_model`、`build_sync`、`maven_dependency_declaration`、`maven_exclude_dependency`（`src-tauri/src/commands/build.rs`）      | —                                                                                                       |
| 前端     | `src/features/build-tools/`（3329 行，13 文件，5 测试）+ 两个 tool window（仅当 `build.maven` / `build.gradle` 上下文为真时注册） | `loadModel()` 在 activate 与 build 文件变更时调用                                                       |

**建议**：**不要整删 `ide-build`**。把 `maven.rs`/`gradle.rs`/`sync.rs`/`pom_edit.rs`/`application_update.rs` 拆出去（或 cfg 掉），保留 `compile/{mod,parse}.rs` 的 CMake 分支 + `xml.rs`。
**风险**：中（`compile/mod.rs` 的 `BuildSystem` 枚举被 CMake、Cargo 共用）。

### 4.7 `ide-profiler` / `ide-coverage`

| crate          | 行数 | 支持矩阵                                                                                                             | C++ 相关性                                                                                                                    | 处理                                                                               |
| -------------- | ---- | -------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| `ide-profiler` | 1091 | `node --cpu-prof`、`py-spy`、async-profiler(JVM)、**`perf record`（native / cargo）**（`src/plan.rs:40-45`、`:182`） | **部分相关**：`config.kind == "native"` → `perf` 采样（`plan.rs:44`、`native()` 在 `:182`），这是 CLion 的 "Profile" 能力之一 | 保留 `native`/`cargo` 分支，删 node/python/jvm 分支与 `formats`/`jvm` 模块         |
| `ide-coverage` | 1241 | `cargo llvm-cov`、JaCoCo(Maven/Gradle)、c8/node、coverage.py、`go test -cover`、**无 C/C++**（`src/plan.rs:63-79`）  | **不相关**：没有 gcov/lcov/ctest 分支                                                                                         | 可整体删（但保留 UI 的 "Run with Coverage" 需要重写成 ctest+gcov，或先删 feature） |

**注意**：`src-tauri/src/commands/coverage.rs`（3 条）、`profiler.rs`（2 条），以及前端 `src/features/coverage/`（1180 行）、`src/features/profiler/`（684 行）。

### 4.8 其他"非语言但非 C++"的后端

| 项                                   | 路径                                                                                                                                                        | 说明                                                                                                | 建议                                                                                                       |
| ------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| `crates/ide-remote` + `ide-ssh`      | `ide-remote/src/lib.rs` 15 行 + SFTP/远程模块、`ide-ssh` 的 SSH 会话与 askpass                                                                              | SFTP 部署、远程文件、remote run targets、askpass                                                    | 语言无关；`deployment`/`ssh`/`remote-dev` feature 的上游。若不做远程开发可整删，**但与本次"语言"清理正交** |
| `crates/ide-workspace::dependencies` | `crates/ide-workspace/src/dependencies/`                                                                                                                    | Cargo 版本检查（`crates.io` 稀疏索引 HTTPS）                                                        | Rust only；`src/features/dependencies/`（418 行）+ `commands/dependencies.rs`（1 条）一起删                |
| `crates/ide-analysis`                | 4 文件                                                                                                                                                      | 重复代码、结构搜索（tree-sitter，通用）                                                             | **保留**（对 C++ 同样有效）                                                                                |
| `ide-syntax::jvm`                    | `crates/ide-syntax/src/jvm.rs`(189)、`ide-host/src/ops/jvm_icons.rs`(102)、`commands/syntax.rs::jvm_file_presentations`、`src/platform/filePresentation.ts` | Java/Kotlin 文件图标/可运行标记：读每个可见 `.java`/`.kt` 文件前 64 KB（`PREFIX_BYTES`，缓存 4096） | **删**（JVM 语言已删，纯残留）                                                                             |
| `kotlin.toml` 的 LSP                 | `languages/kotlin.toml:10-25`                                                                                                                               | jdtls 走后仍保留 kotlin-lsp / kotlin-language-server                                                | 若无 Kotlin 需求，删 server 段                                                                             |

---

## 5. 前端 feature（`src/features/index.ts`）

### 5.1 注册表

`src/features/index.ts:49-92` 43 项；`activateSettingsWindowFeatures`（`src/features/index.ts:118-131`）另注册 10 个设置页 feature。
删除一个 feature = 删 `import`（`:5-43`）+ 删数组项（`:49-92`）+（若有）删 `activateSettingsPages` import 与 `:118-131` 的项。

### 5.2 与 C/C++ 无关的 12 个 feature

| #   | feature                           | 行数 / 文件           | 注册了什么                                                                                                                                                                                | 项目打开时启动什么                                                                                  | 删 `activate()` 会破编译吗                                                                             | 连带删除                                                                                                                                                    |
| --- | --------------------------------- | --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `database`                        | 12119 / 43（9 测试）  | tool window `Database`、service view `RESULTS_VIEW`、`registerEditorKind(TABLE_KIND)`、overlay、editor contribution `database.console`、6 个 action、`watchIdleConnections()`(5 s 定时器) | 无（首次展开才 `loadSources`）；定时器常驻                                                          | **会**：`src/features/app/__tests__/consoleCopy.test.ts:7-8` 直接 import `ResultsView` / `useDatabase` | 整个目录 + `src/platform/inspections.ts:258-266` 的 `builtin:sql` 项 + keymap 键 `db.showToolWindow`（`ideaParity.test.ts:94` 断言顺序）+ `menu.rs:699-701` |
| 2   | `http-client`                     | 2985 / 13（3 测试）   | `registerLanguageMode("HTTP")`、2 个 editor contribution、`registerMenuGroup("http.spring")`、service view、`registerMenuGroup("http.new")`、6 个 action                                  | 无                                                                                                  | 否                                                                                                     | 目录 + `languages/http.toml` + `menu.rs:696-697`                                                                                                            |
| 3   | `notebook`                        | 2593 / 11（4 测试）   | `registerEditorKind("notebook")`（restorable）、`registerFileEditorProvider`（`.ipynb`）                                                                                                  | 打开 `.ipynb` 才建会话；执行才起 kernel                                                             | 否                                                                                                     | 目录 + 11 条命令 + `ide-notebook`                                                                                                                           |
| 4   | `agents`                          | 6914 / 32（10 测试）  | tool window、2 个 editor kind、drop handler、status item、设置页 `REGISTRY_PAGE`、多个 action/menu                                                                                        | 无（`AgentManager` 构造时只读 `store_dir` 的 JSON）                                                 | 否                                                                                                     | 目录 + 22 条命令 + `ide-agent` + `project.rs:289/348`                                                                                                       |
| 5   | `deployment`                      | 1781 / 12（4 测试）   | tool window `Remote Host`、overlay、menu group `deployment`、自动上传                                                                                                                     | 无                                                                                                  | 否（设置页 `settings/pages/deployment.tsx` **是另一个文件**，不受影响）                                | 目录 + `menu.rs` 的 Deployment 子菜单 + `commands/deploy.rs`（15 条）                                                                                       |
| 6   | `services`（Docker）              | 3642 / 17（4 测试）   | tool window `Services`、service view、**`registerServiceProvider(dockerProvider)`**、`registerDockerRunTypes()`、`dockerGutter` editor contribution、12 个 action                         | **Services 树挂载时**：`docker events` 子进程 + 10 s 轮询（`ServicesTree.tsx:52` → `store.ts:258`） | 否                                                                                                     | 目录 + `commands/docker.rs`（14 条）+ `ide-docker` + `e2e/services.spec.ts`                                                                                 |
| 7   | `github`                          | 19583 / 88（12 测试） | tool window `Pull Requests`、editor kind（diff/review）、多个 action                                                                                                                      | 打开 tool window 才 `gh`                                                                            | 否                                                                                                     | 目录 + `commands/github.rs`（10 条）+ `ide-github` + `menu.rs` Git 菜单里的 PR 项                                                                           |
| 8   | `coverage`                        | 1180 / 8（1 测试）    | tool window `Coverage`、`run.coverage` / `run.coverageContext` action、gutter、`editor` 装饰                                                                                              | 无                                                                                                  | 否                                                                                                     | 目录 + `ide-coverage` + `commands/coverage.rs` + `menu.rs:571/597`                                                                                          |
| 9   | `profiler`                        | 684 / 6（1 测试）     | tool window `Profiler`、`run.profile` action                                                                                                                                              | 无                                                                                                  | 否                                                                                                     | 目录（**若保留 native perf 则保留 UI**）+ `ide-profiler` 部分                                                                                               |
| 10  | `build-tools`                     | 3329 / 13（5 测试）   | 条件注册 Maven/Gradle tool window、`registerEditorKind(ANALYZER_KIND)`、run-anything provider、多个 action                                                                                | `loadModel()` → `build_model`（**解析 pom.xml 树**，可能读 `~/.m2`）                                | 否                                                                                                     | 目录 + `ide-build` 的 Maven/Gradle 部分 + `settings/pages/execution.tsx:125-158`                                                                            |
| 11  | `libraries`（External Libraries） | 246 / 2（1 测试）     | `registerProjectViewRoot` + `registerActions(["libraries.reload"])`                                                                                                                       | **activate 时立刻 `sdkProjectLibraries()`**，之后每次 manifest 变更 500 ms 防抖再跑                 | 否                                                                                                     | 目录 + `ide-sdk/src/libraries.rs` + `sdk_project_libraries` 命令                                                                                            |
| 12  | `web`                             | 280 / 3               | `registerTextEditorWrapper`（预览面板）、2 个 menu group、action（Open in Browser / Reload）                                                                                              | 无                                                                                                  | 否                                                                                                     | 目录 + `commands/web.rs`（3 条）+ `ide-web`（`browsers` 部分如果不需要"Open in Browser"）                                                                   |

> `dependencies`（418 行，Cargo 版本提示）与 `analysis`（2319 行，通用检查）不在 12 个里：前者是 Rust only，可归入 §2/§4.8；后者对 C++ 有效，保留。

### 5.3 设置页（Languages & Frameworks / Build, Execution, Deployment）

| 页面 id                                    | 文件:行                                                      | 归属                                                                 | 处理                                                                                                                                              |
| ------------------------------------------ | ------------------------------------------------------------ | -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `python.interpreter`                       | `settings/pages/languages.tsx:18`                            | Python                                                               | 删（含 `languages/PythonInterpreterPage.tsx`、`PythonStatus.tsx`、`AddInterpreterDialog.tsx`；`DownloadJdkDialog.tsx` 属 `languages.java`，另删） |
| `languages.go` / `.buildTags` / `.modules` | `languages.tsx:48/58/69` + `GoBuildTags.tsx`/`GoModules.tsx` | Go                                                                   | 删（3 页 + `settings/__tests__/goBuildTags.test.ts`、`languages.test.ts`）                                                                        |
| `languages.java`                           | `languages.tsx:80`                                           | Java（JDK/Download JDK）                                             | 删                                                                                                                                                |
| `languages.node`                           | `languages.tsx:90`                                           | Node                                                                 | 删                                                                                                                                                |
| `languages.prettier`                       | `languages.tsx:100` + `PrettierPage.tsx`                     | Prettier（web 格式化）                                               | 删（连带 `ide-tools` 的 `prettier` ToolSpec、`languages/*.toml` 的 `formatter` 段）                                                               |
| `languages.rust`                           | `languages.tsx:110`                                          | Rust                                                                 | 视 Rust 去留                                                                                                                                      |
| `languages.typescript`                     | `languages.tsx:129`                                          | TS                                                                   | 删                                                                                                                                                |
| `build.gradle` / `build.maven`             | `settings/pages/execution.tsx:125/148`                       | JVM 构建                                                             | 删                                                                                                                                                |
| `build.pythonDebugger`                     | `settings/pages/execution.tsx:263`                           | Python 调试                                                          | 删                                                                                                                                                |
| `build.debugger.stepping`                  | `settings/pages/execution.tsx:244`                           | 含 Python `justMyCode` / node_modules 文案                           | 改文案或保留（C++ 的 "Do not step into library code" 也适用）                                                                                     |
| `languages`（页面注册装配）                | `settings/pages/registry.tsx:16-28`                          | `LANGUAGE_PAGES` / `EXECUTION_PAGES` 数组                            | 从数组里移除即可，页面文件后删                                                                                                                    |
| Language Servers 页                        | `src/features/tools/index.ts:16-37`（`SETTINGS_PAGE_ID`）    | ★ **保留**（clangd / lldb-dap / neocmakelsp 的安装与路径配置都在这） |

### 5.4 其他静态引用（删 feature 后必须一起改）

| 引用        | 位置                                                                                                                                                                       | 说明                                                                                                                                             |
| ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| 原生菜单项  | `src-tauri/src/menu.rs:571`（`run.coverage`）、`:597`（`coverage.hide`）、`:696-701`（`http.newRequest`/`http.showToolWindow`/`db.*`）、`:703-730`（Deployment 子菜单）    | 菜单按 action id 静态列出；action 消失后菜单项变灰/无效，需删                                                                                    |
| Keymap      | `src/platform/keymaps/{intellij,vscode,eclipse}-*.ts` 的 `db.showToolWindow`、`agents.showToolWindow`、`agents.newTask`、`agents.sendToAgent`、`agents.findInConversation` | 键位表允许未注册的 id（值为 `[]`），但 `ideaParity.test.ts:93-94` 断言 `code.splitLine` 与 `http.runRequest`/`db.execute` 的**顺序**，必须改测试 |
| 检查项      | `src/platform/inspections.ts:258-266`（`builtin:sql`）、`:236-243`（Cargo 依赖检查）                                                                                       | 从 `BUILTIN_INSPECTIONS` 删                                                                                                                      |
| 图标        | `src/ui/icons`（`toolwindow-database`/`toolwindow-agents`/`toolwindow-services`/`file-http`/`file-sql`/`maven-project`/`file-gradle`/`file-python`/`file-go`/…）           | 可选清理；不影响编译                                                                                                                             |
| i18n 字符串 | `src/platform/i18n`（若按 key 收集）                                                                                                                                       | 需跑一次 `pnpm knip` 找孤儿                                                                                                                      |

---

## 6. 依赖边：谁挂了谁

### 6.1 C/C++ 相关 crate → 非 C++ crate（需要切断）

| 依赖方                        | 被依赖                                                                                                           | 证据                                                                                     | 切断方式                                   |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- | ------------------------------------------ |
| `ide-host`                    | `ide-notebook`                                                                                                   | `Cargo.toml`；`src/notebook.rs`、`ops/notebook.rs`、`project.rs:52/285/307/402`          | 删 `Project.notebooks` 字段 + 模块         |
| `ide-host`                    | `ide-db`                                                                                                         | `Cargo.toml`；`project.rs:46/306/335` `Arc<ide_db::DbManager>`                           | 删 `Project.db` 字段 + `project_services`  |
| `ide-host`                    | `ide-http`                                                                                                       | `Cargo.toml`；`project.rs:48/346`；`src-tauri/src/commands/window.rs:326`（**updater**） | 保留 `client.rs`（抽出），删 `History`     |
| `ide-host`                    | `ide-agent`                                                                                                      | `Cargo.toml`；`project.rs:50/348`；`ops/agent.rs`；`registry::maintain`                  | 删 `Project.agents` + `ops/agent.rs`       |
| `ide-host`                    | `ide-sdk`（python/npm/go/java/libraries/angular/vue/godot）                                                      | `project.rs:127-146`（config hook）、`ops/sdk.rs`、`ops/python.rs`、`ops/jvm_icons.rs`   | 只删模块，保留 crate                       |
| `ide-host`                    | `ide-build`（maven/gradle/sync）                                                                                 | `ops/build.rs`（`build_model`/`build_sync`）、`ops/project.rs`                           | 拆模块                                     |
| `ide-host`                    | `ide-run`（tomcat/junit/dotnet/go/python_path）                                                                  | `ops/run.rs:125/135`、`ops/run/commands.rs:16/117/132/158`                               | 删文件                                     |
| `ide-host`                    | `ide-dap`（debugpy/js-debug/delve/netcoredbg）                                                                   | `ops/debug.rs`、`Cargo.toml`                                                             | 只删 `adapter.rs::builtin` 分支            |
| `src-tauri`                   | `ide-docker`/`ide-github`/`ide-notebook`/`ide-db`/`ide-http`/`ide-web`/`ide-profiler`/`ide-coverage`/`ide-agent` | `src-tauri/Cargo.toml` + `src-tauri/src/commands/mod.rs`                                 | 删依赖 + 命令模块                          |
| `ide-profiler`/`ide-coverage` | `ide-run`                                                                                                        | `Cargo.toml`、`plan.rs:5`                                                                | 删 crate 或裁剪后仍依赖 `wrap`/`RunConfig` |
| `ide-tools`                   | `ide-language`                                                                                                   | `registry.rs` 测试 `registry_names_match_language_configs`                               | 保留（test 需要）                          |

### 6.2 **不**依赖非 C++ crate 的（可放心改）

| crate                                                                                                                                                                        | 检查结果                                                                                           |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `ide-index`                                                                                                                                                                  | 无 `ide_notebook`/`ide_db`/`ide_http`/`ide_agent` 引用                                             |
| `ide-language`                                                                                                                                                               | 同上（只依赖 `ide-core`/`ide-text`/tree-sitter）                                                   |
| `ide-vfs` / `ide-text` / `ide-search` / `ide-syntax` / `ide-cmake` / `ide-lsp` / `ide-vcs` / `ide-terminal` / `ide-jsonrpc` / `ide-workspace` / `ide-spell` / `ide-analysis` | 同上                                                                                               |
| `ide-run`                                                                                                                                                                    | 只被 `ide-host`/`ide-sdk`/`ide-profiler`/`ide-coverage`/`ide-dap` 依赖；自身不依赖其他非 C++ crate |
| `ide-remote`/`ide-ssh`                                                                                                                                                       | `ide-remote` 只依赖 `ide-core`/`ide-ssh`/`ide-run`；与语言无关                                     |

### 6.3 必须保留的 `Cargo.toml` 条目（删除时别误伤）

| 条目                                                                                  | 为什么                                    |
| ------------------------------------------------------------------------------------- | ----------------------------------------- |
| `tree-sitter` + 41 个 grammar（`crates/ide-language/Cargo.toml:23-57`）               | 编辑器高亮 / outline / folding / 结构选择 |
| `@codemirror/language-data`、`@codemirror/legacy-modes`、`@lezer/*`（`package.json`） | 前端词法高亮                              |
| `reqwest`/`rustls`/`rustls-platform-verifier`                                         | updater 与 `ide-tools` 下载               |
| `tokio`/`parking_lot`/`notify`/`notify-debouncer-full`/`rayon`                        | 文件监视、索引、进程                      |
| `cmake` 相关：`ide-cmake`、`ide-build::compile` CMake 分支                            | ★ 构建                                    |
| `ide-dap` 的 `lldb-dap`/`codelldb` 分支                                               | ★ 调试                                    |
| `ide-tools` 的 `clangd`/`lldb-dap`/`neocmakelsp`                                      | ★ 工具安装                                |

---

## 7. 运行时 / 内存成本（可证据化）

> 说明：标注「无条件」= 每个 IDE 进程都会发生；「按项目」= 打开项目时；「按需」= 用户触发。

| #   | 成本                            | 位置（file:line）                                                                                                                                                                                                                                                                          | 何时                                                                                                      | 量级 / 说明                                                                                                                                                 |
| --- | ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | 构建全部 tree-sitter `Language` | `src-tauri/src/state.rs:61`（线程 `openclion-syntax-init`）→ `crates/ide-language/src/syntax/mod.rs:88-98`，`grammars.rs` `SPECS`（35 个）                                                                                                                                                 | **无条件**（启动后立刻，后台线程）                                                                        | 35 个 grammar 常驻；`Language::new` 为每个分配解析表。删到 8 个（C/C++/ObjC/CMake/Make/YAML/JSON/TOML）可显著降低 RSS 与启动时间                            |
| 2   | 索引线程                        | `crates/ide-index/src/lib.rs:126`                                                                                                                                                                                                                                                          | 按项目（`open_project`）                                                                                  | 1 个后台线程 + 扫描；`ProjectIndex::open`（`:90`）                                                                                                          |
| 3   | 文件监视                        | `crates/ide-host/src/project.rs:184`（`Watcher::start`）、`:253`（scratch）、`:845`（content root watchers）                                                                                                                                                                               | 按项目                                                                                                    | `notify` debouncer 线程 + inotify/FSEvents 句柄                                                                                                             |
| 4   | 本地历史清理线程                | `crates/ide-host/src/project.rs:103`（`openclion-history-prune`）                                                                                                                                                                                                                          | 按项目                                                                                                    | 短命线程                                                                                                                                                    |
| 5   | 仓库扫描线程                    | `crates/ide-host/src/project.rs:277`（`openclion-vcs-roots`）                                                                                                                                                                                                                              | 按项目                                                                                                    | 递归找 `.git`                                                                                                                                               |
| 6   | 项目服务三件套                  | `crates/ide-host/src/project.rs:284`（`project_services`）→ `:335-363`                                                                                                                                                                                                                     | 按项目 + **每个 LightEdit 窗口**（`:401`）                                                                | `DbManager`（`source::load`、`SecretStore`）+ `History` + `AgentManager`（读 `store_dir/*.json`）                                                           |
| 7   | DB 历史线程                     | `crates/ide-db/src/lib.rs:328`（`openclion-db-history`）                                                                                                                                                                                                                                   | 每个 `DbManager::new`                                                                                     | 常驻 mpsc 线程，即使从未连库                                                                                                                                |
| 8   | ACP registry 维护               | `crates/ide-agent/src/registry.rs:637-659`（线程 `agent-registry`）                                                                                                                                                                                                                        | **第一个项目打开时一次**（`project.rs:289`）                                                              | `prune()` + 可能的 HTTPS `fetch()`（ACP Registry，有网络与超时）                                                                                            |
| 9   | External Libraries 探测         | `crates/ide-sdk/src/libraries.rs:71-79`（`std::thread::scope` 6 个）+ 子进程：`:159` `python -c`、`:369` `rustc --version`、`:371` `rustc --print sysroot`、`:382` `cargo metadata --offline`（60 s 超时）、`:441` `go env`、`:462` `go list -m all`（60 s）；Maven/Gradle jar 列表 `:227` | 按项目（`src/features/libraries/index.ts:139` activate 即跑）+ 每次 manifest 变更 500 ms 后（`:147-151`） | **最贵的项目打开路径**：最多 6 线程 + 6 子进程；大项目上 `cargo metadata` 可跑满 60 s                                                                       |
| 10  | Maven/Gradle 模型               | `crates/ide-build/src/maven.rs:867` / `gradle.rs:238`，经 `src/features/build-tools/index.ts:158` `loadModel()`                                                                                                                                                                            | 按项目 + 构建文件变更                                                                                     | 递归读 `pom.xml` + `~/.m2` 父 POM（纯 IO，可能上千文件）                                                                                                    |
| 11  | 工具探测                        | `crates/ide-tools/src/lib.rs:703-728`（8 线程 `std::thread::scope`，每个工具跑 `--version`）                                                                                                                                                                                               | 打开 **Settings › Language Servers** 时（`tools_list`）                                                   | 47 个 `ToolSpec` + 语言里出现的 server，最多 ~50 次 `--version` 子进程                                                                                      |
| 12  | Jupyter kernel                  | `crates/ide-notebook/src/kernel/process.rs:29-95`、`transport.rs:155/214`                                                                                                                                                                                                                  | 执行 notebook cell 时                                                                                     | **每个 notebook 1 个 Python 进程 + 5 个 ZMQ socket + 1 个 tokio 任务**；`MAX_OUTPUT_BYTES = 32 MiB`、`MAX_OUTPUT_MESSAGES = 10_000`（`transport.rs:23-26`） |
| 13  | Docker events + 轮询            | `src/features/services/store.ts:227-273`（`setInterval(...,10_000)`、`commands.dockerEvents`）                                                                                                                                                                                             | Services 树**可见期间**（`ServicesTree.tsx:52`）                                                          | 每条连接 1 个 `docker events` 子进程 + 10 s 轮询；即使没有容器                                                                                              |
| 14  | DB 连接池                       | `crates/ide-db/src/conn.rs:535`（`MAX = 4`）                                                                                                                                                                                                                                               | connect 时                                                                                                | 每个 data source ≤ 4 连接；前端 5 s 空闲断连（`src/features/database/store.ts:403`）                                                                        |
| 15  | Agent 子进程                    | `crates/ide-agent/src/lib.rs:241-247`（`Peer::spawn_framed` / native connect）                                                                                                                                                                                                             | 每个运行中的 agent 任务                                                                                   | 1 个 CLI 子进程（`claude`/`codex`/`npx …`）+ 1 个 `git worktree`                                                                                            |
| 16  | 内建 Web 服务器                 | `crates/ide-web/src/server.rs:104-115`                                                                                                                                                                                                                                                     | 首次 `web_url`（"Open in Browser"）                                                                       | 1 个 `TcpListener`（127.0.0.1:63342）+ accept 循环                                                                                                          |
| 17  | LSP server                      | `crates/ide-lsp/src/lib.rs:3-5`（"Servers start lazily per (server, workspace root) when a document of a matching language opens"）                                                                                                                                                        | 打开对应语言文件                                                                                          | 每个 server 一个子进程；`clangd --background-index` 会建后台索引（`languages/c.toml:13`）                                                                   |
| 18  | JVM 图标探测                    | `src/platform/filePresentation.ts:22/41` → `commands.jvmFilePresentations` → `crates/ide-host/src/ops/jvm_icons.rs`                                                                                                                                                                        | Project 树 / 编辑器标签 / Search Everywhere 中**可见的** `.java`/`.kt`                                    | 每批 ≤ 64 文件，每个读前 64 KB（`PREFIX_BYTES`），缓存 4096——纯残留开销                                                                                     |
| 19  | 依赖版本检查                    | `crates/ide-host/src/ops/dependencies.rs:20-50`（`https://index.crates.io`）                                                                                                                                                                                                               | 打开 `Cargo.toml` 后 500 ms 防抖                                                                          | 网络请求；Rust only                                                                                                                                         |
| 20  | 运行配置探测                    | `crates/ide-run/src/detect.rs:31`                                                                                                                                                                                                                                                          | 按项目（`run_configs`）                                                                                   | 纯文件读取，无子进程                                                                                                                                        |

---

## 8. 推荐的删除顺序（5 个阶段）

> 每个阶段的验收门（gate）：
> `cargo check --workspace --all-targets` + `cargo test -p ide-language -p ide-tools -p ide-run -p ide-dap` + `pnpm typecheck` + `pnpm test`。
> 涉及命令面变更的阶段，额外跑 `pnpm gen:bindings`（内部是 `cargo test -p openclion --test export_bindings`）并提交 `src/bindings.ts` 的 diff。

### Stage 1 — 数据面（TOML + registry + detect + 前端 run 类型）

**目标**：所有"语言 × 工具"的声明消失，但 C/C++ 行为不变。

1. `languages/*.toml`：删非 C++ 语言的 `[[language-servers]]`、`[debugger]`、`formatter`、`[[framework-ui-modes]]`（`html.toml:13-15`）。
2. `crates/ide-tools/src/registry.rs`：`TOOLS` 只留 `clangd`、`lldb-dap`、`neocmakelsp`（可选 `codelldb`）；删 `Method` 的 8 个变体与 `ReleaseSource::HashiCorp/OpenVsx`。
3. `crates/ide-run/src/detect.rs`：删 9 个 `detect_*` 调用与函数；`rm crates/ide-run/src/detect/dotnet.rs`；删 `src/{tomcat,junit,java_cmdline,gradle_jvm,dotnet,go,python_path}.rs` 及其 `ops`/命令。
4. `crates/ide-dap/src/adapter.rs:60-92`：`builtin()` 只留 `lldb-dap`（+`codelldb`）；删 `cargo.rs`。
5. 前端：删 `run/{jvmTypes,tomcat,goTypes,goActions,goGenerate,goModules,goArguments,dotnetTypes,serverPort}.ts`、`run/testing/{js,go,jvm,pytest}.ts`、`types.ts` 的 `BUILTIN_TYPES` 项、`producers.ts` 的语言分支。
6. 测试：`run/{goActions,goTypes,goGenerate,dotnetTypes,tomcat,services}.test.*`、`run/testing/parsers.test.ts` 中对应断言。

**为什么先做**：纯数据 + 叶子模块，不动 `Project`/`Host` 结构，回滚容易；此时后端命令仍在，前端删掉调用方后 `typecheck` 依然绿。
**风险**：低。唯一硬约束是 `registry.rs:864` 的测试要求 TOML 与 registry 同步。

### Stage 2 — 前端 feature 下线（后端命令暂留）

1. `src/features/index.ts`：删 12 个 import 与数组项；`activateSettingsWindowFeatures`（`:118-131`）里只删 `agentsPages`（`toolsPages` 保留）。
2. 删目录：`database`、`http-client`、`notebook`、`agents`、`deployment`、`services`、`github`、`coverage`、`profiler`、`build-tools`、`libraries`、`web`。
3. 删设置页：`settings/pages/languages.tsx` 全表、`execution.tsx:125-158`（Gradle/Maven）、`:263-286`（Python Debugger）；`settings/pages/registry.tsx:16-28` 去引用。
4. 删静态引用：`menu.rs:571/597/696-730`、keymap 键、`inspections.ts:258-266`、`ideaParity.test.ts:93-94`、`app/__tests__/consoleCopy.test.ts`、`e2e/services.spec.ts`、`e2e/agentsDock.spec.ts`。
5. `pnpm knip` 找剩余孤儿文件。

**此时**：`src/bindings.ts` 里还有 db/notebook/… 的命令，但无人调用；`pnpm typecheck` 绿（knip 可能报 unused exports，属预期）。
**风险**：中——`services` feature 与 `run/services.ts` 同名，别删错；`deployment` 的**设置页**与 **feature** 是两个文件。

### Stage 3 — 后端命令面 + 叶子 crate

1. `src-tauri/src/commands/mod.rs`：删 `agent`(22，`:97-118`)、`notebook`(11，`:119-129`)、`docker`(14，`:431-444`)、`github`(10，`:445-454`)、`db`(28，`:462-476`+`:521`)、`deploy`(15，`:477-490`)、`http`(7)、`coverage`(3)、`profiler`(2)、`web`(3)、`tools` 里的 `jdk_*`(4)、`sdk` 里的 python/libraries(5)、`build` 里的 maven 项、`run` 里的 tomcat/junit/gradle/dotnet(10，逐条核)。
2. 删 `src-tauri/src/commands/{agent,notebook,db,docker,github,http,coverage,profiler,web,deploy}.rs`；`src-tauri/src/state.rs:22/103` 删 `web` 字段。
3. `crates/ide-host`：删 `src/notebook.rs`、`ops/{notebook,agent,python,jvm_icons}.rs`（`ops/deploy.rs` 随 `deployment` feature 一起删；`remote-dev` 不受影响）、`project.rs:284-363` 的 `project_services`、`Project` 的 `db/http_history/agents/notebooks` 字段、`rpc.rs:161` 的 `jvm_file_presentations`。
4. 删 crate 与依赖：`ide-notebook`、`ide-db`、`ide-docker`、`ide-github`、`ide-agent`、`ide-coverage`；`ide-http` 只留 `client.rs`（搬到 `ide-core` 或新 `ide-net`）；`ide-web` 只留 `browsers.rs`（若不再需要"Open in Browser"则整删）。
5. `cargo test -p openclion --test export_bindings` 重生成 `src/bindings.ts`。

**风险**：高（跨 crate 边最多）。建议每个 crate 一笔提交，先删依赖项再删代码。

### Stage 4 — 复合 crate 内部裁剪

1. `crates/ide-sdk`：删 `python.rs`、`npm.rs`、`go.rs`、`java.rs`、`libraries.rs`、`angular.rs`、`vue.rs`、`godot.rs`；`discover.rs` 随 `lib.rs:32-44` 的 `SdkKind` 一起删（`cpp.rs` 的 `CompilerKit::detect` 是独立入口，不受影响）；`apply.rs` 删语言分支（保留 `setting_path`、`tool_command`、`server_launch_overrides`、`terminal_activation`）。
2. `crates/ide-tools`：删 `jdk.rs`、`install` 的 npm/pip/gem/dotnet/go 分支、`prettier.rs` 测试。
3. `crates/ide-build`：删 `maven.rs`、`gradle.rs`、`sync.rs`、`pom_edit.rs`、`application_update.rs`、`web.rs`；`compile/mod.rs` 只留 `BuildSystem::{CMake}`（可选 Cargo）。
4. `crates/ide-profiler`：删 `jvm.rs`、node/python 分支；`crates/ide-coverage`：整删（或重写为 ctest+gcov）。
5. `crates/ide-syntax`：删 `jvm.rs` + `lib.rs:23` 的 `mod jvm` 与 `lib.rs:34` 的 `pub use jvm::{JvmPresentation, JvmTypeKind}`。
6. 前端：删 `src/platform/filePresentation.ts` 与 4 处调用（`ProjectView.tsx`、`EditorArea.tsx`、`SearchEverywhere.tsx`、`RecentPopups.tsx`）。
7. `src/features/settings/pages/tools.tsx`、`behavior.tsx` 里与语言/工具相关的项按需清。

**风险**：中（`ide-build::compile` 与 `ide-sdk::apply` 是共享面，改动要跑 `cargo test -p ide-build -p ide-sdk`）。

### Stage 5 — 语法收尾与文档

1. 删确认不需要的语言 TOML（§1.3「整删」列）与 `ide-language/src/registry.rs:10-76` 的 `BUILTIN` 项。
2. 删 `crates/ide-language/Cargo.toml:23-57` 中不再有 TOML 引用的 grammar；`grammars.rs` `SPECS` 同步删。
3. 删 `src/editor/core/cm/angular/`（68 K）与 `languages.ts:93` 的 `Angular Template`；删 `languages.ts` 的 `Kotlin/Scala/Dart/Groovy/C#` own-modes（`:77-91`）、`Batch` 解析器（`:96-160`）。
4. 删 `src/features/run/testing/cargo.ts`（若 Rust 不要）。
5. 更新 `THIRD_PARTY_LICENSES.md`（上一笔 JVM 提交就删了 25 行）、`docs/porting-idea-to-clion.md` §6 的 "Deliberate gaps"。
6. 最终验证：`cargo check --workspace --all-targets`、`pnpm typecheck && pnpm test && pnpm lint && pnpm knip`、`pnpm e2e`。

**风险**：低（纯删除），但 `cargo check` 会暴露 `#[cfg]` 漏网的引用。

---

## 9. 必须保留：C/C++/CMake 依赖的东西

| 保留项                                                                                                                                                                                                                                           | 依赖路径（为什么）                                                                                                      |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------- |
| `crates/ide-cmake`                                                                                                                                                                                                                               | CMake 模型、profile、presets、CTest（`cmake_model`/`cmake_presets` 命令）                                               |
| `crates/ide-build::compile` 的 `BuildSystem::CMake`                                                                                                                                                                                              | `crates/ide-build/src/compile/mod.rs:208` + `cmake_steps`（`:307`），Build Project 的 C++ 路径                          |
| `crates/ide-build/src/xml.rs`                                                                                                                                                                                                                    | `compile`（读 `pom.xml`）与 `pom_edit` 共用；删 Maven 后要确认 `compile` 是否仍需要它                                   |
| `crates/ide-dap`（`lldb-dap` / 可选 `codelldb`）                                                                                                                                                                                                 | `languages/{c,cpp,objective-c,objective-cpp}.toml` 的 `[debugger]`；`ops/debug.rs` 29 条命令                            |
| `crates/ide-lsp` 核心（`lib.rs`/`launch.rs`/`server.rs`/`handler.rs`/`caps.rs`/`convert.rs`/`virtual_doc.rs`/`inspect.rs`）                                                                                                                      | clangd 的 LSP 通道；`launch.rs:236` 的 clangd 默认 args（`--background-index`、`--compile-commands-dir`）               |
| `crates/ide-lsp/src/angular.rs`、`external_project.rs`、`tsserver_bridge.rs`                                                                                                                                                                     | **不保留**（只服务 Angular/TS），删                                                                                     |
| `crates/ide-run::process`                                                                                                                                                                                                                        | 所有运行/构建/终端进程（`ProcessManager`）                                                                              |
| `crates/ide-run::which`                                                                                                                                                                                                                          | `ide-sdk` 的 `locate`、profiler 的 `perf` 查找                                                                          |
| `crates/ide-run::config`（`RunConfig`）                                                                                                                                                                                                          | 共享 IPC 类型；删语言分支但保留结构                                                                                     |
| `crates/ide-tools` 的 `clangd`/`lldb-dap`/`neocmakelsp`                                                                                                                                                                                          | `languages/*.toml` 的 `install-hint` 与 Settings › Language Servers 页                                                  |
| `crates/ide-tools::Http`                                                                                                                                                                                                                         | 工具安装器的下载通道（`clangd`、`neocmakelsp` 的 GitHub release）                                                       |
| `crates/ide-language`（registry + syntax）                                                                                                                                                                                                       | 全部高亮 / outline / folding / 结构选择                                                                                 |
| `crates/ide-syntax::cpp`（`ide-syntax/src/cpp.rs`）                                                                                                                                                                                              | `cpp_related_files`、`cpp_generate_definition`（`ops/cpp.rs`、`commands/cpp.rs`）                                       |
| `crates/ide-index`                                                                                                                                                                                                                               | 符号索引、Find Usages、补全                                                                                             |
| `crates/ide-terminal` / `ide-vcs` / `ide-search` / `ide-spell` / `ide-analysis`                                                                                                                                                                  | 语言无关的 IDE 基础能力                                                                                                 |
| `crates/ide-http::client`（**搬走后**）                                                                                                                                                                                                          | `src-tauri/src/commands/window.rs:326` updater 的代理配置                                                               |
| `src/features/{cmake,build,debugger,run,project-tree,search,code-insight,vcs,terminal,settings,tools,viewers,scratch,perf,plugins,templates,todo,vim,ssh,remote-dev,project-trust,welcome,app,macros,run-anything,valgrind,sanitizers,analysis}` | C/C++ IDE 的主干 UI                                                                                                     |
| `src/features/tools`                                                                                                                                                                                                                             | Language Servers 设置页（clangd / lldb-dap / neocmakelsp 的安装入口）                                                   |
| `src/features/cmake` + `runTypes.ts`                                                                                                                                                                                                             | CMake Application / CTest 运行配置                                                                                      |
| `src/features/valgrind`、`sanitizers`                                                                                                                                                                                                            | 上一笔提交新增的 C++ 内存诊断                                                                                           |
| `languages/{c,cpp,cmake,objective-c,objective-cpp,makefile}.toml`                                                                                                                                                                                | ★ 主语言                                                                                                                |
| `languages/{yaml,json,jsonc,toml,ini,gitattributes,ignore,diff,plaintext,markdown,log,dotenv,properties,shellscript}.toml`                                                                                                                       | C/C++ 项目里天天出现的配套文件（`.clang-format`、`compile_commands.json`、`.gitignore`、`CMakePresets.json`、构建脚本） |
| `tree-sitter` grammar：`c`、`cpp`、`cmake`、`make`、`yaml`、`json`、`toml`、`bash`（+ ObjC 无 grammar）                                                                                                                                          | C/C++ 及配套文件的高亮/outline                                                                                          |

---

## 10. 验证清单（每阶段）

```bash
# Rust
cargo check --workspace --all-targets
cargo test -p ide-language -p ide-tools -p ide-run -p ide-dap -p ide-build -p ide-sdk
cargo clippy --workspace --all-targets -- -D warnings

# 绑定（仅当 Tauri 命令面变化）
pnpm gen:bindings         # cargo test -p openclion --test export_bindings

# 前端
pnpm typecheck
pnpm test
pnpm lint
pnpm knip                 # 找删除后遗留的孤儿文件/导出

# 端到端（去除非 C++ 用例后）
pnpm e2e
```

**已知会绊住 typecheck 的 3 处**：

1. `src/bindings.ts` 未重生成（删了命令但前端仍有引用）→ Stage 3 必须跑 `pnpm gen:bindings`；
2. `src/features/app/__tests__/consoleCopy.test.ts` 直接 import `@/features/database/*` → Stage 2 一起删；
3. `src/platform/keymaps/ideaParity.test.ts:93-94` 引用 `http.runRequest` / `db.execute` 的顺序断言 → Stage 2 一起改。

---

_报告结束。全部结论来自只读检索；未执行构建，未修改除本文件外的任何文件。_
