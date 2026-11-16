# 工作台外壳骨架（M1）

把 web 工作台的外壳结构搬成 iced 原生实现，先写代码、过静态检查，再跑起来看。

```bash
cargo build --release
DISPLAY=:10.0 ./target/release/iced-shell      # SHELL_TRACE=1 打印按键；SHELL_PALETTE=1 启动即开命令面板
```

## 模块划分（与 web 侧一一对应）

| 文件               | 对应 web                                                                 | 内容                                                                |
| ------------------ | ------------------------------------------------------------------------ | ------------------------------------------------------------------- |
| `src/registry.rs`  | `registerToolWindow` / `registerActions` / `registerStatusItem` / keymap | 14 个工具窗、12 条命令（带 `when` 上下文）、键位表，全部按 id 索引  |
| `src/tokens.rs`    | CSS 变量                                                                 | 明/暗两套色板（背景、面板、边框、正文、次要、强调、成功/警告/错误） |
| `src/i18n.rs`      | `locales/*.json`                                                         | 按 key 取文案，中英双语；未知 key 回退原 key                        |
| `src/tools.rs`     | 各 feature 的工具窗视图                                                  | 项目树、搜索结果、Git 日志、问题列表、占位面板                      |
| `src/workbench.rs` | workbench 布局                                                           | 活动栏、左右/底部 dock、标签栏、编辑区、状态栏、命令面板            |
| `src/app.rs`       | zustand store + actions                                                  | 状态与 `Message`/`update`，命令分发，键位匹配                       |
| `../iced-editor`   | CodeMirror + 编辑器集成                                                  | 复用的文本 widget（rope、光标、选区、IME、`Palette` 主题）          |

## 已经能用的

- 活动栏切换工具窗，左/右/底部三处 dock，dock 内标签可切换、`×` 关闭；
- 项目树读**真实工程**（`ide_vfs::read_dir`，excludes 与 web 相同）；点目录展开、点文件打开真实内容；
- 文件/文件夹图标用 web 的 **SVG 图标集**（16px，暗色/亮色各一套），git-ignored 的条目文字与图标变淡；
- 状态栏语言名来自 `languages/*.toml`（`ide_language` 的嵌入注册表），不是手写扩展名映射；
- 标签页切换与关闭，关闭最后一个会自动补一个 `welcome.md`；
- 状态栏：分支、行列、编码、主题开关、语言开关、命令面板入口；
- 命令面板：`Ctrl+Shift+P` 打开、输入过滤、点行执行、`⏎` 按钮执行；
- 主题：`Ctrl+\` 或状态栏切换明暗，**编辑器跟随主题**（`iced_editor::Palette`）；
- 语言：状态栏 `中文/EN` 切换，工具窗标题与提示随之切换。

## 静态检查

```bash
cargo clippy --release           # 无警告
cargo test --release             # registry 4 项：id 唯一、keymap 命令存在、chord 修饰键匹配
(cd ../iced-editor && cargo test --release)   # 3 项：CJK 宽度、x↔列号往返、rope 编辑
```

## 已知问题（下一步）

1. **命令面板里的 Enter 不生效**：焦点在 `text_input` 时 iced 吞掉 Enter，`on_submit` 在 overlay 里没有触发
   （`SHELL_TRACE=1` 能看到 `Named(Escape)` 和其它键，但看不到 `Named(Enter)`）。目前用 `⏎` 按钮兜底，
   需要按 iced 的 overlay 事件路由再查一次。
2. 工具窗内容是示例数据，还没接 `ide-*`（M1 的下一步：打开工程 / 文件树 / 打开文件三条真实链路）。
3. dock 还不能拖拽分栏（要换 `pane_grid`），活动栏也没有分组分隔和提示气泡。
