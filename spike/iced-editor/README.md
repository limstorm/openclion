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

## IME 为什么还没显示出来

原型能把 `Event::InputMethod(Preedit)` 画成"下划线 + 橙色"，但 **`update()` 根本收不到 IME 事件**，
`EDITOR_TRACE` 只看到一次 `ime: Closed`。原因不是 iced 没有这个能力，而是：

> iced 把 `Event::InputMethod` **只投递给"获得焦点"的 widget**，而焦点是
> `operation::Focusable` + `Widget::operate` 注册的（见 `iced_widget::text_editor` 里的
> `state.focus` 与 `operation.focusable(...)`）。裸 `Widget` 没有 `Id`、不参与这套注册，
> 就永远收不到 IME 事件。

所以修法明确，且不涉及 fork：

1. 给编辑器一个 `Id`，实现 `operation::Focusable`，在 `operate()` 里转发
   （`operation.focusable(id, bounds, state)`）；
2. 点击时通过 `iced::advanced::widget::operate::focus(id)` 之类的操作把焦点给自己；
3. 焦点到手后，`Shell::request_input_method(&InputMethod::Enabled { cursor: caret_rect, .. })`
   才会真正下发到 winit（`set_ime_allowed(true)` + `set_ime_cursor_area`），
   `Preedit` / `Commit` 随即可用；
4. 也可以把 `preedit` 交给 `InputMethod::Enabled` 里的 `preedit` 字段，让运行时以
   "over-the-spot" 方式画在光标处——两条路都行。

## 与真实编辑器的差距

原型刻意省掉的部分（M2 里要做）：真实字形度量（现在用"0.6em/1em"估算列宽，
应当改用 `Paragraph::grapheme_position` / `hit_test`）、语法高亮、多光标、折叠、
自动缩进、undo/redo、查找替换、双向文本、滚动条、行号栏。
