# 编辑器滚动 / 交互性能调查 — Editor scroll & interaction performance

> 范围：OpenCLion 编辑器（Tauri 2 + React 19 + CodeMirror 6），C/C++ 大文件打开后滚动卡顿、普通交互偶发掉帧。
> 只读调查（read-only investigation）；代码未改动，仅本文件为新增产物。
> 结论按"证据强度"排序：先给实测数字，再给 `file:line`，最后给修复清单。

---

## 0. TL;DR（10 行）

1. **稳态滚动本身不慢**：20k 行 C++（623 KB）在浏览器基准里 60 fps（`frameP95 16.8 ms`，0 长任务），每帧主线程只有约 `1.2 ms script + 1.2 ms style/layout`。
2. **慢机器 / 大文件会塌**：同样文件在 4× CPU 下 `frameP95 33.3 ms`、掉帧 **12.4%**；一次"整屏"快速滚动（fling）掉帧 **35%**、`frameP95 50 ms`、9 个长任务。对照组的纯文本文件**更大**（1 591 KB，2.5 倍体积）在 4× 下仍是 `16.7 ms / 0 掉帧` → 差异来自 **C++ 语法解析/高亮 + CM measure**，不是文件体积本身。
3. **打开后第一次滚动有明显长任务**：3 个 `~100 ms` 长任务（`TBT 120 ms`、最大帧 `116 ms`、掉帧 5.8%），全部来自 CodeMirror 后台 Lezer 解析（`requestIdleCallback → work`）。
4. **最大的"与文件大小成正比、每帧都跑"的热点是右侧 overview lane**（`src/editor/core/cm/lanes.ts`）：每次视口变化遍历**全部** `scrollbarMarks`（诊断 + VCS + 搜索），**每个标记 2 次 `lineBlockAt`**。微基准：4 000 个标记 **7.4–9.1 ms/次**（4× CPU 下 28.7 ms），20 000 个 **14.6 ms/次**（4× 下 61 ms）。大 C++ 工程（clangd 诊断上千）必然掉帧。
5. 同一个 lane 的 `write()` 把 **caret 位置写进 DOM 重建的 key**，所以**每次光标移动都会重建最多 4 000 个 DOM 节点**。
6. **滚动停止后每个 LSP 插件各自刷新**：inlay 1 次（400 ms 防抖）、codeLens **最多 40 次 references**（900 ms 防抖）、lineMarkers **最多 60 次 implementations**（500 ms 防抖）；每次 `lspRequest` 还要先 `await whenAllSynced()`（多一次 IPC）。大工程里这是滚动后的第二波卡顿。
7. **光标移动路径**每键 `5–8 ms` script（4× 下 **18.7 ms/键**，`p95 24 ms`）：CM 坐标测量 + Lezer 增量解析 + 4 个 React 组件重渲染（`CaretStatus`、`Breadcrumbs`、上下两个 `CodeBreadcrumbs`）+ **每次 keydown 重建整张 keymap 绑定表**。
8. **C/C++ 高亮不走 IPC**：`ui-mode = "C++"` → `@codemirror/lang-cpp`（Lezer，页面内）。tree-sitter 的 `doc_highlights` 只在"没有 CodeMirror mode"的语言上调用（`document.ts:511`）。
9. **语义高亮（clangd semantic tokens）不是滚动问题**：全文件 `Decoration.set` 40k 个区间 **2.1 ms**、`map` **0.1 ms**（4× 下 10.9 / 0.9）→ 降级为低优先级。
10. **`contain` / `content-visibility` CSS 实测无收益**（±5% 噪声内）——不要为此改 CSS。

---

## 1. 测量方法（Measurement setup）

| 项       | 值                                                                                                                                                                                                             |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 应用构建 | `vite build --mode perf`（**生产包**，`--minify false` 以保留函数名），`target/perf/dist-unminified`                                                                                                           |
| 运行环境 | Chromium（Playwright headless），`1400×900 @1x`，Linux；CPU 1× 与 4×（`Emulation.setCPUThrottlingRate`）                                                                                                       |
| 内存后端 | 浏览器模式 `createMemoryBackend`（`src/editor/core/backend.ts:542`），`seed()` 注入文件，`__openclionPerf.openFile()` 打开                                                                                     |
| 滚动脚本 | `page.mouse.wheel(0, ±120)` × 80 下 + 80 上，步进 16 ms（≈ 7 200 px/s，触控板快速滑动）；fling = `±600 px` × 60                                                                                                |
| 采集     | `PerformanceObserver(longtask)`、`requestAnimationFrame` 帧间隔、CDP `Performance.getMetrics`（Script/Layout/RecalcStyle/Task）、CDP `Profiler`（self time）、CDP `Tracing`（devtools.timeline，主线程 split） |
| 文件     | `Scroll.cpp` 623 KB / 20 000 行 / 可见 **36 行**（`.cm-content` 215 个节点）；对照 `Scroll.ts` 686 KB / 20 010 行 / 369 节点；`Scroll.txt` 1 591 KB / 20 000 行 / 37 节点                                      |

**重要局限（必须知道）**：浏览器模式 `isTauri() === false`，`lspRequest()` 立即抛 `unavailable`，所以 **LSP / IPC 相关的每视口开销不在下面的数字里**。这部分用"代码路径 + 精确复刻的微基准"给出，第 4.3 节单独说明。Tauri 窗口 + clangd 的端到端测量未做（需要 `cargo build` + 真实 C++ 工程 + clangd，超出本次只读调查的时间预算）。

---

## 2. 热点地图（Hot-path map：触发条件 → 代码位置 → 每帧工作量）

### 2.1 滚动 / 视口变化（scroll & viewport）

| 触发                                                                     | 位置                                                           | 每次视口变化做什么                                                                                          | 复杂度                                                      |
| ------------------------------------------------------------------------ | -------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| `viewportChanged` / `selectionSet` / `geometryChanged` / `heightChanged` | `src/editor/core/cm/lanes.ts:54-64`                            | 调度 `requestMeasure` → `read()`                                                                            | —                                                           |
| 同上                                                                     | `src/editor/core/cm/lanes.ts:69-106`                           | **遍历全部 `scrollbarMarks`**，每个 2× `lineBlockAt` + `Map` + `sort`                                       | **O(marks)** ❗                                             |
| 同上                                                                     | `src/editor/core/cm/lanes.ts:107-131`                          | 用 `rows.map(...).join("                                                                                    | ")`拼 key，**key 含 caret** → 每次光标移动`replaceChildren` | **O(marks)** ❗ |
| `scroll`（原生监听）+ `viewportChanged` / `geometryChanged`              | `src/editor/core/cm/stickyScroll.ts:116, 120-131`              | 每帧 `requestMeasure`；`read()` 里 2× `querySelector`、2× `getBoundingClientRect`、≤5× `lineBlockAtHeight`  | O(sticky lines)                                             |
| `viewportChanged`                                                        | `src/editor/core/cm/indentGuides.ts:72-84`                     | `visibleRanges` 内逐行重建 indent guide                                                                     | O(可见行) ✅                                                |
| `viewportChanged`                                                        | `src/editor/core/cm/rainbowBrackets.ts:292-303`                | 重建视口括号装饰（**默认关闭**，`editorSettings.ts:74`）                                                    | O(可见行)                                                   |
| `viewportChanged`                                                        | `src/editor/core/cm/prefs.ts:87-117`                           | `wrapIndent`：软换行时逐行重建（`.cpp` 默认不换行，`softWrapFiles: "*.md;*.txt;…"`）                        | O(可见行)                                                   |
| 文档变化                                                                 | `src/editor/core/model/document.ts:502-537`                    | 450 ms 防抖后拉整文件 `folding` / `outline` / `highlights`                                                  | O(file)，非滚动                                             |
| `docChanged`                                                             | `src/editor/lsp/semantic.ts:147, 157-168`                      | 600 ms 防抖后**全文件**语义 token 装饰                                                                      | O(tokens)                                                   |
| `viewportChanged`                                                        | `src/editor/lsp/inlay.ts:180-182`                              | 400 ms 防抖 → **IPC** `lsp_inlay_hints(visible range)`                                                      | 1 IPC / 滚动停止                                            |
| `viewportChanged`                                                        | `src/editor/lsp/codeLens.ts:138-151`                           | `cancel()`（对每个在途请求发 `cancelRequest` IPC）+ 900 ms 防抖 → `docOutline` + **≤40 次 `lspReferences`** | ≤40 IPC ❗                                                  |
| `viewportChanged`                                                        | `src/editor/navigation/implementationGutter.ts:250-252, 50-51` | 500 ms 防抖 → **≤60 个** 可见符号的 `lspImplementations` / `typeHierarchy`                                  | ≤60 IPC ❗                                                  |

### 2.2 高亮（highlighting：C/C++ 到底走哪条路）

- **C/C++ = Lezer，页面内、无 IPC**：`languages/cpp.toml` 的 `ui-mode = "C++"` → `findLanguageDescription()`（`src/editor/core/cm/languages.ts:168-182`）→ `@codemirror/language-data` 的 `C++`（`@codemirror/lang-cpp`）。懒加载一次后缓存（`loadSupport`，`languages.ts:185-199`）。
- **tree-sitter 不参与 C/C++ 高亮**：`document.ts:511` 只在 `this.desc` 为空（没有 CodeMirror mode）时才调 `backend.highlights()`；`syntaxHighlight.ts:43-57` 把后端 tree-sitter 结果整文件转成 `DecorationSet`。`crates/ide-syntax/src/cpp.rs` 的 tree-sitter 用于后端 outline / folding / 结构服务，不走前端高亮。
- **LSP 语义高亮**：`semantic.ts` —— 整文件 decorations，`Prec.highest`（`semantic.ts:203-212`），默认 `semanticHighlighting: false`（`editorSettings.ts:72`）。
- **高亮的真实代价**：`@codemirror/language` 的 `treeHighlighter` 在每个视口更新时 `ensureSyntaxTree(viewport.to, 50ms)`，未完成则用 `requestIdleCallback` 续解析（`requestIdle`，`{timeout: 400}`）→ **这就是第 3 节里 3 个 100 ms 长任务的来源**。

### 2.3 光标 / 选区 / React（cursor → IPC → re-render）

| 触发                       | 位置                                                                                                                          | 开销                                                                                                                   |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| 每次 keydown               | `src/platform/keymap.ts:197-201, 508`                                                                                         | `effectiveBindings()` **重建整张 keymap**：spread 整个 preset + 对每条绑定 `normalizeBinding()`（trim/split/map/join） |
| `selection` / `docChanged` | `src/editor/core/model/slot.ts:184-208`                                                                                       | `host.onSelection()` → 状态栏                                                                                          |
| 光标移动                   | `src/editor/core/controller.ts:572-584`                                                                                       | `publishStatus`（新对象）→ `useEditorStatus` 订阅者全部重渲染                                                          |
| 同上                       | `src/editor/core/controller.ts:740-754`                                                                                       | `scheduleStatus`：rAF + 100 ms 兜底（节流得当 ✅）                                                                     |
| 同上                       | `src/editor/core/controller.ts:410-414` → `417-434` → `session.ts:215-217`                                                    | **400 ms 后把整个 session `JSON.stringify` 写 localStorage**                                                           |
| 重渲染组件                 | `StatusItems.tsx:57`（Breadcrumbs）、`144`（CodeBreadcrumbs，**上下各一个**，`EditorArea.tsx:406/419`）、`195`（CaretStatus） | 每个都跑 `useCaretSymbols`（`StatusItems.tsx:34-42`）→ `symbolChain()` 线性扫描整个 outline（`outline.ts:6-16`）       |
| `selectionSet`             | `src/editor/lsp/intentions.ts:574, 578-590`                                                                                   | 500 ms 防抖 → `lspCodeActions` IPC（灯泡）                                                                             |
| `selectionSet`             | `src/editor/lsp/navigation.ts:671, 673-700`                                                                                   | 300 ms 防抖 → `documentHighlight` IPC                                                                                  |
| `docChanged`               | `src/editor/lsp/spelling.ts:246-248`                                                                                          | 防抖后**整文件** `spell_check` IPC                                                                                     |
| `docChanged`（打字停顿）   | `src/editor/lsp/inlineCompletion.ts:194-215`                                                                                  | `TYPING_DELAY` 后 `inlineCompletion` IPC                                                                               |

---

## 3. 实测数据（Trace numbers）

### 3.1 稳态滚动（160 × 120 px 往返，wall ≈ 5.5 s，CPU 1×）

| 文件           | 帧数    | frameP95    | 掉帧   | 长任务      | script     | style+layout | layouts | style recalcs | task / wall        |
| -------------- | ------- | ----------- | ------ | ----------- | ---------- | ------------ | ------- | ------------- | ------------------ |
| **cpp** 623 KB | 332–343 | **16.8 ms** | 0–5.8% | 0–3（首次） | 171–210 ms | 173–196 ms   | 162     | 495–509       | 727–824 / 5 549 ms |
| ts 686 KB      | 339     | 16.7 ms     | 0%     | 0           | 188 ms     | 241 ms       | 178     | 519           | 1 012 / 5 669 ms   |
| txt 1 591 KB   | 338     | 16.8 ms     | 0%     | 0           | 144 ms     | 96 ms        | **30**  | 369           | 770 / 5 641 ms     |

每帧 ≈ **1.2 ms script + 1.2 ms style/layout**，其余是空闲 → 桌面机上"能跑满 60 fps"。

### 3.2 同一脚本，CPU 4×（慢机器 / 低功耗模式）

| 场景                       | 帧数 | frameP95    | 掉帧      | 长任务          | script | style+layout | task             |
| -------------------------- | ---- | ----------- | --------- | --------------- | ------ | ------------ | ---------------- |
| **cpp 平滑**               | 340  | **33.3 ms** | **12.4%** | 3（max 163 ms） | 756 ms | 768 ms       | 3 090 ms / 6 493 |
| **cpp fling**（600 px/帧） | 143  | **50.0 ms** | **35.0%** | 9（max 163 ms） | 823 ms | 1 128 ms     | 2 922 / 3 688    |
| txt 平滑                   | 333  | **16.7 ms** | **0%**    | 0               | 307 ms | 226 ms       | 1 160 / 5 566    |

**这是最有说服力的一组对照**：同样的滚动脚本、同样 36 行可见，纯文本 4× 下仍然零掉帧，C++ 掉到 30 fps（fling 20 fps）。差异只能来自 C++ 的语法树工作 + 由此增加的 DOM/measure。

### 3.3 打开后"第一次滚动"（traced，CPU 1×，cpp）

```
长任务 ×3：101 ms / 100 ms / 90 ms
  └─ FireIdleCallback → FunctionCall "work"  (@codemirror/language 后台解析)
最大帧 116.6 ms，掉帧 4.3–5.8%，TBT 120–125 ms
主线程 top events（wall 5.99 s，嵌套关系：RunTask 包含其余）：RunTask 1764 ms · FunctionCall 860 ms · Commit 232 ms
                             · Paint 184 ms · UpdateLayoutTree 170 ms · Layout 95 ms
                             · Layerize 69 ms · PrePaint 56 ms · GC 24 ms
```

未加 trace 的同场景同样出现 `100 / 100 / 66 ms` 三个长任务 → **不是 trace 开销**。

### 3.4 光标移动（↓↓ 24–40 次，16 ms 间隔）

| CPU | p50        | p95         | script / 键 | style+layout          | 长任务     |
| --- | ---------- | ----------- | ----------- | --------------------- | ---------- |
| 1×  | 7.1–9.1 ms | 14–16 ms    | **4–11 ms** | 25–56 ms（24 键合计） | 0          |
| 4×  | 13.6 ms    | **24.0 ms** | **18.7 ms** | 106 ms                | 1（50 ms） |

4× 下每按一次 ↓ 就超过一个 16.7 ms 帧 → "按方向键跟不上手"的体感。

### 3.5 主线程 self time Top（滚动，CPU 1×，cpp）

```
 84.9–122.5 ms  measure                  @codemirror/view   ← CM 的 measure 周期
 35.7– 52.8 ms  getBoundingClientRect    (native)           ← measure / sticky / lane
 32.2– 85.2 ms  (garbage collector)      (native)           ← 装饰/DOM 抖动
 30.5 ms        nextChild                @lezer/common      ← C++ 语法树游标
  8.4– 11.3 ms  observers.scroll         @codemirror/view
  7.4 ms / 7.1 ms / 5.5 ms  replaceChildren / removeChild / remove  ← lane + sticky DOM 重建
  6.8 ms        write                    src/editor/core/cm/lanes.ts  (overview lane)
  5.7 ms        visiblePixelRange        @codemirror/view
  4.4 ms        getListener              react-dom
```

4× CPU 下同一组数字：`measure 362.8 ms`、`getBoundingClientRect 147.0 ms`、`nextChild 99.5 ms`。

### 3.6 微基准（同一份 `node_modules` 里的 CodeMirror 6.43，Chromium，20k 行 C++ 文档，36 行可见）

复刻真实代码路径（`lanes.ts:84-110` 的循环逐行照抄）：

| 基准                                         | 1×             | 4×               |
| -------------------------------------------- | -------------- | ---------------- |
| overview lane，**4 000** 个 marks            | **7.4–9.1 ms** | **28.3–28.7 ms** |
| overview lane，**20 000** 个 marks           | **14.6 ms**    | **61.0 ms**      |
| 裸 `lineBlockAt` ×8 000                      | 4.9–5.1 ms     | 19.9–20.9 ms     |
| 语义高亮：40 000 个 mark 的 `Decoration.set` | 2.1 ms         | 10.9–12.8 ms     |
| 语义高亮：`decorationSet.map(change)`        | 0.1 ms         | 0.9 ms           |
| 视口级 300 个 mark 重建                      | ~0 ms          | ~0 ms            |
| **整文件 Lezer C++ 解析**（20k 行 / 550 KB） | **373 ms**     | **1 466 ms**     |

---

## 4. 主要嫌疑与证据（Culprits）

### 4.1 ⭐ 右侧 overview lane 每帧遍历全部 marks（与文件大小成正比）

`src/editor/core/cm/lanes.ts`

```ts
update(u) {                                    // 54-64：viewportChanged | selectionSet | geometryChanged | heightChanged
  if (u.docChanged || u.geometryChanged || u.heightChanged || u.viewportChanged || u.selectionSet || …) this.schedule();
}
read(view) {
  …
  for (const m of marks) {                     // 85：遍历 facet 里的每一条
    if (rows.size > MAX_RENDERED) break;       // 86：⚠ 限制的是"折叠后的像素桶"，不是遍历数
    const a = view.lineBlockAt(from);          // 89
    const b = to === from ? a : view.lineBlockAt(to);  // 90
    …
  }
}
```

- `scrollbarMarks` 来自三处：clangd 诊断（`lsp/diagnostics.ts:181, 204`，每文件每条诊断一个）、VCS 改动 hunk（`features/vcs/gutter.ts:260`）、搜索匹配（`core/cm/find.ts:134`）。
- 第 86 行的守卫按 `rows.size`（去重后的 lane 像素）判断，而 900 px 高的 lane 最多只有 ~900 个像素桶 → **循环实际上会遍历所有 mark**，20 000 条诊断 = 40 000 次 `lineBlockAt`。
- 实测 4 000 → 7.4–9.1 ms，20 000 → 14.6 ms（1×）/ 61 ms（4×），**每帧**。大 C++ 工程必然掉帧，且与窗口高度无关、只与诊断/变更数量成正比 → 完美匹配"工程越大越卡"的症状。
- 次要问题：`write()` 第 108-109 行把 `caret` 拼进 key，`#${caret}` → **每次光标移动都 `replaceChildren` 重建最多 4 000 个节点**（profile 里 `replaceChildren / removeChild / remove` ≈ 20 ms）。

### 4.2 ⭐ 打开/跳转后的 Lezer 解析追赶（一次性长任务）

- 现象：3 个 ~100 ms 长任务，全部落在 `FireIdleCallback → work`（`@codemirror/language` 的 `ParseContext`，通过 `requestIdle` `{timeout: 400}` 调度）。
- 根因：`ensureSyntaxTree(state, viewport.to, 50)` 只能同步解析 50 ms；首次滚到文件深处（本测试在 caret 跳到第 10 000 行之后）需要把前面 10 000 行全部解析完。
- 代价可测：**20k 行 C++ 全量解析 373 ms（1×）/ 1 466 ms（4×）**，以 ~100 ms 的 idle 分片投递 → 每片都是一个 long task（阻塞输入/滚动帧）。
- 4× 下稳态仍有 3 个长任务（max 163 ms）→ 慢机器上"打开就滚"必卡。

### 4.3 LSP / IPC：滚动停止后的请求风暴（浏览器模式测不到，代码路径 + 常量）

每次 `viewportChanged`（= 每次滚动帧，防抖后每段滚动一次）会触发：

| 插件                           | 防抖                    | 停止后发出的请求                                                                              | 位置                                                              |
| ------------------------------ | ----------------------- | --------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| inlay hints                    | 400 ms                  | 1 × `lspInlayHints(可见范围)`                                                                 | `lsp/inlay.ts:161, 180-210`                                       |
| usages code lens               | 900 ms                  | 1 × `docOutline`（按 doc 文本缓存）+ **最多 40 × `lspReferences`** + `cancelRequest` × 在途数 | `lsp/codeLens.ts:34-37, 109, 138-151, 189-212`                    |
| implementation / supers gutter | 500 ms                  | 1 × `docOutline` + **最多 60 ×** `lspImplementations` / `lspTypeHierarchy`                    | `navigation/implementationGutter.ts:50-51, 229, 250-252, 305-320` |
| 语义高亮                       | 600 ms（仅 docChanged） | 1 × `lspSemanticTokens`（整文件）                                                             | `lsp/semantic.ts:147-201`                                         |

放大器：`lspRequest()` 每次请求前 `await whenAllSynced()`（`lsp/lsp.ts:27-38`）——多一次 `doc_sync` 往返；clangd 侧 `references` / `inlayHint` 在 20k 行 TU 上都是重查询，且 **40 条 → 分批 CONCURRENCY=4 串行**（`codeLens.ts:189-212`）。这解释了"滚动停下来之后还要卡一下"。

### 4.4 光标移动：每键 5–19 ms

来自 3.4 + 3.5 的证据链：

1. `effectiveBindings()` 每次 keydown 重建整张表（`platform/keymap.ts:197-201, 508`）：profile 里 `effectiveBindings 5.4 + normalizeBinding 3.4 + letterOf 3.0 ≈ 12 ms / 40 键`。
2. CM：`measure 4.6 ms`、`getClientRects 10.7 ms`、`scrollableParents 3.5 ms`、`coordsInWidget 3.1 ms`（方向键会 `scrollIntoView`）。
3. Lezer：`addActions / readToken / advanceStack ≈ 17 ms / 40 键`（视口移动 → 重新高亮）。
4. React：`commitReconciliationEffects 4.9 ms`、`getListener 4.4 ms`，来自状态栏 + 3 个 breadcrumb 组件；每个都跑 `symbolChain()` 全量扫描 outline（`StatusItems.tsx:34-42`）。
5. `scheduleSessionSave` → 400 ms 后 `JSON.stringify(session)` 写 localStorage（`controller.ts:410-414, 417-434`；`session.ts:215-217`）。

### 4.5 已排除 / 已降级的嫌疑（negative results）

| 嫌疑                                          | 结论                            | 证据                                                                                                                                   |
| --------------------------------------------- | ------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| 语义高亮全文件装饰                            | **不是滚动问题**                | 40k 区间 build 2.1 ms、map 0.1 ms；仅 docChanged 600 ms 防抖                                                                           |
| `contain` / `content-visibility` CSS          | **无收益**                      | `.cm-line{contain:layout paint style}` 注入后：style+layout 321→287 ms、task 1906→1823 ms（首次滚动，被长任务主导），稳态无差异 → 噪声 |
| indent guides / rainbow brackets / wrapIndent | 视口级，可接受                  | `indentGuides.ts:72-84` 只用 `visibleRanges`；rainbow 默认关                                                                           |
| minimap                                       | 不存在（用 overview lane 代替） | `theme.ts:266-296`                                                                                                                     |
| `doc.toString()`                              | 仅命令触发，非滚动              | `actions.ts:401`（Cyclic Expand Word）、`actions.ts:421`（Copy Reference）、`controller.ts:710`（右键菜单）                            |
| `EditorView.updateListener` 全量扫描          | 未发现                          | `slot.ts:107-113` 只转发给 find bar 等监听者                                                                                           |
| status 更新节流                               | 正常                            | `controller.ts:740-754` rAF + 100 ms                                                                                                   |

---

## 5. 排序修复清单（Ranked fixes：收益 / 风险）

| #     | 修复                                                                                                                                                                                                                  | 预期收益（有实测锚点）                                                                                                        | 风险                                                | 工作量    |
| ----- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------- | --------- |
| **1** | **overview lane 改为"按 mark 集合变化时预分桶"**：把 mark 一次性折成固定数量的 lane 像素桶（≤ 900），滚动时只读缓存数组；caret 用独立元素 `transform` 更新，不再进 DOM 重建 key；循环加"最多遍历 N 条"的硬上限        | 每帧省 **7–15 ms**（4k–20k marks，1×）/ **28–61 ms**（4×）→ 大工程从 30 fps 回到 60 fps；光标移动省一次 ≤4 000 节点的重建     | 低（纯渲染层，可加单测）                            | 60–90 行  |
| **2** | **大文件语法高亮策略**：`docLength > 阈值`（建议默认 1–2 MB，可设置）时不挂 CM language（保留 commentTokens / indent），颜色交给 clangd 语义 token；或至少在 `stickyScroll` / `folding` 里避免强制整文件解析          | 去掉首次滚动的 **3 × ~100 ms** 长任务、`TBT 120 → ~0`、最大帧 `116 → ~17 ms`；20k 行 C++ 省 **373 ms** 解析（4× 下 1 466 ms） | 中（大文件失去本地高亮；需设置项 + 与语义高亮联动） | 80–120 行 |
| **3** | **视口→LSP 刷新加闸门**：`codeLens` / `lineMarkers` / `inlay` 只在（a）doc 变化，或（b）视口移动超过 N 行（例如 > `MARGIN_LINES`）时才刷新；`MAX_PER_PASS` 按文件大小自适应；`cancelRequest` 合并（一段滚动只发一次） | 滚动停止后的请求从 **≤40 + ≤60 + 1** 降到按需几条；去掉 clangd 侧排队与 IPC 序列化；预期消除"停下后再卡一下"                  | 中（inlay/lens 可能短暂缺省，需要 stale 标记）      | 50–80 行  |
| **4** | **光标路径减负**：`effectiveBindings()` 按 `useKeymap` 版本 memo；`symbolChain` 按 `(outline, line)` memo 或把 outline 建成按行索引；session 保存 400 ms → 2 s（且只在 tab 集合/脏状态变化时写）                      | 每键省 **1–3 ms**（4× 下 18.7 → ~15 ms/键，p95 24 → ~20 ms）                                                                  | 低                                                  | 40–60 行  |
| **5** | **语义 token 上限**：超过 N 个 token 时只保留视口附近（或干脆跳过），避免 4× 下 10.9 ms 的全量建树                                                                                                                    | 低（默认关闭；仅启用者受益）                                                                                                  | 低                                                  | 30 行     |
| **6** | **`viewportMargin`**：给大文件设 1–2 屏预渲染，减少快速滚动时的重渲染；或反过来在超大文件设为 0                                                                                                                       | 未实测（本次未观察到收益点），需 A/B                                                                                          | 低-中（内存/首屏变慢）                              | 2 行      |
| **7** | **CSS `contain`**                                                                                                                                                                                                     | **实测无收益，不建议做**                                                                                                      | —                                                   | —         |
| **8** | `doc.toString()` → `sliceDoc(from, to)`（`actions.ts:401/421`）                                                                                                                                                       | 仅 Copy Reference / Cyclic Expand Word，单次 600 KB 拷贝，收益极小                                                            | 低                                                  | 10 行     |

---

## 6. Quick wins（每项 ≤ 30 行）

### QW1 — overview lane：先分桶、caret 独立、遍历封顶（`src/editor/core/cm/lanes.ts`）

```ts
const MAX_MARKS = 4000; // 硬上限，防止诊断风暴
// 在 read() 里替换 84-99 的循环：
const marks = view.state.facet(scrollbarMarks);
const rows = new Map<string, { top: number; height: number; mark: ScrollbarMark }>();
let visited = 0;
for (const m of marks) {
  if (++visited > MAX_MARKS) break;          // ← 限额按"遍历数"，不是像素桶
  const from = Math.min(Math.max(0, m.from), len);
  const to = Math.min(Math.max(from, m.to ?? from), len);
  const a = view.lineBlockAt(from);
  const b = to === from ? a : view.lineBlockAt(to);
  const px = Math.floor(a.top * scale);
  const key = `${px}:${m.color}:${m.kind ?? "line"}`;
  if (rows.has(key)) continue;               // ← 已占位的像素不再算 height
  rows.set(key, { top: pos(a.top), height: …, mark: m });
}
```

再把 `write()` 的 key 去掉 caret（`lanes.ts:108-109`），caret 用一个独立 `div` 每次只改 `style.top`。

### QW2 — 大文件不挂 CM language（`src/editor/core/model/document.ts` 打开路径 / `slot.ts:78-88`）

```ts
// setup.ts：加一个阈值，超限就不传 language（只留 commentTokens / indent）
export const SYNTAX_LIMIT = 2_000_000; // UTF-16 units，约 2 MB
export function languageFor(o: { language: LanguageSupport | null; docLength: number }) {
  return o.docLength > SYNTAX_LIMIT ? [] : (o.language ?? []);
}
// slot.ts:  compartments.language.of(languageFor({ language: model.language, docLength }))
```

> 注意：`HEAVY_LIMIT`（`setup.ts:82`，8 MB）目前只关掉括号匹配/缩进参考线等；语法高亮没有阈值。

### QW3 — memo `effectiveBindings()`（`src/platform/keymap.ts:197-201`）

```ts
let cache: { key: string; list: Array<[string, string]> } | null = null;
export function effectiveBindings() {
  const { preset, overrides } = useKeymap.getState();
  const key = preset + "\u0000" + JSON.stringify(overrides); // 或用 store revision
  if (cache?.key === key) return cache.list;
  const merged = { ...PRESETS[preset], ...overrides };
  const list = Object.entries(merged).flatMap(([id, keys]) => keys.map((k) => [normalizeBinding(k), id] as [string, string]));
  return (cache = { key, list }).list;
}
```

### QW4 — 只在视口真的移动了才刷新 code lens / inlay（`src/editor/lsp/codeLens.ts:138-140`、`inlay.ts:180-182`）

```ts
update(u: ViewUpdate) {
  if (u.docChanged) return this.refresh();
  if (!u.viewportChanged) return;
  const line = u.state.doc.lineAt(u.view.viewport.from).number;
  if (Math.abs(line - this.lastLine) < 80) return;   // 小幅滚动不重算
  this.lastLine = line;
  this.refresh();
}
```

### QW5 — session 保存降频（`src/editor/core/controller.ts:410-414`）

```ts
private scheduleSessionSave() {
  if (!this.sessionRoot) return;
  clearTimeout(this.sessionTimer);
  this.sessionTimer = setTimeout(() => this.saveSession(), 2_000); // 400 → 2000
}
```

### QW6 — `symbolChain` memo（`src/editor/core/view/StatusItems.tsx:34-42`）

```ts
const chainCache = new WeakMap<readonly OutlineItem[], Map<number, OutlineItem[]>>();
function cachedChain(outline: readonly OutlineItem[], line: number, col: number) {
  let byLine = chainCache.get(outline);
  if (!byLine) chainCache.set(outline, (byLine = new Map()));
  const hit = byLine.get(line);
  if (hit) return hit;
  const chain = symbolChain(outline, line, col);
  byLine.set(line, chain);
  return chain;
}
```

---

## 7. 复现方式（Reproduce）

```bash
# 1) 生产包（保留函数名，便于 profile 归属）
pnpm exec vite build --mode perf --minify false --outDir target/perf/dist-unminified

# 2) 已有官方基准（20k 行 TS 滚动，含预算校验）
pnpm perf --only scroll --trace --profile

# 3) 本次调查用的驱动（仓库外，避免污染工作区）
node /tmp/ocprobe/drive.mjs --kind cpp --cpu 1 --trace    # 帧率/长任务/CDP split/CPU profile
node /tmp/ocprobe/drive.mjs --kind cpp --cpu 4 --trace    # 慢机器
node /tmp/ocprobe/drive.mjs --kind txt --cpu 4            # 对照组（无语法树）
# 微基准（overview lane / 语义装饰 / Lezer 全量解析）
cd /tmp/probe && /path/to/node_modules/.bin/vite build && node run.mjs
```

产物：`/tmp/ocprobe/out/result-*.json`、`*.cpuprofile`、`*.trace.json`（Chrome trace，可直接拖进 Perfetto / DevTools）。

---

## 8. 结论与建议顺序

1. **先做 #1（overview lane）**：唯一的"每帧 × 与文件大小成正比"的热点，收益最大、风险最低、可单测。
2. **再做 #2（大文件语法阈值）**：消除打开/跳转后 3 × 100 ms 的长任务，这是用户"打开文件就卡"的直接来源。
3. **然后 #3（LSP 闸门）**：浏览器基准测不到，但在真实 clangd 工程里是"滚动停下后再卡一下"的主因（≤40 references / ≤60 implementations 每次视口刷新）。
4. #4 是日常手感（每次按键 5–19 ms），可作为随手优化。
5. **不要**为滚动去加 `contain` / `content-visibility`（已实测无收益）；`viewportMargin` 需要单独 A/B。

> 待补：Tauri 窗口 + 真实 clangd + 真实 C++ 工程的端到端 trace（`e2e/` 已能驱动 Tauri 窗口），用于验证 #3 的 IPC 收益量级。
