# 启动路径性能分析：loading bar 归因与修复清单

> 只读调查（未改任何 tracked 文件；仅新增本报告）。测量时间 2026-11-03 09:41–10:02。
> 基线：`main` @ `fa12a42`（"docs: record the completed Java/JVM tooling removal"）+ **工作区里其他 agent 的未提交改动**，见 §1.4。
> 复现所需的一切命令、探针脚本路径与原始日志都在 §8。

---

## 0. 结论摘要

1. **loading bar 与 Rust 后端初始化几乎无关。** 它是 `index.html` 里的静态 `#splash`（假进度条，4 s 动画到 92%），唯一 gate 是 `holdSplash(launch)`：等 `windowTakeLaunch` → `openProject` 这条 IPC 链。
2. **`pnpm dev:app`（debug + Vite dev server）实测：进程启动 → 遮罩开始淡出 4.07–4.16 s，完全消失 4.38–4.45 s。**
3. 这 4.4 s 里最大的一块是 **dev 模式的 ESM 模块图**：首屏 **250 个请求 / 233 个 `/src` 模块 / 40 个（共 42 个）feature 目录全部在阻塞图里**，WebKit 到 `DOMContentLoaded` 要 2160–2245 ms，`main.tsx` 的第一条 IPC 在 2139–2222 ms（page time）才发出。**同一 binary 换成生产 bundle（`vite build` 产物）：遮罩 1.83 s 开始淡出 / ≈2.4 s 消失**，即 dev 罚时 **≈2.0 s**。
4. 第二块是 **`open_project`：740–880 ms**（warm），其中 **`Watcher::start` 635–771 ms** —— Linux 走 per-directory 策略，**对 18,522 个目录建了 4,181 个 inotify watch** 才返回；负载高/冷缓存时同一 open 到 **3.4–5.2 s**。
5. **审计 `docs/audit-non-cpp-features.md` §7 #1 关于 `state.rs:61` 的说法已过时**：当前 HEAD 的 grammar 构建已经在 `openclion-syntax-init` 线程上；实测 `Syntax::new()`（37 个 grammar + 又一次 registry load）**debug 2.31 ms / release 0.91 ms**、常驻 **3.5 MB RSS**，且被 `Host::syntax()` 懒取，不进启动关键路径。`LanguageRegistry::load`（73 个 TOML）**1.99 ms**。
6. 真正“贵”的 Rust 项只有 watcher（见 4）；language registry、settings、工具探测、LSP manager、index service、git、agent registry、plugin host 都 **≤ 数十 ms 或走后台线程**（§5 逐项给了测量）。
7. `AppState::new` 自身 **3.4–4.6 ms**；进程启动到 setup 完成 **106–120 ms**（含 Tauri 初始化、login shell PATH 缓存、`export_bindings`、menu）。
8. 工作区已有并发改动把 splash 的 hold 上限设为 **700 ms**（`LAUNCH_WAIT_MS`），本报告测的就是这版；HEAD（无上限）等价于“等整个 `open_project`”，冷开 kaiwudb 会多等 2.7–3.4 s。
9. **不挡 bar 但让 kaiwudb “第一次打开很慢”**：index worker 首次全量 parse **45,950 files / 17.5 s**（纯 CPU），以及 clangd `--background-index` 首次 `initialize` **102 s**（`languages/c.toml:13`）。两者都在后台，但会和 UI 抢 CPU/IO。
10. 可落地的最大三笔：**拆 features 的 import/activate 链（dev −1.3~1.5 s）**、**splash 不再等 `open_project`（−0.7 s warm，冷开 −2.7~3.4 s）**、**`Watcher::start` 增量化/并行化（−0.6~0.8 s）**；仅开发体验还能靠“dev 用 build 产物”再省 2 s。

---

## 1. 测量方法与环境

### 1.1 机器与构建基线

| 项        | 值                                                                                                                           |
| --------- | ---------------------------------------------------------------------------------------------------------------------------- |
| CPU       | Intel Core i7-9700 @ 3.00 GHz，8 核 / 8 线程                                                                                 |
| 内存      | 61 GiB（测量时 16 GiB 已用，43 GiB buff/cache）                                                                              |
| OS / 会话 | Linux，Wayland（`WAYLAND_DISPLAY=wayland-0`）+ X11 `:0`，WebKitGTK webview                                                   |
| 构建      | **debug**（`pnpm dev:app` 同款，`target/debug/openclion`，1.8 GB，text 190 MB）                                              |
| 前端      | Vite 8.3.4 dev server（`http://localhost:22014`，即本 worktree 的 `preferredVitePort`），未压缩 ESM                          |
| 项目      | `/home/limstorm/gowork/src/gitee.com/kwbasedb`（kaiwudb）：157,681 个文件 / 18,522 个目录（不含 `.git`）/ 索引 45,952 个文件 |
| 工具链    | cargo/rustc 1.99.1，Node 24.21，Playwright 1.64                                                                              |

“warm” = 同一 Vite 实例、OS page cache 已热；本报告的 dev 数字取两次连续启动（§3 给了两列）。

### 1.2 用了哪五种测量手段

1. **应用自带日志（最可信的 Rust 侧）**：`src-tauri/src/state.rs:117 note_ipc()` 打印 `startup: first IPC … (main …, setup …, took …)`；`crates/ide-host/src/project.rs:282` 打印 `opened <root> in <n> ms [("roots", …), ("index", …), ("watcher", …), ("git", …)]`；`ide_index::worker` 打印 `indexed N files (M parsed) in T`。日志文件：`~/.local/share/ai.l0ng.openclion.dev.wb879b047d6acc5a6/logs/openclion.2026-11-0{2,3}.log`。
2. **Tauri MCP bridge 探针（真实 WebKit 里的前端时间线）**：debug 构建启用了 `tauri-plugin-mcp-bridge`（`src-tauri/src/lib.rs:96`，`127.0.0.1:9223`）。用 Node 内置 `WebSocket` 连上后发 `{"command":"execute_js","args":{"script":…,"windowLabel":"main"}}`，在页面里每 100 ms 采样 `#splash` 是否还在、`splash-out` class、`[role=tree][aria-label=Project]` 是否出现、DOM 节点数，并读 `performance.getEntriesByType("paint"|"navigation"|"resource")`。脚本：`target/perf-probe/app-probe.mjs`；结果：`target/perf-probe/app-{run1,run2,prod1}.json`。
3. **同一 binary 的 A/B（dev ESM vs 生产 bundle）**：debug binary 编译期嵌入了 `http://localhost:22014`（`strings target/debug/openclion`）。把该端口从 Vite 换成 `python3 -m http.server --directory target/perf-probe/dist-prod`（`vite build` 产物），Rust 侧、WebKit、项目、探针全部不变，只换前端分发方式 → 干净地隔离出“dev 模块图”的代价。
4. **Rust 侧微基准**：仓库里已有的 `crates/ide-language/examples/startup_cost.rs`（release：`cargo run --release -p ide-language --example startup_cost`）给出 release 口径；我另写了一个临时变体（untracked，测完已删）加上 `fork`/`set_file_types`/`SyntaxCache::new` 与 `/proc/self/status:VmRSS` 前后差，用 debug profile 跑（与 `pnpm dev:app` 同口径）。
5. **浏览器侧请求分类 + CPU profile**：Playwright Chromium 打开 dev server，统计 `performance.getEntriesByType("resource")` 的 URL 前缀与字节，并用 CDP `Profiler` 取 self-time 前 40。

外加：`/proc/<pid>/fdinfo/*` 数 inotify watch（`grep -c '^inotify wd:'`），`strace -f -c -e trace=inotify_add_watch,…` 观察 syscall 放大后的归因（只用于相对比较）。

### 1.3 度量口径与陷阱

- **`process_to_main` 不可信，不要引用**：它来自 `crates/ide-workspace/src/process.rs:187`（Linux 读 `/proc/<pid>` 目录 mtime）。同一 binary 上它给出过 1 ms / 2 ms / 9.7 s / 55 s / 103 s / 110 s —— 机器 idle 时正常，有负载时是垃圾值。本报告一律用**外部 wall clock（spawn 时刻）+ 日志时间戳 + 页面 `performance.now()` 对齐**：
  `process_relative_ms = page_time_ms + (performance.timeOrigin − spawn_wall_time)`，三条 run 的交叉校验都能对上（例：prod run 的 `timeOrigin − spawn = 412 ms`，first IPC 记为 page 429 ms → 841 ms，与 Rust 日志的 841 ms 完全一致）。
- “遮罩开始淡出” = `#splash` 加上 `splash-out`（此时 UI 已可点，只是还在 160 ms 淡出）；“完全消失”= 元素被移除（`+200 ms` 固定 `setTimeout`）。
- WebKit 的 `encodedBodySize` 对 chunked 响应不稳定（同一 dev 页两次跑出 1.2 MB / 3.3 MB），**字节数只作量级参考，请求数才是硬指标**；字节数另用 Chromium + 生产构建交叉核对。

### 1.4 工作区状态（会改变结论的部分）

`git status` 里除本报告外还有**其他 agent 未提交的改动**（不是本报告作者所为）：

```
 M crates/ide-index/src/lib.rs          M crates/ide-index/src/worker.rs
 M crates/ide-language/src/lib.rs       M crates/ide-language/src/syntax/mod.rs
 M src/features/app/windows.ts          M src/platform/splash.ts
?? crates/ide-index/src/perf_probe.rs
```

其中与 loading bar 直接相关的是 `src/platform/splash.ts` + `src/features/app/windows.ts`：新增 `LAUNCH_WAIT_MS = 700`，`holdSplash(launch, 700)` —— **项目打开最多只压住遮罩 700 ms，之后的 `open_project` 在工作台后面继续跑**。本报告所有前端测量都基于这版（已用 `curl http://localhost:22014/src/platform/splash.ts` 确认 dev server 服务的就是它）。
**HEAD 行为**（无 700 ms 上限）= 遮罩等完整 `open_project`，只受 `MAX_WAIT_MS = 4000` 兜底；两者差值见 §6 的 F2。

---

## 2. loading bar 到底是什么

`index.html` 里是一段**静态** splash 标记，不是 React 组件，所以它在任何 JS 之前就能绘制：

- `index.html:12` `<div id="splash" class="splash">`，`:35` `<div class="splash-bar" role="progressbar">`
- `public/splash.css`：`.splash-bar > i` 的 `splash-fill` 动画 **4 s 缓动到 scaleX(0.92)** —— 这是“假进度”，与真实进度无关；`#splash` 在 settings 窗口里只显示 bar。
- `public/boot.js`：在首屏 CSS 之前恢复上次主题，避免闪白（有 `localStorage` 读 + 内联 `<style>`，~1 ms 级）。

**唯一让它消失的路径**（当前工作区版本）：

```
index.html:12                     #splash（静态，FCP 时就画出来了）
src/main.tsx:43                   ReactDOM.createRoot(...).render(<Workbench/>)
src/main.tsx:46                   hideSplash()
src/platform/splash.ts:39         hideSplash():
                                    Promise.race([Promise.all(holds.map(wait)), 4000ms])   // :42-44
                                    → 2×requestAnimationFrame → completeBar() FILL_MS=120   // :46-54
                                    → el.classList.add("splash-out")（160ms CSS 过渡）        // :50
                                    → setTimeout(el.remove, 200)                            // :51
src/platform/splash.ts:34         holdSplash(work, maxMs = 4000)  ← 唯一的生产者↓
src/features/app/windows.ts:453   holdSplash(launch, LAUNCH_WAIT_MS=700)   [工作区改动]
src/features/app/index.ts:47      installLaunchHandling()   ← 由 features/index.ts:52-95 的 "app" 项调用
src/features/app/windows.ts:205   consumeLaunch():
                                    commands.windowTakeLaunch()          // IPC #1
                                    → openHere(launch.project)           // :52
                                    → openProject(path)                  // src/platform/project.ts:51
                                    → commands.openProject(path)         // IPC #2
src-tauri/src/commands/window.rs:122   window_take_launch
src-tauri/src/commands/app.rs:48       open_project  → spawn_blocking(AppState::open_project)
src-tauri/src/state.rs:138             AppState::open_project  → host.open_project + recent.touch/rename
crates/ide-host/src/project.rs:63      Host::open_project（laps: :92 roots / :119 index / :274 watcher / :281 git / :282 log）
```

时序常量（决定“最短也要多久”）：

| 常量                                            | 值                               | 位置                                 |
| ----------------------------------------------- | -------------------------------- | ------------------------------------ |
| `MAX_WAIT_MS`（所有 hold 的总兜底）             | 4000 ms                          | `src/platform/splash.ts:13`          |
| `LAUNCH_WAIT_MS`（launch 单独上限，工作区新增） | 700 ms                           | `src/platform/splash.ts:15`          |
| `FILL_MS`（把假进度条补到 100%）                | 120 ms                           | `src/platform/splash.ts:16`          |
| 淡出 / 移除                                     | 160 ms CSS + 200 ms `setTimeout` | `public/splash.css` / `splash.ts:51` |

→ **光动画就固定 320–480 ms**，且 `hideSplash()` 还要等两帧。所以即使后端和 JS 都是 0，遮罩从“可以撤”到“真的没了”也有 ~0.4 s。

---

## 3. 阶段表（实测，三次 run）

### 3.1 时间线（process-relative，ms）

| #   | 阶段                                                 | dev run 1   | dev run 2   | prod bundle    |
| --- | ---------------------------------------------------- | ----------- | ----------- | -------------- |
| 0   | spawn（`setsid ./target/debug/openclion <kaiwudb>`） | 0           | 0           | 0              |
| 1   | `AppState::new` 完成（`setup`）                      | 106         | 110         | 117            |
| 2   | webview navigation start（`performance.timeOrigin`） | 734         | 814         | 412            |
| 3   | 首屏 FCP                                             | 1128        | 1081        | 721            |
| 4   | `DOMContentLoaded`                                   | 2894        | 3059        | 936            |
| 5   | **第一条 IPC 抵达 Rust**（`startup: first IPC`）     | **2873**    | **3036**    | **841**        |
| 6   | `open_project` 返回（laps 见 §5.4）                  | +738 → 3611 | +876 → 3912 | +826 → 1667    |
| 7   | 项目树首次绘制 / **遮罩开始淡出**                    | **4072**    | **4155**    | **1827**       |
| 8   | **遮罩完全移除**                                     | **4449**    | **4376**    | **≈2.0–2.4 s** |

> 遮罩“可见”时长 ≈ 从 FCP（1.1 s / 0.7 s）到第 7 行：**dev ≈ 3.0 s 的假进度条，prod ≈ 1.1 s**；到第 8 行才是视觉上消失。
> 第 5 行早于第 4 行（first IPC < DCL）是正常的：第一条 IPC 在模块脚本执行期间就发出了，`DOMContentLoaded` 要等模块图执行完（含 top-level `await` 的收尾）。

### 3.2 阶段 → 耗时 → 它阻塞了什么

| 阶段                                      | 耗时（dev / prod）                                                                     | 阻塞了什么                                                      |
| ----------------------------------------- | -------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| 进程 exec → `AppState::new` 结束          | 106–120 ms（其中 `AppState::new` 自身 3.4–4.6 ms）                                     | 无用户可见物：窗口还不存在                                      |
| `setup` → webview navigation start        | +300~700 ms（106→734/814；117→412）                                                    | **splash 还不能绘制**；用户看到窗口的 `backgroundColor #E9EAEE` |
| navigation → `DOMContentLoaded`           | **2160–2245 ms**（dev，250 请求）/ **524 ms**（prod，87 请求）                         | **所有 app JS**：模块图不执行完，`main.tsx` 一行都跑不了        |
| DCL → first IPC                           | ~0–100 ms（`activateWorkbench()` + 42 个 `activate()` + React 首次 render 的同步部分） | 第一条后端调用（`windowTakeLaunch`）                            |
| `open_project`（roots+index+watcher+git） | **738–876 ms**（prod 826）                                                             | **遮罩 hold**（≤700 ms）+ 项目树内容                            |
| IPC 返回 → 项目树绘制 + 遮罩淡出开始      | ~160–330 ms                                                                            | 工作台可用（第 7 行）                                           |
| 淡出 160 ms + `setTimeout 200`            | 320–380 ms                                                                             | 仅视觉残留（第 8 行）                                           |

### 3.3 关键差值

| 对比                               | 差值                       | 归因                                                                                |
| ---------------------------------- | -------------------------- | ----------------------------------------------------------------------------------- |
| dev first IPC 2873/3036 → prod 841 | **−2.03 / −2.20 s**        | 233 个 `/src` 模块 + 12 个 Vite dep chunk 的按需 transform + HTTP + WebKit 解析执行 |
| dev 遮罩淡出 4072/4155 → prod 1827 | **−2.25 / −2.33 s**        | 同上（其余阶段两版一致）                                                            |
| dev DCL 2160–2245 → prod 524       | −1.6~−1.7 s                | 同上（DCL 前只剩模块图）                                                            |
| `open_project` 占总时长            | prod 826 / 1827 = **45 %** | watcher 688/826 = 83 % of open                                                      |

---

## 4. 前端侧：谁在阻塞首屏

### 4.1 首屏 ESM 图（dev）

Playwright 对 dev server 的冷加载统计（`target/perf-probe/urls.mjs`）：

```
total 250 requests
 /src/*                     233 requests   6517 KB   ← 应用自己的模块，逐个 HTTP 请求
 /node_modules/.vite/deps    12 requests   3338 KB   ← 预打包的第三方 dep chunk
 /@vite/client                2 requests    332 KB
 other                        3 requests      8 KB
feature dirs touched: 41  → （含 features/index.ts，即 40 个 feature 目录）
  agents,analysis,app,build,cmake,code-insight,coverage,
  database,debugger,dependencies,deployment,github,http-client,libraries,macros,
  notebook,perf,plugins,profiler,project-tree,project-trust,remote-dev,run,
  run-anything,sanitizers,scratch,search,services,settings,ssh,templates,
  terminal,todo,tools,valgrind,vcs,viewers,vim,web,welcome
```

原因很直接：`src/main.tsx:6` 静态 `import { activateFeatures, … } from "./features"`，`src/features/index.ts:7-49` 静态 import 了 **42 个 feature**（数组在 `:52-95`），每个 feature 又静态拉起自己的 store/组件/命令绑定。**没有任何动态边界**，所以 Vite 必须先把这 233 个模块全部 transform 并送达，`main.tsx` 才能执行第一行。

生产构建同一入口（`vite build --outDir target/perf-probe/dist-prod`）：

```
index.html 里 71 个 modulepreload + 1 个入口 script + 1 个 CSS = 87 请求
入口 index-*.js 2,180 KB（gzip 677 KB）；71 个 preload 合计 2,500 KB（gzip 641 KB）
→ 首屏 JS ≈ 1.32 MB gzip，而 perf/budgets.json 的 bundle.initialJsKb 是 850 KB（已超 ~55 %）
构建耗时 3.1 s；rolldown 警告若干 chunk > 500 KB（ui 1,066 KB、elk 1,462 KB、cytoscape 435 KB）
```

→ 生产是“84 个 chunk 一次 preload”，dev 是“233 个模块逐个请求 + 每次 Vite 现场 transform”。**这就是 2 s 罚时的来源**，不是 Rust。

### 4.2 模块级（module-level）重量级工作

| 项              | 现状                                                                                                                                                                 | 代价                                                                         |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| keymaps         | `src/platform/keymap.ts:16-21` **静态 import 6 个完整 preset**（intellij/eclipse/vscode × mac/windows，源文件 59 KB / gzip 17 KB）                                   | 6 个模块、17 KB gzip；只用 1 个                                              |
| locale          | `src/platform/i18n.ts:29` 静态 `import en from "./locales/en.json"`（21 KB）；`zh` 走 `import.meta.glob` 懒加载                                                      | 小，合理                                                                     |
| icons           | `src/ui/icons.tsx` 37 KB，一个模块级 `Record`                                                                                                                        | 小                                                                           |
| CodeMirror 语言 | `src/editor/core/cm/languages.ts:18` `import { languages } from "@codemirror/language-data"`：只带 _描述表_（name/alias/ext + 动态 `load()`），语法包按需 `import()` | 描述表小、语法包懒加载，**没问题**（审计担心的“全部 CM 语言”并未发生在首屏） |
| 其余懒加载      | `src/ui/lazy.tsx:46 prefetchLazyComponents()` 在 `hideSplash()` 之后用 `requestIdleCallback` 串行预取                                                                | 它会与“项目打开后首帧”抢主线程（见 F7）                                      |

### 4.3 是否等后端才渲染？

- **不等 `appInfo`**：`src/workbench/activate.ts:41` `void loadPlatformInfo()`（fire-and-forget）。
- **不等 index warmup**：`ProjectIndex::open` 只开 redb store + 起 worker 线程（`crates/ide-index/src/lib.rs:90-131`），扫描在 `openclion-index` 线程；`open_project` 返回时 index 还没扫完。
- **不等 LSP `initialize`**：`LspManager::new` 只构造结构体（`crates/ide-lsp/src/lib.rs:281`，`servers: Mutex::new(Vec::new())`），server 子进程在**首个匹配语言的文件被打开**时才起（`crates/ide-lsp/src/lib.rs:3-5` 的文档注释）。日志里 `clangd initialized` 在 `opened …` 之后 0.6 s（warm）到 102 s（冷）。
- **唯一 gate 就是 launch**（§2）。所以“render workbench before index/LSP ready”这件事**现状已经成立**；不成立的是“render workbench before _project open_”，因为 splash 拿 `openProject` 当 hold。
- 也没有显式的 `ready` state：`hideSplash()` 是唯一的“启动完成”信号；`useProject` 只有 `opening`（`src/platform/project.ts:53`，仅在 `openProject` 期间为 true）。

---

## 5. Rust 侧：逐项测量

### 5.1 启动到窗口（同步路径）

| 步骤                                                                                   | 位置                                                             | 实测                                                                     | 在关键路径？                  |
| -------------------------------------------------------------------------------------- | ---------------------------------------------------------------- | ------------------------------------------------------------------------ | ----------------------------- |
| `main` → `ide_ssh::askpass::run_helper()` 早退检查                                     | `src-tauri/src/main.rs:5-8`                                      | <1 ms                                                                    | 无                            |
| `generate_context!` + `dev::configure` + `logging::init`                               | `src-tauri/src/lib.rs:53-56`                                     | 含在下面 106 ms 里                                                       | 是（一次性 ~10 ms）           |
| `inherit_login_shell_path`（登录 shell 取 PATH）                                       | `src-tauri/src/lib.rs:58`；`crates/ide-host/src/shell_env.rs:23` | **0 ms**（有缓存；无缓存时 `$SHELL -lc` 本机实测 0.00 s，最坏 3 s 超时） | 是（首次启动有风险）          |
| `ide_tools::extend_process_path` / `set_tools_root` / `add_jdk_root`                   | `lib.rs:60-67`                                                   | 含在 106 ms 里（几次 `read_dir`）                                        | 是，但小                      |
| `specta_builder()` + **`export_bindings()`（debug 每次启动写 `src/bindings.ts`）**     | `lib.rs:69-71`                                                   | 含在 106 ms 里                                                           | 是（debug 专属；release 无）  |
| Tauri plugin 注册（single-instance/opener/dialog/window-state/updater/**mcp-bridge**） | `lib.rs:78-98`                                                   | 含在 106 ms 里                                                           | 是，小                        |
| `AppState::new`（**整体**）                                                            | `state.rs:52-115`，日志 `took`                                   | **3.4 / 3.5 / 3.6 / 4.3 / 4.6 ms**                                       | 是，可忽略                    |
| ├ grammar 线程 spawn                                                                   | `state.rs:57-66`                                                 | ~0.05 ms（只是 spawn）                                                   | 否                            |
| ├ `LanguageRegistry::load`（73 个 TOML）                                               | `state.rs:83`                                                    | **1.99 ms**                                                              | 是，可忽略                    |
| ├ `Host::new`（settings/trust/ssh/version cache/debugger/terminals/processes/proxy）   | `crates/ide-host/src/lib.rs:84-137`                              | 计入上面 3.4 ms                                                          | 是，可忽略                    |
| ├ `PluginHost::new` + `load_all()` 线程                                                | `src-tauri/src/commands/plugin.rs:39-51`                         | 构造 ~0；load_all 在 `plugin-load` 线程                                  | 否                            |
| ├ `ide_web::WebServer::new()`                                                          | `crates/ide-web/src/server.rs:53-59`                             | 只建 UUID + `OnceCell`，**首次 `url_for` 才 bind**                       | 否                            |
| ├ `AskpassServer::start`                                                               | `state.rs:150`                                                   | 计入 3.4 ms                                                              | 是，小                        |
| └ `RecentProjects::load`                                                               | `state.rs:103`                                                   | 计入 3.4 ms                                                              | 是，小                        |
| `menu::install`（原生 GTK 菜单）                                                       | `lib.rs:151`                                                     | 含在 setup→navigation 的 300–700 ms 内                                   | 是                            |
| `windows::create_main_window` + WebKit 初始化                                          | `lib.rs:162`；`windows.rs:354`                                   | setup(106) → navigation(412~814) = **+306~708 ms**                       | 是（splash 之前唯一的黑屏期） |

### 5.2 tree-sitter grammar（对审计 §7 #1 的更正）

**release（仓库自带 example，可直接复现）**：

```
$ cargo run --release -p ide-language --example startup_cost
LanguageRegistry::load 781.173µs
Syntax::with_languages 165.214µs
Syntax::new 908.326µs
fingerprint 41.856µs (20105ec52a87ea14)
```

**debug（`pnpm dev:app` 同 profile，临时探针）**：

```
languages=73
registry_load_ms=1.989        registry_load_again_ms=1.877
registry_fork_ms=0.615        set_file_types_ms=0.002
syntax_new_ms=2.309           syntax_new_again_ms=2.175
syntax_fingerprint_ms=0.051   syntax_cache_new_ms=0.001
grammar_count=37
rss_delta_mb=3.5
```

结论：

- `Syntax::new()` = 37 次 `Language::new()`（`tree-sitter-language` 的 `LanguageFn` 只是把静态表指针包一层，**不分配解析表**）+ **又一次 `LanguageRegistry::load(None)`**（`crates/ide-language/src/syntax/mod.rs:82`）→ **debug 2.3 ms / release 0.9 ms**，常驻 **3.5 MB**。
- 它**已经在后台线程**（`state.rs:57-66`，线程名 `openclion-syntax-init`），`Host::syntax()`（`crates/ide-host/src/lib.rs:130`）只在 `open_project` 里 `get_or_init`——按 2.3 ms 计，项目打开时它早就完成了。
- 审计说“35 个 grammar / 41 个语言”：实际 **37 个 grammar / 73 个 TOML**；“为每个分配解析表”也不成立。
- 唯一真实的浪费：**73 个 TOML 被解析了两遍**（`state.rs:83` 一次，`Syntax::new` 内再一次），合计 ~4 ms。可以顺手传 `&languages.all()` 进 `Syntax::with_languages`（3 行改动，~2 ms）。

### 5.3 `Host::open_project` 的各 lap（来自真实日志，全部样本）

| 来源                                   | 总       | roots   | index | watcher  | git |
| -------------------------------------- | -------- | ------- | ----- | -------- | --- |
| 11-02 03:24 KaiwuDB                    | 592      | 140     | 5     | 447      | 0   |
| 11-02 05:32 KaiwuDB                    | 972      | 135     | 242   | 594      | 0   |
| 11-02 05:33 kwbasedb                   | 1525     | **712** | 7     | 805      | 0   |
| 11-02 07:27 kwbasedb #1                | 1153     | 54      | 2     | 1096     | 0   |
| 11-02 07:27 kwbasedb #2                | **3404** | 204     | 519   | **2680** | 0   |
| 11-03 00:51 kwbasedb                   | 1310     | 138     | 444   | 727      | 0   |
| 11-03 01:13 kwbasedb                   | 794      | 52      | 39    | 702      | 0   |
| 11-03 09:51（本报告）                  | 843      | 66      | 34    | 742      | 0   |
| 11-03 09:52 run1                       | 738      | 59      | 43    | 635      | 0   |
| 11-03 09:52 run2                       | 876      | 65      | 38    | 771      | 0   |
| 11-03 09:55 prod                       | 826      | 79      | 58    | 688      | 0   |
| 11-03 09:57（strace，全 syscall 放大） | 5191     | 940     | 274   | **3976** | 0   |

**watcher 稳定占 80–90 %**。`git` lap 恒为 0 是因为 `scan_repositories` 在 `openclion-vcs-roots` 线程（`project.rs:275-279`）；`Repos::enclosing` → `Repo::discover` 是 ms 级。`roots` lap = `SettingsStore::load`（global+project）+ `Workspace::load` + `roots::detect` + `exclude_globs`，13–712 ms。

### 5.4 watcher 为什么贵：Linux per-directory + 全树 walk

`crates/ide-vfs/src/watch.rs`：

- `WatchMode::default()`（`:39-44`）= Linux `PerDirectory`，其它平台 `Recursive`。
- `start_per_directory`（`:152`）在**返回前同步**调用 `state.watch_tree(&root, false, &mut created)`（`:182`）→ 递归 `readdir` + 每个目录一次 `inotify_add_watch`。
- 实测 kaiwudb：**4,181 个 inotify watch**（`/proc/<pid>/fdinfo/54` 里 `inotify wd:` 计数），根目录下 18,522 个目录（不含 `.git`，含 `.git` 19,722）——excludes 已经滤掉了 `build*`、`install` 之类的大头，剩下的仍要 4,181 次 watch。
- 685 ms / 4,181 ≈ **164 µs/目录**（walk + stat + `inotify_add_watch`）；冷缓存或并发 IO 时退化到 2.7–4.0 s（07:27 那次 2680 ms、strace 那次 3976 ms）。
- `start_recursive` 的注释已经承认同类问题（`:130-131`：“building it walks and stats the whole tree before `watch` returns, which made opening large projects block for seconds”）——但 Linux 的 per-directory 路径仍保留了同步 walk。
- 上限：`fs.inotify.max_user_watches = 65536`，4,181 远未触顶，加大 watch 数不是问题，**walk 的同步性是问题**。

### 5.5 index / LSP / 其它后台项（不挡 bar，但影响“用起来慢”）

| 项                                                       | 位置                                                          | 实测                                                                                                                         | 是否挡启动                              |
| -------------------------------------------------------- | ------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- | --------------------------------------- |
| `ProjectIndex::open`（redb store + worker 线程）         | `crates/ide-index/src/lib.rs:90-131`                          | 2–519 ms（冷 444/519，warm 2–58）                                                                                            | 是（在 `open_project` 的 index lap 内） |
| index 全量扫描 + tree-sitter parse                       | `crates/ide-index/src/worker.rs`；日志 `indexed …`            | **warm 328–376 ms（0 parsed，45,952 files / 519,688 symbols）；冷 12.1 s（18,759 文件全 parse）/ 17.5 s（45,950 全 parse）** | 否（后台线程），但抢 CPU                |
| `LspManager::new`                                        | `crates/ide-lsp/src/lib.rs:281`                               | ~0（不 spawn 进程）                                                                                                          | 否                                      |
| clangd `initialize`（`--background-index --clang-tidy`） | `languages/c.toml:13`；日志 `clangd initialized`              | **warm 0.6 s；冷 102 s**（11-02 03:24:25 open → 03:26:07 initialized）                                                       | 否（首次打开 C/C++ 文件才起）           |
| tool probing（`--version` × ~50）                        | `crates/ide-tools/src/lib.rs:703-728`                         | 未在启动时发生（只有 Settings › Language Servers 的 `tools_list`）                                                           | **否**                                  |
| agent registry 维护线程                                  | `crates/ide-agent/src/registry.rs:637-659`                    | 首个项目打开时 spawn；仅当装了 registry agent 且 cache 过期才 HTTPS                                                          | 否                                      |
| dep 版本检查                                             | `crates/ide-host/src/ops/dependencies.rs`                     | 打开 Cargo.toml 后 500 ms 防抖                                                                                               | 否                                      |
| git 仓库扫描                                             | `crates/ide-host/src/project.rs:275`（`openclion-vcs-roots`） | 后台线程（lap `git`=0）                                                                                                      | 否                                      |
| plugin `load_all`                                        | `src-tauri/src/commands/plugin.rs:50`                         | `plugin-load` 线程                                                                                                           | 否                                      |
| 内建 Web server bind                                     | `crates/ide-web/src/server.rs:53`                             | 首次 `url_for` 才 bind                                                                                                       | 否                                      |
| 本地历史 prune                                           | `crates/ide-host/src/project.rs:103`                          | 线程                                                                                                                         | 否                                      |

---

## 6. 排序修复清单

> “win” 指**进程启动 → 遮罩开始淡出**的减少量，除非另注。基准：dev 4.07–4.16 s，prod 1.83 s。

| #       | 修复                                                                                                                                                                                                                                                                             | 预期收益                                                                                      | 风险                                                                                                                                                                 | 精确改动点                                                                                                                          |
| ------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| **F1**  | **拆开 feature 的 import / activate 链**：让 `Workbench` shell 先用小图渲染，42 个 feature 通过 `import()` 分批激活（至少把 database / notebook / services / github / http-client / deployment / agents / coverage / profiler / valgrind / sanitizers / web 这 12 个移出阻塞图） | **dev −1.3~1.5 s**（first IPC 2.14–2.22 s → ~0.7 s）；**prod −0.15~0.3 s**                    | 中：`features/index.ts:3-6` 的注释明确写了顺序约束（editor 提供 editor service、app 必须在 welcome 之前装 launch barrier）。需要先落一个“features 未就绪”的 shell 态 | `src/main.tsx:6,26,39,43`；`src/features/index.ts:52-95,97-109`；各 feature 的 `activate()`                                         |
| **F2**  | **splash 完全不等 `open_project`**：`holdSplash(launch)` 改成不 hold（或 250 ms），把“项目已开”作为工作台自己的加载态；`openProject` 失败仍走通知                                                                                                                                | **−0.7~0.9 s**（warm：open 738–876 ms vs 现行 700 ms 上限；**HEAD 无上限时冷开 −2.7~3.4 s**） | 低：工作区已做了一半（700 ms）；只需再确认“空工作台 → 树出现”的无闪烁                                                                                                | `src/platform/splash.ts:34-36,42-44`；`src/features/app/windows.ts:453`；`src-tauri/src/commands/app.rs:48`                         |
| **F3**  | **`Watcher::start` 增量化**：先只 `inotify_add_watch(root)` 并立刻返回，`watch_tree` 挪进 `openclion-fs-watch` 线程继续装（期间的事件由一次全量 rescan 兜底）；或先把 walk 用 8 线程并行 readdir，再串行 `inotify_add_watch`（`inotify` fd 本身是串行的）                        | **−0.6~0.8 s / 每次 open**（4,181 watches × 164 µs）；冷/负载下 **−2.7~4 s**                  | 中：装 watch 的时间窗内会漏事件，需要“装完做一次 rescan/对账”；excludes 语义不能变                                                                                   | `crates/ide-vfs/src/watch.rs:152-193`（`start_per_directory`）、`:325`（`watch_tree`）；调用点 `crates/ide-host/src/project.rs:184` |
| **F4**  | **`dev:app` 改用 `vite build --watch` 产物**（或给 tauri dev 一个 `--prod-frontend` 开关）：dev 也走 84 个 preload chunk，而不是 233 个按需 transform 的模块                                                                                                                     | **dev −2.0~2.3 s**（实测 4.07/4.16 s → 1.83 s）                                               | 低：只影响开发者体验；失去 HMR（可配 `build --watch` + 自动 reload 缓解），Rust 侧重建不受影响                                                                       | `scripts/tauri.ts:main()`（`withDevConfig(args, session.devUrl)`）、`package.json` 的 `dev:app`                                     |
| **F5**  | **index 首扫限速/限域**：首次全量 parse 45,950 文件 17.5 s，明显与首屏/打开后的交互抢 CPU；给 worker 加并发上限 + 让 `index_roots`/excludes 排除 `install/`、`qa/`、`tsbs-*` 这类非源码树（或首次只索引打开文件所在子树，其余惰性）                                              | 不缩短 bar，但**首次打开后 12–17 s 的卡顿**显著下降；CPU 占用可控                             | 中：影响 find-usages / 全局符号的完整性                                                                                                                              | `crates/ide-index/src/worker.rs`（`run` 的批处理/并发）；`crates/ide-host/src/project.rs:824 index_roots`、`:65 exclude_globs`      |
| **F6**  | **clangd 默认参数**：`--background-index` + `--clang-tidy` 在 46k 文件的 C++ 仓上让首次 `initialize` 花 102 s；改成按项目大小/设置开关（默认后台索引关闭或延迟到用户显式请求）                                                                                                   | 不缩短 bar，但**首次打开 C/C++ 文件后 100 s 的“索引中/不可用”**消失                           | 低-中：牺牲首次跳转速度；建议做成 `clangd.backgroundIndex` 设置，默认 off on huge repos                                                                              | `languages/c.toml:13`、`languages/cpp.toml`、`languages/objective-c*.toml`                                                          |
| **F7**  | **`prefetchLazyComponents()` 等 launch 落定再跑**：现在 `main.tsx:48` 在 `hideSplash()` 后立刻 `requestIdleCallback` 串行预取，正好和“项目打开后的首次树渲染”抢主线程                                                                                                            | −50~150 ms 的主线程争用（dev 更明显）                                                         | 低                                                                                                                                                                   | `src/main.tsx:48`；用 `src/platform/project.ts:47 startupLaunchHandled()`                                                           |
| **F8**  | **砍掉 splash 的固定动画开销**：`completeBar` 的 120 ms + `setTimeout(remove, 200)`；可合并成 1 帧 + 100 ms 淡出，或在等待 < 150 ms 时直接移除                                                                                                                                   | −0.2~0.3 s（纯感知，UI 实际已可点）                                                           | 低                                                                                                                                                                   | `src/platform/splash.ts:16,19-28,48-54`                                                                                             |
| **F9**  | **6 个 keymap preset 懒加载**：只静态 import 当前 OS 的 IDEA preset，其余 4 个 `import()`                                                                                                                                                                                        | dev −6 个请求 / ~30–60 ms；包体 −17 KB gzip                                                   | 低                                                                                                                                                                   | `src/platform/keymap.ts:16-21,35-43`（`PRESETS` 改成 getter/异步解析；`setKeymapPreset` 处 await `import("./keymaps/…")`）          |
| **F10** | **`LanguageRegistry::load` 去重**：`state.rs:83` 已加载，`Syntax::new()`（`syntax/mod.rs:82`）又加载一次                                                                                                                                                                         | −2 ms（debug）                                                                                | 低                                                                                                                                                                   | `crates/ide-language/src/syntax/mod.rs:81-84` 改为 `with_languages(&registry.all())`；`state.rs:61` 把 `languages` 传进线程         |
| **F11** | （可选，收益小）首屏 bundle 超预算：`perf/budgets.json` 的 `bundle.initialJsKb: 850`（gzip），实测入口 677 + preload 641 = **≈1,320 KB gzip**                                                                                                                                    | 影响 F1 的 prod 收益上限                                                                      | 中：需要真正的 code-splitting 审计                                                                                                                                   | `src/features/index.ts`、`src/ui/*`、`vite.config.ts`                                                                               |

**为什么是这个排序**：F1 与 F4 解决同一个 ~2 s 的 dev 罚时（F4 是零风险的“立刻见效”，F1 才是产品级修复）；F2/F3 解决剩下 1.8 s 里的 0.7–0.9 s，并在大仓/冷缓存上放大到数秒；F5/F6 不缩短 bar，但直接命中“kaiwudb 第一次打开很慢”的主观感受。

---

## 7. ≤30 行的 quick wins

按“改动行数 / 风险 / 收益”排序，每条都能独立提交：

1. **`splash.ts`：launch hold 归零**（≈8 行，配合已存在的 `LAUNCH_WAIT_MS`）
   `src/features/app/windows.ts:453` 的 `holdSplash(launch, LAUNCH_WAIT_MS)` → 直接删掉这行（或 `holdSplash(launch, 150)`），`launch` 仍由 `provideStartupLaunch` 管。**−0.7~0.9 s**。
2. **`main.tsx`：prefetch 延后**（4 行）
   `if (settingsParent === null) prefetchLazyComponents();` → `void startupLaunchHandled().then(() => prefetchLazyComponents());`。−50~150 ms 的主线程争用。
3. **`splash.ts`：去掉 120 ms 补条**（5 行）
   `completeBar` 返回 false（或 `FILL_MS = 0`），并把 `setTimeout(remove, 200)` 降成 120。−0.2 s 感知。
4. **`languages/c.toml:13`：`--background-index` 改配置**（1 行 + 设置项）
   先直接删掉验证：kaiwudb 冷开 clangd init 102 s → ~1 s。要做成 `clangd.backgroundIndex` 设置则 ~25 行。
5. **`LanguageRegistry` 复用**（3 行）
   `Syntax::new()` → `Syntax::with_languages(&languages.all())`，并让 `state.rs:61` 的线程捕获 `Arc<LanguageRegistry>`。−2 ms，但顺手消除了“解析两遍 73 个 TOML”的困惑。
6. **keymap 懒加载**（≈15 行）
   `PRESETS` 里 4 个非当前 OS 的 preset 改成 `() => import(...)`；`setKeymapPreset` 里 await。dev −6 请求 / −17 KB gzip。
7. **`dev:app` 用 build 产物**（≈25 行，脚本层）
   `scripts/tauri.ts` 增加 `OPENCLION_DEV_BUNDLE=1` 分支：先 `vite build --outDir target/dev-dist`，再用 `sirv`/`http.server` 起在 `session.devUrl`。**dev −2.0 s**（不需要改任何产品代码）。

---

## 8. 复现与原始数据

### 8.1 复现命令

```bash
# 1) dev server（端口 = preferredVitePort(worktree identifier)，本例 22014）
node node_modules/vite/bin/vite.js --port 22014 --strictPort &

# 2) 启动真实 app（debug binary 里嵌的就是 http://localhost:22014）
setsid ./target/debug/openclion /home/limstorm/gowork/src/gitee.com/kwbasedb >/tmp/app.log 2>&1 &

# 3) 用 MCP bridge 抓页面时间线（WS 9223）
node target/perf-probe/app-probe.mjs --label run1

# 4) A/B：换成生产 bundle（同一个 binary / 同一个 webview）
node node_modules/vite/bin/vite.js build --outDir target/perf-probe/dist-prod --emptyOutDir
kill <vite-pid>; python3 -m http.server 22014 --directory target/perf-probe/dist-prod &
setsid ./target/debug/openclion /home/limstorm/gowork/src/gitee.com/kwbasedb >/tmp/app-prod.log 2>&1 &
node target/perf-probe/app-probe.mjs --label prod1

# 5) Rust 微基准（仓库自带 example；§5.2 的 debug 数字用它的临时变体测得）
cargo run --release -p ide-language --example startup_cost

# 6) dev 模块图分类（Chromium）
node target/perf-probe/urls.mjs

# 7) inotify watch 计数
for fd in /proc/<pid>/fd/*; do case "$(readlink $fd)" in anon_inode:inotify*)
  echo "$fd $(grep -c '^inotify wd:' /proc/<pid>/fdinfo/$(basename $fd))";; esac; done
```

### 8.2 关键日志原文（`~/.local/share/ai.l0ng.openclion.dev.wb879b047d6acc5a6/logs/`）

```
# dev run 1（09:52:26Z）
startup: first IPC 2873 ms after process start (main 1 ms, setup at 106 ms, took 3.4 ms)
opened /home/limstorm/gowork/src/gitee.com/kwbasedb in window main in 738 ms [("roots", 59), ("index", 43), ("watcher", 635), ("git", 0)]
indexed 45952 files (0 parsed, 519688 symbols) in 351.570784ms

# dev run 2（09:52:55Z）
startup: first IPC 3036 ms after process start (main 2 ms, setup at 110 ms, took 3.5 ms)
opened … in 876 ms [("roots", 65), ("index", 38), ("watcher", 771), ("git", 0)]

# prod bundle（09:55:09Z）
startup: first IPC 841 ms after process start (main 2 ms, setup at 117 ms, took 3.6 ms)
opened … in 826 ms [("roots", 79), ("index", 58), ("watcher", 688), ("git", 0)]

# 冷开/首次索引的历史样本
startup: first IPC 15623 ms after process start (main 9769 ms, setup at 9878 ms, took 3.8 ms)   ← process_to_main 是坏值，见 §1.3
opened … in 3404 ms [("roots", 204), ("index", 519), ("watcher", 2680), ("git", 0)]
indexed 45950 files (45950 parsed, 519688 symbols) in 17.45982533s
clangd initialized: … (open 03:24:25 → init 03:26:07，102 s)
```

### 8.3 探针产物（`target/` 被 .gitignore，不入库）

| 文件                                                                   | 内容                                                                                                                  |
| ---------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| `target/perf-probe/app-run1.json` / `app-run2.json` / `app-prod1.json` | 每 100 ms 的 `{splash, splashOut, tree, nodes, page performance.now()}` + `paint/navigation/resource` 汇总 + IPC 事件 |
| `target/perf-probe/boot-probe.json`                                    | Chromium 对 dev server 的 boot（FCP/DCL/long task/self-time top-40）                                                  |
| `target/perf-probe/urls.mjs`                                           | 250 请求按 `/src` / `.vite/deps` 分类                                                                                 |
| `target/perf-probe/dist-prod/`                                         | A/B 用的生产构建（87 请求 / preload 2.5 MB）                                                                          |

### 8.4 已删除的临时文件

- 一个临时的 `crates/ide-language/examples/startup_probe.rs`（untracked，测完 `rm`；`target/debug/examples/startup_probe*` 是构建残留，在 ignored 的 `target/` 内）。仓库自带的 `crates/ide-language/examples/startup_cost.rs` 未改动，可复现 §5.2 的 release 数字。
- 除本文件外，本次调查**没有修改任何 tracked 文件**。

---

## 9. 一句话回答三个原始问题

1. **“loading bar 为什么长？”** —— 它是静态 splash 假进度条，等的是 `windowTakeLaunch → openProject`；`pnpm dev:app` 下 4.4 s 里有 ~2.0 s 是 dev ESM 图（250 请求 / 233 模块）、0.74–0.88 s 是 `open_project`（其中 watcher 635–771 ms）、其余是 WebKit 起窗 + 渲染 + 320–480 ms 固定淡出动画。
2. **“35 个 grammar 是元凶吗？”** —— 不是。`Syntax::new()` 实测 2.3 ms / 3.5 MB，且在后台线程；审计 §7 #1 已过时。
3. **“kaiwudb 第一次打开为什么更慢？”** —— 与 bar 无关的后台开销：index 首次全量 parse 45,950 文件 17.5 s（CPU），clangd `--background-index` 首次 init 102 s，加上 watcher 要建 4,181 个 inotify watch（冷缓存时 2.7 s）。
