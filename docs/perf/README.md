# 性能：测量结论与已实施修复

三个方向的实测报告与代码修复记录。重新测量时按各报告里「测量方法」一节操作。

| 报告                     | 内容                                                                                   |
| ------------------------ | -------------------------------------------------------------------------------------- |
| [startup.md](startup.md) | 启动链路：splash gate、`open_project` 各阶段（`opened … {laps}` 日志）、dev ESM 图开销 |
| [index.md](index.md)     | 索引：walk/read/parse/persist 阶段耗时、cold vs warm、符号与 RSS、查询延迟             |
| [editor.md](editor.md)   | 编辑器：滚动帧时间、长任务、概览轨道、keymap/光标路径、视口 LSP 请求                   |

## 已实施（按提交）

| 提交      | 改动                                                                | 依据 / 预期                                                        |
| --------- | ------------------------------------------------------------------- | ------------------------------------------------------------------ |
| `505f05a` | splash 只等 launch 700 ms，项目在后台继续打开                       | gate 是 `holdSplash(launch)`；warm −0.7~0.9 s                      |
| `505f05a` | 索引读前过滤：8 KiB 前缀判二进制/语言，无符号文件用 size+mtime 哈希 | 读取字节 71% 是二进制；cold −0.5~1.5 s、RSS −150~200 MB            |
| `2c8c2bb` | watch 树注册移到 `openclion-fs-watch` 线程                          | `Watcher::start` 635–771 ms / 4,181 个 watch；每次 open −0.6~0.8 s |
| `5103ce3` | clangd `-j=${cpus}` + `--background-index-priority=normal`          | 首开 clangd 索引 102 s；默认 `low` 只用空闲 CPU                    |
| `6fe6905` | 索引解析走自有线程池（`cores-1`），查询留全局池                     | 索引期间查询 p95 75–111 ms → <10 ms                                |
| `9ba09dc` | 概览轨道分桶结果缓存 + 光标独立元素                                 | 每帧 7.4–9.1 ms（4,000 marks），光标移动不再重建 DOM               |
| `e9d6bfe` | `effectiveBindings()` 按 `(preset, overrides)` 缓存                 | 每键 5–8 ms → ~1 ms                                                |
| `47527cb` | codeLens 只在视口越出已取窗口时刷新                                 | 每滚动停止 ≤40 `lspReferences`，合计 ≤101 请求                     |
| `2233dfc` | inlay hints 同样只在视口越界时刷新                                  | 同上，去掉重复请求                                                 |
| `115e359` | splash 动画尾巴 320–480 ms → ~230 ms                                | 补条 80 ms + 淡出 120 ms + 30 ms 移除                              |

## 有意不做

- **大文件跳过语法高亮**：实测长任务来自 623 KB 文件，把阈值降到能让它生效（~500 KB）会让多数真实文件失去高亮；保留 8 MB `large` 阈值。

## 实测后撤回的一项（2026-11-03）

**feature 延迟加载**：把只提供工具窗/设置页的 8 个 feature（database、http-client、notebook、github、coverage、profiler、deployment、libraries）改成 `import()` + 首帧后激活，A/B 实测（Chromium + Vite dev，`performance.getEntriesByType("resource")` 统计 `/src/features/` 模块）：

| 状态   | 首帧加载的 feature 模块数 |
| ------ | ------------------------- |
| 改动前 | 91                        |
| 改动后 | 88                        |

只少 3 个请求——这些 feature 的 `activate()` 内部本来就是懒加载重模块，入口文件很小。报告里「−1.3~1.5 s」是对**全部 40 个 feature**（含 editor/run/search/vcs/debugger 这类重量级）的估算，而那部分会改变首屏行为、风险与收益不匹配。已 revert（`a323a9c`），结论记录在此。

## 待办

1. **拆分 feature 的 `import`/`activate` 链**（dev −1.3~1.5 s）：`app`（launch barrier）、`editor`、`project-trust` 同步；其余走 `import()` 并在首帧后激活；同步调整调用 `activateFeatures()` 的测试。
2. **改后实测**：同机跑启动 + 索引对照（`target/perf-probe/app-probe.mjs`、`cargo bench -p ide-index`），把上表收益数字化。
