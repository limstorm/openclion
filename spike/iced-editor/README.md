# 自研编辑器原型（M2.0）

用 iced 0.14 的自定义 `Widget` 写一个编辑器：`ropey` 缓冲区、光标、选区、按键、输入法。
目的是验证迁移方案里"编辑器要自己写"这一条到底靠不靠谱。

```bash
cargo build --release
DISPLAY=:10.0 ./target/release/iced-editor    # EDITOR_TRACE=1 打印 IME 事件
```

## 已验证

| 能力                                    | 状态          |
| --------------------------------------- | ------------- |
| `ropey` 缓冲区 + 按行视口渲染           | ✅ 只画可见行 |
| 光标绘制、点击定位                      | ✅            |
| 拖拽选择 + 选区高亮                     | ✅            |
| ASCII 输入、退格、方向键、Home/End、Tab | ✅            |
| 中文渲染（字体回退）                    | ✅            |
| IME 预编辑串                            | ❌ 见下       |

## IME：三个坑，都填上了

原型最初收不到任何 `Event::InputMethod`。逐个排查后，能显示"你好"的完整条件是这样三条：

1. **必须在 iced 的焦点体系里注册**：`Widget::state()` 返回一个实现 `operation::Focusable` 的
   状态，`Widget::operate()` 里 `operation.focusable(Some(&id), bounds, state)`。
   裸 `Widget` 不注册就永远收不到 IME 事件。
2. **`Id` 必须跨帧稳定**：`Id::unique()` 每帧新建一个，焦点会丢、IME 随之中断
   （最初就是这个症状：事件来了但下一帧又没了）。
3. **不要在 key 事件里清空 preedit**：IBus 会同时发 `Keyboard` 和 `Preedit`，
   如果键盘分支把 `preedit` 置空，候选串就永远不显示。`preedit` 只该由
   `Commit` / `Closed` 清掉，且组合期间要把键盘的 `text` 交给 IME，不能自己插入。

填完之后：`Preedit("你好", 6..6)` 到达 `Widget::update`，我们自己画成"橙色 + 下划线"，
空格提交时 `Commit("你好")` 插入 rope，光标跟到后面。

另一条备选路径（未采用，但可用）：把 preedit 塞进
`InputMethod::Enabled { preedit: Some(..), .. }`，由 `iced_winit` 以
"over-the-spot" 方式画在光标处——`iced_winit/src/window.rs` 里已实现。

| 能力                            | 状态 |
| ------------------------------- | ---- |
| IME 组合串显示（橙字 + 下划线） | ✅   |
| 组合期间不误插入拼音字母        | ✅   |
| 空格/回车提交中文               | ✅   |

## 与真实编辑器的差距

原型刻意省掉的部分（M2 里要做）：真实字形度量（现在用"0.6em/1em"估算列宽，
应当改用 `Paragraph::grapheme_position` / `hit_test`）、语法高亮、多光标、折叠、
自动缩进、undo/redo、查找替换、双向文本、滚动条、行号栏。
