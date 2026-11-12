# WebKitGTK / Wayland 前端渲染性能：合成路径与每帧开销

> 范围：把 openclion 的前端（React + CodeMirror 6）跑在真实的 WebKitGTK webview 里，
> 在 Wayland 这类"合成器驱动"的会话中测滚动与窗口 resize，找出与 Chromium 基准不同的那部分开销。
> 基线：`741746a`（2026-11-12）。只读调查 + 两处修复（见 §3），测量数据在 §2、§5。
>
> **一句话结论**：前端代码不是瓶颈（主线程每帧 < 2 ms，没有长任务）；差距在 **WebKit 的加速合成路径**——
> 在没有 GPU 的机器上（VM、容器、llvmpipe、远程桌面），它每帧都要用 CPU 光栅化整个窗口，
> 反而比 WebKit 的 cairo 直绘路径慢一倍以上。Wayland 会放大这个问题：resize 由合成器逐帧驱动，
> X11 上的一次性卡顿在 Wayland 上就是"窗口拖着指针走不动"。

---

## 0. 结论摘要（TL;DR）

1. **前端没有热点**：滚动时主线程 JS 每帧约 1 ms，`longtask` 计数为 0；`perf` 采样中
   WebKit 主线程（`libwebkit2gtk`）只占约 6.5%，而 **llvmpipe（软件 GL）线程占约 45%**。
2. **加速合成在软件 GL 上是负收益**。同一份代码、同一个 12k 行 C++ 文件、同一个窗口：

   | 交互（1.2–1.4 s） | 合成开（WebKit 默认）                     | 合成关（cairo 直绘）                            |
   | ----------------- | ----------------------------------------- | ----------------------------------------------- |
   | 滚轮滚动 60 次    | **65 帧**，p95 **130 ms**，max 141–222 ms | **103–115 帧**，p95 **50–78 ms**，max 77–109 ms |
   | 拖拽 resize 30 步 | **23 帧**，p95 **614 ms**，max **668 ms** | **86–97 帧**，p95 ≈52 ms，max **49 ms**         |

3. **resize 的代价随文档长度线性增长**（不是随可见行数）：同一个窗口，3 行的 `main.cpp`
   59 帧 / max 77 ms，2.4k 行的 `medium.cpp` 50 帧 / max 102 ms，12k 行的 `big.cpp`
   23 帧 / max 668 ms —— 合成路径会为整条超长滚动区（12k 行 ≈ 216,000 px）重建 tile。
4. **修复**：`src-tauri/src/linux_rendering.rs` 在 GTK/WebKit 启动前判断这台机器有没有真正的 GPU
   （`/dev/dri/renderD*`、`LIBGL_ALWAYS_SOFTWARE`、`GALLIUM_DRIVER`），没有就替 WebKit 设
   `WEBKIT_DISABLE_COMPOSITING_MODE=1`。有 GPU 的机器行为不变；WebKit 自己的开关和
   `OPENCLION_WEBKIT_COMPOSITING` 都能覆盖它。
5. **前端顺手修掉一处与文档长度成正比的每次光标移动开销**：`symbolChain()` 从线性扫描改成
   二分（`src/editor/core/view/outline.ts`）。8,000 个顶层符号的 outline：**87.5 µs → 1.5 µs/次（58×）**。
   端到端落在噪声里（光标移动还有别的开销），但大 C/C++ 头文件下它是真实存在的浪费。
6. **不要为滚动去改 CSS**：去掉编辑器左侧 gutter / 右侧概览条 / sticky header 只多出约 25% 的帧
   （而且那是拿掉功能），`contain`、`will-change: transform`、`translateZ(0)`、去掉圆角与阴影
   在合成开的路径下都在噪声内（±5%）。
7. **每帧 DOM 操作是干净的**：滚动时每帧约 11.6 次 `getBoundingClientRect`、1.4 次
   `querySelector`、0 次 `replaceChildren`；46 次 `setAttribute` 全部来自 CodeMirror 给
   新进入视口的行建 span，不是应用代码的抖动。

---

## 1. 测量方法与局限

| 项       | 值                                                                                                                  |
| -------- | ------------------------------------------------------------------------------------------------------------------- |
| 应用构建 | `cargo build -p openclion`（debug，含 `tauri-plugin-mcp-bridge`）                                                   |
| 前端     | `pnpm build` 的生产包，用 `python3 -m http.server 1420 --directory dist` 提供（避免 dev ESM 图的噪声）              |
| 会话     | Ubuntu 24.04，GNOME，X11 `:10.0`（`DISPLAY=:10.0`），WebKitGTK **2.52.6**                                           |
| GPU      | 无：`/dev/dri` 只有 `card0`（ASPEED VGA），**没有 `renderD*`**；`LIBGL_ALWAYS_SOFTWARE=1`，GL 走 llvmpipe           |
| 窗口     | 1440×900，`devicePixelRatio = 1`                                                                                    |
| 工程     | `/tmp/oc-perf/proj`：`big.cpp` 12,008 行 / 751 KB，`medium.cpp` 2,406 行，`main.cpp` 3 行                           |
| 采样     | 应用内 `requestAnimationFrame` 帧间隔 + `PerformanceObserver(longtask)`；`perf record -F 999 -p <WebKitWebProcess>` |

**为什么在 X11 上测 Wayland 的结论**：帧时间由"WebKit 每帧画多少像素、走哪条路径"决定，
这部分两个后端相同（都是 WebKitGTK + 同一套渲染管线）。差别在窗口管理：Wayland 的 resize 是
合成器逐帧驱动的连续过程，X11 上的一次 668 ms 卡顿在 Wayland 上会表现成拖拽时的持续掉帧，
方向一致、只重不轻。真机复测的脚本见 §4。

**怎么驱动**（用 MCP bridge 在页面里执行 JS，用 `xdotool` 产生真实输入）：

```bash
# 帧采样器
node probe.mjs 'window.__f=[];window.__on=true;let l=performance.now();
  requestAnimationFrame(function t(n){if(!window.__on)return;window.__f.push(n-l);l=n;requestAnimationFrame(t)}); return 1'
# 真实滚轮：60 次，间隔 20 ms，指针放在编辑器中央
xdotool mousemove 700 400 && xdotool click --repeat 60 --delay 20 5
# 取结果（p50/p95/max/超 25 ms 的帧数）
node probe.mjs 'window.__on=false;const f=window.__f.slice(2).sort((a,b)=>a-b);const q=p=>f[Math.floor(f.length*p)];
  return {n:f.length,p50:+q(.5).toFixed(1),p95:+q(.95).toFixed(1),max:+f.at(-1).toFixed(1),over25:window.__f.filter(x=>x>25).length}'
```

resize 用 `xdotool windowsize $W $((1200+i*8)) $((700+i*6))`，30 步、每步 30 ms。

**局限**：单机、单次会话、样本 2–3 次；帧时间在这台没有 GPU 的 VM 上受 llvmpipe 支配，
绝对值不能外推到有 GPU 的桌面。结论只用**相对比较**（同一会话内开/关合成路径、改了/没改代码）。

---

## 2. 发现

### 2.1 合成路径是这台机器上最大的开关

`WEBKIT_DISABLE_COMPOSITING_MODE` 是 WebKit 自己的开关（`1` = 关掉加速合成，回退到 cairo 直绘）。
其他条件完全不变，只改这个环境变量：

| 场景                                   | 合成开                                   | 合成关                                  |
| -------------------------------------- | ---------------------------------------- | --------------------------------------- |
| 14,400px 的窗口 resize，12k 行文件打开 | 23 帧 / max 668 ms                       | 86 帧 / max 49 ms                       |
| 滚轮 60 次                             | 61–72 帧 / p95 126–135 ms / over25 20–23 | 100–115 帧 / p95 50–78 ms / over25 9–14 |

`perf record` 的归因（滚动 5 s，27,245 个样本）：`llvmpipe-*` 等软件光栅线程合计约 45%，
`libwebkit2gtk` 6.5%、`JSC` 的 JIT 代码只占很小一部分。也就是说：**CPU 花在"用软件把每帧重新画一遍"上，
不是花在 JS / 布局上**。

### 2.2 resize 的代价与文档长度成正比

同样 1440×900 窗口、同样 30 步 resize：

| 打开的文件   | 行数   | 帧数 | max        |
| ------------ | ------ | ---- | ---------- |
| `main.cpp`   | 3      | 59   | 77 ms      |
| `medium.cpp` | 2,406  | 50   | 102 ms     |
| `big.cpp`    | 12,008 | 23   | **668 ms** |

可见行数（`.cm-line`）三种情况都只有 60–70 个，所以差别只能来自**整条滚动区的高度**
（12k 行 ≈ 216,000 px）：合成路径在每次尺寸变化时重建 tile/backing store，开销跟着滚动区走。
把编辑器整个 `display:none` 掉之后 resize 回到 83 帧 / max 82 ms，进一步坐实是编辑器渲染树的问题。
合成关掉之后同一个文件是 86–97 帧 / max 49 ms。

### 2.3 每帧 DOM 操作（应用侧）

在滚动期间给 `Element.prototype` 打点统计（69 帧样本）：

| 调用                    | 次数  | 每帧 | 来源                                                     |
| ----------------------- | ----- | ---- | -------------------------------------------------------- |
| `getBoundingClientRect` | 799   | 11.6 | CodeMirror 的 measure + sticky scroll 的 gutter 宽度读取 |
| `setAttribute`          | 3,206 | 46   | CodeMirror 给新进入视口的行建 span（`class`）            |
| `querySelector`         | 94    | 1.4  | sticky scroll 的 `.cm-gutters` / `.cm-lineNumbers`       |
| `replaceChildren`       | 0     | 0    | —                                                        |

应用代码没有每帧重建 DOM 的地方。`symbolChain()` 是唯一一处"每次光标移动 × 与文件长度成正比"的
纯计算（见 §3.2）。

### 2.4 编辑器装饰：有代价，但不值得动

合成开的路径下，把左侧 gutter、右侧概览条、sticky header 一起 `display:none`：
61–68 帧 → 82–84 帧（约 +25%）。代价真实存在，但那是拿掉行号、诊断概览和 sticky header 换来的；
在合成关的路径上（也就是本次修复后的默认路径）同样的隐藏只有 1–2 帧的差别。**不动它们。**

同样在噪声内的实验：`.cm-scroller { will-change: transform }` / `translateZ(0)` /
`contain: paint`、去掉全局 `box-shadow` 与 `border-radius`、去掉工作台的 project 渐变背景
（`radial-gradient`）。**这些都不是这里的杠杆**，不要再为滚动加这类 CSS。

---

## 3. 修复

### 3.1 让 WebView 走这台机器扛得住的渲染路径

`src-tauri/src/linux_rendering.rs`，在 `main()` 里 GTK/WebKit 启动之前调用：

```rust
#[cfg(target_os = "linux")]
openclion_lib::linux_rendering::tune();
```

判定顺序（纯函数 `decide()`，7 个单测覆盖）：

1. `OPENCLION_WEBKIT_COMPOSITING=off|on` —— 显式覆盖，优先。
2. 环境里已经有 `WEBKIT_DISABLE_COMPOSITING_MODE` —— 尊重用户的设置，不动。
3. `LIBGL_ALWAYS_SOFTWARE` 为真值，或 `GALLIUM_DRIVER` 是 `llvmpipe`/`softpipe`/`swrast`/`lavapipe` —— 关。
4. `/dev/dri` 下没有 `renderD*` 渲染节点（VM、容器、远程桌面、驱动回退）—— 关。
5. 其余（真有 GPU）—— 保持 WebKit 默认（合成开）。

命中时打印一行，便于用户确认：

```
openclion: no /dev/dri/renderD* render node (software GL): WebKit accelerated compositing disabled for this session
```

验证（本机，未设任何环境变量）：resize 23 帧 / max 668 ms → **97 帧 / max 49 ms**；
滚动 65 帧 / p95 130 ms → **104–115 帧 / p95 50–74 ms**。

为什么放在 `main()`：WebKit 在 web process 启动、`WebSettings` 初始化时读这个变量；`run()` 之后再设已经晚了。
`unsafe { env::set_var }` 在这里安全：GTK/WebKit 的线程还没有起来。

### 3.2 `symbolChain()`：线性扫描改二分

`src/editor/core/view/outline.ts` 的 `symbolChain()` 在**每次光标移动**（状态栏面包屑 + 编辑器内
面包屑，两处订阅）里被调用，原来对每一层做 `level.find(...)` —— 12k 行 C++ 文件有约 8,000 个顶层
symbol，等于每次按键扫 8,000 项。

outline 的兄弟节点按起始位置有序、互不重叠（`crates/ide-language/src/syntax/outline.rs` 对每个节点的
children 按 start byte 排序），所以"最后一个 start ≤ 光标"的兄弟就是唯一可能包住光标的那个：
二分即可。同一份 8,000 项数据、同一台机器：

| 实现        | µs / 次（2,000 次平均） |
| ----------- | ----------------------- |
| 线性 `find` | **87.5**                |
| 二分        | **1.5**                 |

`src/editor/core/view/outline.test.ts` 用线性版本做 oracle，在 400 个符号、边界（区间两端闭）、
空 outline、负坐标上逐一对照，另外覆盖了嵌套与同层多选。

---

## 4. 复现

```bash
# 1) 构建
pnpm install && pnpm build
cargo build -p openclion        # debug 构建才带 MCP bridge（127.0.0.1:9223）

# 2) 起一个静态服务给 debug 二进制（它编译期写死了 http://localhost:1420）
python3 -m http.server 1420 --directory dist &

# 3) 起应用；想要"有 GPU"的对照组就显式打开合成
DISPLAY=:10.0 ./target/debug/openclion /tmp/oc-perf/proj
#   OPENCLION_WEBKIT_COMPOSITING=on   → 强制合成
#   OPENCLION_WEBKIT_COMPOSITING=off  → 强制关
#   WEBKIT_DISABLE_COMPOSITING_MODE=1 → 同 WebKit 自己的开关，本模块不会再改

# 4) 页面里探针（需要 debug 构建 + withGlobalTauri；`pnpm dev:app` 的 dev 配置已打开）
#    见 §1 的三段 node probe.mjs 命令。
```

注意：`cargo build` 直接编出来的 debug 二进制加载的是 `dist/`，不是 vite dev server；
如果要用 MCP bridge 的 `execute_js`，需要 `window.__TAURI__`（dev 配置的 `withGlobalTauri`），
可以 `pnpm dev:app`，或 `TAURI_CONFIG='{"app":{"withGlobalTauri":true}}' cargo build -p openclion`。

---

## 5. 还没有做的

1. **Wayland 真机复测**：本机是 X11 + ASPEED，没有 Wayland socket。结论方向一致
   （WebKitGTK 渲染路径相同），但 resize 的连续性和分数缩放（`GDK_SCALE=2` 时像素翻 4 倍，
   成本同比例上升）需要在真机上量一遍。
2. **分数缩放**：`devicePixelRatio > 1` 时上面所有 paint 成本按 `dpr²` 放大。如果在 2x 屏上
   resize 仍然卡，下一个可做的杠杆是"resize 期间降低装饰画质"，但需要真机数据支撑。
3. **合成路径的 tile 策略**：`big.cpp` 在合成开时 668 ms 一帧，怀疑是超长滚动区（216,000 px）
   的 tile 重建；没有 WebKit 内部计数器，暂时只能从"关掉合成就回到 49 ms"侧面证明。
4. Chromium 基准（`pnpm perf`）在这台无 GPU 的 VM 上会被 vsync 锁在 30 fps，
   `scroll.frameP95Ms`、`typing.missedFramePct` 等预算项会误报——**不要在无 GPU 环境里用预算判定回归**，
   只用它比较"改前/改后"的相对值。
