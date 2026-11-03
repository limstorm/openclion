# OpenCLion 索引 (Indexing) 性能调查

> 范围：`crates/ide-index` 及其驱动路径（`ide-host/src/project.rs`、`ops/nav.rs`、`ide-language`、`ide-vfs` watcher），
> 以及状态栏里用户看到的 “Indexing / Indexes” 进度。
> 基线：`fa12a42`（2026-11-03）。机器：Intel i7-9700（8C/8T）、61 GB RAM、NVMe、Linux 7.0.0、rustc 1.99.1。
> 真实工程：`/home/limstorm/gowork/src/gitee.com/kwbasedb`（KaiwuDB 工作树；197 GB / 163,774 个文件，忽略规则后 **45,965** 个文件 / **1.26 GB** / **519,688** 个符号）。
> 另参考 `~/gowork/src/gitee.com/KaiwuDB`（较小的 CMake 工作树，18,759 个文件），用于对照应用日志。
> 合成工程：`/tmp/openclion-perf/proj`（2,000 个 `.cpp` + 500 个 `.h` + 生成目录，忽略后 **2,975** 个文件 / 13.9 MB / **86,646** 个符号）。

---

## 0. 结论摘要（TL;DR）

1. **索引在“打开工程”时启动，不是首个查询触发**：`Host::open_project` 同步 `ProjectIndex::open`，随后后台线程立即 `load_persisted()` → `full_scan()`（`ide-host/src/project.rs:110`、`ide-index/src/lib.rs:103-131`）。
2. **正常单窗口重启是 warm，不是全量重建**：真实工程 warm 为 **317 ms（release）/ 1.06 s（debug）**，`todo=0`，没有重新解析任何文件；应用日志同样是 `0 parsed`。所以“每次启动都全量索引”**不成立**（常见场景下）。
3. **但 cold 构建很贵**：**9.9 s（release）/ 12.1 s（debug）**，应用日志实测 17.5 s；进程侧解析计时合计 **46.8 s**，读取 **1.26 GB**，其中 **889 MB（71%）是二进制文件**，峰值 RSS **414 MB**。
4. **仍有 4 条路径会导致“每次启动全量重建”**：
   - workspace（`include_root=false`）成员**永远不持久化**（`filter.rs:164-170`；实测第二次打开仍 `records=0, todo=20`）；
   - 同一 cache 被第二个窗口/实例占用：redb `DatabaseAlreadyOpen` → 退化为纯内存索引并全量解析（`store.rs:35-40`；应用日志有 `index cache ... is in use` 警告）；
   - 缓存指纹变化：schema / grammar / **`CARGO_PKG_VERSION`**（`store.rs:48`、`syntax/mod.rs:128`）；
   - “Clear caches”、数据库损坏（`caches.rs:26-30`、`store.rs:41-45`）。
5. **状态栏里“很久的 Indexing”很可能不是这个索引**：LSP 的 `$/progress` 被原样转成 `IdeEvent::Task`，标题也常是 “Indexing”，带百分比（`ide-lsp/src/handler.rs:45-108`）；而本项目索引的 `IndexState` 事件**不带百分比**，warm 只显示 <1 s。两个同名任务并存，`StatusBar` 取最后一个 running 任务（`src/workbench/StatusBar.tsx:96`），所以用户看到的多半是 **clangd 后台索引**。
6. **索引期间“导航/搜索卡”是可复现的**：索引与查询共用同一个全局 rayon 线程池（`worker.rs:300` 与 `matching.rs:118`）。cold 索引期间后台查询 **p50 2.2 ms，p95 75 ms，p99 171 ms，max 874 ms**；索引结束后 p95 < 7 ms。
7. **warm 的 317 ms 里 75% 花在反序列化和重建 `FileEntry`**（decode 97 ms + build/insert 139 ms），不是 walk（39 ms）。
8. **最大的单点浪费**：全量读取 889 MB 二进制（NUL 探测在**整文件读完之后**，`worker.rs:350-364`），以及 23,317 个“该语言没有符号提取”的文件也被完整读取 + blake3。
9. **不是瓶颈**：模糊匹配（1.4-4.9 ms/query）、redb 写入（176 ms/92 次事务）、walk（48 ms）、进度 IPC（cold 仅 25 个 Task 事件）。
10. 最高性价比修复：**读前过滤（二进制/无符号语言）** → **专用有界 rayon 池** → **修 workspace/多窗口的持久化漏洞**；若要彻底解决 cold，则要**缩小符号解析范围**（本项目 Go 占解析 51%，C/C++ 占 43%）。

---

## 1. 触发与流水线 (pipeline)

```
用户打开工程
└─ Host::open_project                        crates/ide-host/src/project.rs:63
   ├─ roots::detect + exclude_globs          project.rs:89-91, roots/detect.rs:189-205
   ├─ ProjectIndex::open                     project.rs:110  →  ide-index/src/lib.rs:90
   │  ├─ Store::open(index.redb)             lib.rs:103      →  store.rs:29
   │  │    stamp = schema + syntax fingerprint + canonical root   store.rs:48
   │  ├─ PathFilter::new(root, roots, excludes)  lib.rs:104   →  filter.rs:72
   │  └─ spawn thread "openclion-index"      lib.rs:123-131
   │       Shared::run(scan=true)            worker.rs:131
   │       ├─ load_persisted()               worker.rs:169
   │       │    store.load() (redb scan + postcard decode)  store.rs:86
   │       │    → FileEntry::new → IndexData::insert        data.rs:70,151
   │       └─ full_scan()                    worker.rs:211
   │            ├─ PathFilter::walk (ignore::WalkBuilder, parallel)  filter.rs:206
   │            ├─ todo = found - (mtime,size 相同)          worker.rs:234-242
   │            ├─ for chunk in todo.chunks(512)             worker.rs:255
   │            │    └─ index_files()                        worker.rs:296
   │            │         ├─ rayon par_iter → extract()      worker.rs:299-319
   │            │         │    fs::File::open + read_to_end  worker.rs:349-356
   │            │         │    blake3(bytes)                 worker.rs:357
   │            │         │    NUL sniff (first 8 KiB, after full read) worker.rs:362
   │            │         │    languages.detect + Syntax::tags        worker.rs:366-373
   │            │         │      Syntax::analyze → parse() + outline::tags  syntax/mod.rs:312-324, outline.rs:299-322
   │            │         ├─ data.write().insert(entries)    worker.rs:325-332  →  data.rs:151
   │            │         └─ store.apply(puts) postcard txns worker.rs:333      →  store.rs:105
   │            ├─ store.apply(durable) + IndexState(false)  worker.rs:273-283
   │            └─ (可选) progress Task every ≥250 ms        worker.rs:261-271
   ├─ Watcher::start  (同步遍历目录 + 每目录 inotify watch)  project.rs:184 → ide-vfs/src/watch.rs:152-190
   └─ LspManager::new / clangd 启动                          project.rs:120-122
```

查询路径：`ops/nav.rs:31-46` → `index.find_files/find_symbols` → `matching.rs:175/349` → `matching.rs:118` 对 `IndexData.files` 做 **rayon** 并行扫描（每次按键都全量扫）。

增量路径：watcher 批处理 → `ProjectIndex::update`（`lib.rs:184-193`）→ `Job::Changed/Removed`；按 mtime/size 过滤后再解析（`worker.rs:421-452`）；`set_scope` 触发 `Job::Rescan`（`lib.rs:137-145`）。

---

## 2. 索引范围（walk / ignore）

`PathFilter::walk`（`filter.rs:206-255`）使用 `ignore::WalkBuilder`：

- `.hidden(false)`：**连隐藏文件和隐藏目录都会扫**（`.idea`、`.vscode` 等只靠 `.gitignore` 排除）；
- `.parents(true)`、`git_ignore/git_global/git_exclude(true)`、`require_git(false)`：尊重父级和全局 gitignore；
- 永远跳过 `.git/.hg/.svn/.jj`（`filter.rs:14`）；
- 用户/默认 excludes（glob）：默认只有 **`**/.git`、`**/node_modules`、`**/target`**（`ide-core/src/settings.rs:560`）；
- CMake 工程自动排除 `/build`、`cmake-build-*`（`roots/detect.rs:189-205`），其它栈类似；
- **`third_party`、`vendor`、`build` 没有全局默认排除**：只有 `.gitignore` 或语言检测（CMake）命中才跳过；
- 额外 content roots（workspace 成员、Project Structure）也走同一套 walk。

对真实工程的实测：磁盘 163,774 个文件 → walk 后 45,965 个（`.gitignore` 排除了 `build*`、`cmake-build-debug`、`third_party`、`roachpb` 等）。

---

## 3. 读取、解析、存储

### 3.1 读取

- **逐文件 `fs::File::open` + `read_to_end` 到新的 `Vec<u8>`**（`worker.rs:349-356`），没有 mmap、没有批量 `read`、没有复用 buffer；
- 读完整个文件才做 `blake3::hash`（`worker.rs:357`，用于“mtime 变了但内容没变”时跳过重解析）；
- **读完整个文件才做 NUL 检测**（只看前 8 KiB，`worker.rs:362`）→ 二进制文件被完整读入；
- `MAX_SYMBOL_FILE = 2 MiB`，`size > MAX` 才跳过（`worker.rs:346`；正好 2 MiB 的文件仍被完整读取，边界 off-by-one）；
- 语言检测和 `Syntax::has_symbols` 在读取**之后**（`worker.rs:366-367`），所以没有符号提取能力的文件（plaintext/log/sql/markdown…）也会被完整读取 + 哈希。

### 3.2 解析

- `index_files` 以 **512 个文件为一 chunk**，chunk 内 `par_iter`，使用 **rayon 全局池**（本机 8 线程）；
- `Syntax::tags`（`ide-language/src/syntax/mod.rs:189`）先 `parse()`（`:334`，tree-sitter 整棵树），再 `outline::tags`（`outline.rs:299`）——它先构建**完整 outline 树**（`collect`，大量 `String`/`Vec`/`HashMap` 分配），再递归过滤出 tag，最后又 clone 一遍名字和 container；
- 进度/取消只在 chunk 边界和每个文件入口检查（`worker.rs:302-303`、`:256-258`），单个超大文件的解析无法中断。

### 3.3 存储

- 内存：`IndexData { files: Vec<Option<FileEntry>>, by_path: HashMap<PathBuf,u32>, ... }`（`data.rs:134`）；`FileEntry` 存 `PathBuf + Box<str> rel + Box<[Sym]>`，`Sym` 存 `Box<str> name + Option<Box<str>> container + Range + mask`（`data.rs:53-80`）；
- 持久化：**redb**（不是 sled/sqlite），`files` 表 key=相对路径，value=postcard `FileRecord`（`store.rs:8-11,86-124`）；
- 写入：每个 512 文件 chunk 一次非 durable 事务，扫描结束一次 durable（`worker.rs:333`、`:273`）；warm 无变化时仍会开 2 次写事务（`full_scan` 的 `persist(&[],&stale,false)` 和 `persist(&[],&[],true)`）。

### 3.4 何时全量重建

| 条件                              | 代码                                                             | 影响                                                |
| --------------------------------- | ---------------------------------------------------------------- | --------------------------------------------------- |
| workspace（`include_root=false`） | `filter.rs:164-170` `store_key` 直接返回 `None`                  | **成员永远不落盘**，每次启动全量解析（已复现）      |
| 第二个窗口/实例打开同一工程       | `store.rs:35-40` `DatabaseAlreadyOpen` → `None`                  | 退化为内存索引，**每次启动全量解析**（日志有 WARN） |
| schema / grammar / app 版本变化   | `store.rs:11,48`；`syntax/mod.rs:128` 指纹含 `CARGO_PKG_VERSION` | 清空索引表，cold                                    |
| Clear caches / 损坏 DB            | `caches.rs:26-30`、`store.rs:41-45`                              | cold                                                |
| `.gitignore`/excludes 改变        | `set_scope` → `Rescan`（`lib.rs:137`）                           | 重新 walk，但未变文件不重读                         |

---

## 4. 进度与 IPC

- `IndexState`（`IdeEvent::IndexState { indexing, files, symbols }`）在一次 `full_scan` 中只发开始/结束两次，增量更新（`process` 里 `touched`）再发一次（`worker.rs:120-126, 213-214, 274-283, 480-483`），**没有百分比字段**；
- `Task` 进度事件：每 chunk（512 文件）结束检查一次，且距上次 ≥250 ms 才发（`worker.rs:255-271`），`fraction = done / total`；
- 实测 cold 45,965 文件只发 **25 个 Task 事件**（约 2.5/s），**不存在 IPC 风暴**；
- 索引运行在 **独立 `std::thread`**，没有 `block_on`、没有在 tokio runtime 里直接跑 rayon（`lib.rs:123-131`）；
- 但 `load_persisted()` 发生在第一次 `IndexState(indexing=true)` **之前**，UI 在 warm 的 0.1-0.8 s 加载期间看不到任何索引状态；
- 应用日志 `indexed N files (M parsed, ...)` 里的 `M` 实际是 `total`（todo 数量），不是 parsed（`worker.rs:284-291`）——cold 时两者接近，warm 时 0，容易误读；日志计时也只覆盖 `full_scan`，**不含 `load_persisted`**（warm debug 日志 328 ms vs 实际 1.06 s）。

### 4.1 两个 “Indexing” 任务

- 本项目索引：标题硬编码 `"Indexing"`（`worker.rs:217, 264`），前端 `setIndexing` 又建一个 id=`"indexing"` 的任务，`fraction: null`（`src/workbench/tasks.ts:77-85`）；
- LSP `$/progress`：`handler.rs:57-68` 把 title/message/percentage 原样转成 `IdeEvent::Task`，clangd 后台索引的标题也是 “Indexing”，**带百分比、可取消**；
- `StatusBar` 只显示 `running.findLast(...)`（`StatusBar.tsx:96`），所以 clangd 的长任务会盖住本项目索引；Background Tasks 弹窗里可能同时出现两条 “Indexing”。
- 结论：用户抱怨的“Indexes 进度很久”**大概率是 clangd**（`--background-index --clang-tidy`，本工程 7,443 个 C/C++ 文件、6,182 个 `.idx`、58 MB 索引；日志里 clangd 初始化曾比工程打开晚 102 s）。

### 4.2 是否阻塞 async runtime

- 索引线程本身没有 `block_on`（独立 `std::thread`，`lib.rs:123-131`），也没有在 tokio runtime 里直接跑 rayon；
- **但查询路径会阻塞 tokio worker**：`find_files`/`find_symbols`（`ops/nav.rs:33-45`）在 `register` 的 `async move` 闭包里被**同步调用**（`nav.rs:157-160`），内部 `matching.rs:118-167` 直接 `par_iter`，没有用 `ops/mod.rs:93` 的 `blocking`/`spawn_blocking` 包起来。一次 45k 文件/520k 符号的扫描会占住一个 async worker 直到 rayon 结束；索引期间多次按键会同时占住多个 worker，IPC 队列变慢；
- 更直接的是 rayon 池竞争：索引 chunk 与查询共用同一个全局池，查询任务排在 512 文件解析任务后面。实测 cold 期间查询 p95 75 ms（release）/ 111 ms（debug），p99 171/261 ms，max 874/713 ms —— 这就是“索引后打开文件、导航慢”的直接来源之一。

---

## 5. 测量方法与结果

### 5.1 方法

- 临时在 `ide-index`/`ide-language` 加 `Instant` 计数（walk / read / hash / parse / outline-query / insert / persist / load），以及每语言文件数、字节数、解析时间；**测量后已全部还原**（`git diff` 只剩仓库原有改动）。
- 通过临时 `#[test] perf_probe` 调用真实 `ProjectIndex::open` + `wait_idle`；warm = 复用上一次留下的 `index.redb`，cold = 删除 cache 目录。
- 耗时用 `Instant`，峰值内存用 `/proc/self/status` 的 `VmHWM` 和 `/usr/bin/time -v` 交叉验证；线程数取 `Threads:`。
- 查询延迟：索引期间由后台线程每 5 ms 调一次 `find_symbols("Order", All, 20)`，记录 p50/p95/p99/max。
- 语料：合成工程（2,975 文件 / 13.9 MB）与真实工程（45,965 文件 / 1.26 GB）各一组；release 与 debug 各一组。
- 下表为隔离环境（无并发构建/clangd）的单次结果；同一场景重复 3 次的范围已在括号中标注（cold 9.9-10.6 s、warm 250-317 ms、合成 cold 0.59-0.79 s）。

### 5.2 主表（cold / warm）

| #   | 场景                      | profile   | 索引文件 | 实际解析 |                     总耗时 |     ms/文件 |       峰值 RSS | 线程 |
| --- | ------------------------- | --------- | -------: | -------: | -------------------------: | ----------: | -------------: | ---: |
| 1   | 合成工程 cold             | release   |    2,975 |    2,768 |                 **789 ms** |        0.27 |          40 MB |   11 |
| 2   | 合成工程 warm             | release   |    2,975 |        0 |         **49 ms**（27-49） |       0.016 |          29 MB |   11 |
| 3   | kwbasedb cold             | release   |   45,965 |   17,363 | **9,914 ms**（9.9-10.6 s） |       0.216 | **394-420 MB** |   11 |
| 4   | kwbasedb warm             | release   |   45,965 |        0 |      **317 ms**（250-317） |      0.0069 |     **155 MB** |   11 |
| 5   | kwbasedb cold             | debug     |   45,965 |   17,363 |              **12,127 ms** |       0.264 |         420 MB |   11 |
| 6   | kwbasedb warm             | debug     |   45,965 |        0 |               **1,062 ms** |       0.023 |         158 MB |   11 |
| 7   | kwbasedb cold（应用日志） | debug app |   45,950 |        — |              **17,460 ms** |       0.380 |              — |    — |
| 8   | kwbasedb warm（应用日志） | debug app |   45,952 |        0 |             **356-653 ms** | 0.008-0.014 |              — |    — |
| 9   | KaiwuDB cold（另一工程）  | debug app |   18,759 |        — |              **12,146 ms** |        0.65 |              — |    — |
| 10  | KaiwuDB warm（另一工程）  | debug app |   18,760 |        0 |             **125-132 ms** |       0.007 |              — |    — |

> 注：应用日志的 cold 行 “(45950 parsed)” 是 `total`（todo）而非 parsed（见 §4）；warm 行不含 `load_persisted`（约 0.4-0.8 s debug）。kwbasedb 的 197 GB 里有 11 GB `build/`、14 k 文件 `cmake-build-debug/` 等被 `.gitignore`/excludes 排除，实际索引 1.26 GB。
> 应用日志的 17.5 s 是在 CMake 构建、clangd 初始化并发时记录的，故高于隔离测量的 12.1 s（debug）/ 9.9 s（release）。

### 5.3 cold 阶段拆分（真实工程，release，45,965 文件）

| 阶段                         |                       计时 |  占 wall 约 | 说明                                        |
| ---------------------------- | -------------------------: | ----------: | ------------------------------------------- |
| walk（并行 `ignore`）        |                      48 ms |        0.5% | 45,965 个文件，19,722 个目录                |
| read（逐文件，多线程累加）   | 1,848 ms（各次 0.7-2.2 s） | ~0.4 s wall | 1.258 GB；其中 **889 MB 二进制**            |
| blake3                       |                     355 ms |           — | 1.258 GB，~3.5 GB/s                         |
| tree-sitter **parse**        |                  31,994 ms | ~7.1 s wall | 17,363 个文件                               |
| outline **query + collect**  |                  12,703 ms | ~2.8 s wall | 占 tags 的 27%                              |
| `FileEntry` 构建 + 插入      |                     101 ms |           — | 519,688 symbols                             |
| `FileRecord` clone（写盘前） |                      85 ms |           — | 每 chunk 复制全部记录                       |
| redb `apply`                 |                     176 ms |           — | 92 次事务                                   |
| **index 总计**               |               **9,815 ms** |           — | 进程 user 41.5 s，395-455% CPU（≈4.5/8 核） |

解析花费按语言（parse+query，CPU 计时）：

| 语言                                      | 文件数 |     字节 | parse+query |    占比 |
| ----------------------------------------- | -----: | -------: | ----------: | ------: |
| Go                                        |  7,566 | 101.5 MB |  **23.8 s** | **51%** |
| C++                                       |  4,679 |  42.8 MB |  **11.4 s** |     24% |
| C                                         |  2,764 |  22.5 MB |   **8.8 s** |     19% |
| Python / shell / Java / proto / cmake / … | ~2,350 |   ~15 MB |      ~2.8 s |      6% |

> 该工程是 Go + C/C++ 混合；纯 C/C++ 工程里 C/C++ 占比会接近 100%。

### 5.4 warm 拆分（真实工程，release，317 ms）

| 阶段                                   |       计时 | 说明                                |
| -------------------------------------- | ---------: | ----------------------------------- |
| redb 读取 + postcard 解码              |    96.6 ms | 45,965 条记录（34 MB DB）           |
| `FileEntry::new` + `IndexData::insert` |   139.4 ms | 519,688 个符号，`String → Box<str>` |
| walk + mtime/size 比较                 |    38.5 ms | todo=0                              |
| persist（2 次空事务）                  |     1.4 ms |                                     |
| 线程/其他                              |     ~40 ms |                                     |
| **合计**                               | **317 ms** | debug 为 1,062 ms（load 782 ms）    |

### 5.5 查询延迟

| 场景                      | find_files（50 条） | find_symbols（50 条）                                  |
| ------------------------- | ------------------- | ------------------------------------------------------ |
| warm release              | 0.28-0.74 ms        | 1.4-3.1 ms                                             |
| warm debug                | 0.7-1.6 ms          | 2.8-6.4 ms                                             |
| cold 索引**期间** release | —                   | p50 2.17 / p95 **75.0** / p99 **171** / max **874** ms |
| cold 索引**期间** debug   | —                   | p50 2.84 / p95 **111** / p99 **261** / max **713** ms  |

### 5.6 内存

- warm：155 MB / 45,965 文件 / 519,688 符号 ≈ **3.4 KB/file、305 B/symbol**；
- cold 峰值 414 MB（**2.7× warm**）：`Vec<u8>` 读缓冲（每线程最大 2 MB）+ `Vec<FileRecord>`（String）→ `FileEntry`（Box<str>）双份 + 写盘前 `FileRecord` clone + redb 写缓冲；
- `SyntaxCache` 有界（24 棵树，`cache.rs:25`）；`Documents` 只缓存打开文档；未发现其它无界文档缓存。
- 符号名字/容器各一次堆分配（`Box<str>`），`Option<Box<str>>` 每个符号再占 16 B；`Range` 16 B。

---

## 6. 瓶颈（按影响排序，附证据）

### B1. 解析 + outline query 占 cold 的 ~96%（最大头）

证据：`index_ms 9,815 ms` 中 `extract_ms 9,463 ms`；parse 31,994 ms + query 12,703 ms；walk 仅 48 ms。
`Syntax::tags` 为了取 tag 先构建**整棵 outline 树**（`outline.rs:299-322` 的 `collect` + `walk`），再做一次 clone；query 本身占 27%。
影响：cold 9.9 s；纯 C++ 工程同样会以 parse 为主。

### B2. 889 MB（71%）二进制被完整读取，NUL 检测太晚

证据：`files_binary=4,829`，`BINARY_BYTES=888,698,614`；`bytes_read=1,257,793,409`。Top 文件是 `tsbs-benchmark-data/.../count.stat-*`（每个正好 2,097,152 B，因 `>` 而非 `>=` 没被跳过）、`tsbs-compare/*/gocache/*-d`。
`extract` 先 `read_to_end`（`worker.rs:350`）再检查 NUL（`:362`）；`has_symbols` 也在读完以后（`:366`）。
影响：1.85 s 读取 + 355 ms 哈希 + 峰值 RSS 主要来源；在 page cache 冷时更严重。

### B3. 23,317 个“无符号语言”文件被读 + 哈希，却完全不解析

证据：`files_no_symbols=23,317`（plaintext 19,536 个 / 150 MB、log 1,333 / 18.8 MB、sql 1,116 / 8.8 MB、markdown 496、json/yaml/xml…）。它们合计约 187 MB。
如果按扩展名/首 8 KiB 预判语言并 `has_symbols` 提前跳过，可少读 **28,146 / 45,965（61%）个文件**、少读 **1,076 / 1,258 MB（86%）字节**。

### B4. warm 的 75% 在“重建内存结构”，而不是 walk

证据：warm 317 ms = decode 96.6 + build/insert 139.4 + walk 38.5。`store.load()` 先收集 `Vec<(String, FileRecord)>`（全部 String），再转成 `FileEntry`/`Sym`（又分配一遍）；`data.write()` 一次性插入 45,965 个 entry，阻塞并发查询。
debug 更差：load 782 ms（warm 总计 1.06 s），而应用日志只记录 `full_scan` 的 328 ms，造成“看起来只要 0.3 s”的错觉。

### B5. 索引与查询共用 rayon 全局池 → 索引期间查询 p95 75-111 ms

证据：`worker.rs:300` 的 `par_iter` 与 `matching.rs:118` 的 `scan` 都跑在全局池；`ops/nav.rs:157-160` 又在 async worker 里同步调用；cold 期间后台查询 p95 75/111 ms、max 874/713 ms；索引结束后 p95 < 7 ms。
这是“索引后打开文件/导航慢”的**代码级**原因（另一个是 clangd 本身忙）。

### B6. 持久化漏洞导致特定场景每次全量重建

- workspace：`filter.rs:164-170` 在 `include_root=false` 时 `store_key` 恒为 `None`；`index_files` 的 puts 为空（`worker.rs:323-324`），`store.apply` 什么也不写。实测第二次 open：`records=0, todo=20, files_parsed=20`（全部重解析）。
- 第二个窗口/实例：`store.rs:35-40` 返回 `None`；实测第二个 live index：`records=0, todo=20`；应用日志 2026-11-02T07:27:30 有 `index cache ... is in use; indexing in memory only`。
- 影响：这两类场景下每次启动都是 10-12 s cold，正好符合用户“每次打开都慢”的描述。

### B7. watcher 初始化在打开工程的关键路径上（0.45-2.7 s）

证据：`start_per_directory`（`watch.rs:152-190`）在调用线程同步 `watch_tree`，每个目录一次 inotify watch；应用日志 `opened ... [("roots",135),("index",242),("watcher",594),("git",0)]`、最坏 `watcher 2680 ms`。整树共 19,722 个目录（watcher 只注册未被 ignore 规则排除的目录，数量略少）。
影响：打开工程总时长 +0.5-2.7 s；与索引线程争 CPU/IO。同一日志里 `index` lap 为 34-519 ms（`ProjectIndex::open` 在调用线程同步打开 redb），watcher 是更大的那一项。

### B8. 进度/任务语义混乱（用户感知放大）

- 本项目索引任务无百分比（`tasks.ts:77-85`），LSP 任务同名 “Indexing” 且有百分比（`handler.rs:57-68`），`StatusBar` 显示最后一个（`StatusBar.tsx:96`）；
- `load_persisted` 阶段无状态事件，UI 在 warm 的前 0.1-0.8 s 空白；
- 日志把 `total` 写成 `parsed`（`worker.rs:284-291`），计时不含 load；
- clangd 的 `$/progress` 可跑几分钟（7,443 个 C/C++ TU、58 MB / 6,182 shard 索引），用户看到的“Indexes 很久”很可能来自这里。

### B9. 内存峰值 414 MB（cold）

证据：warm 155 MB → cold 414 MB。主要增量：1.26 GB 读取（每线程 2 MB buffer）、`Vec<FileRecord>` 与 `FileEntry` 双份字符串、`puts` clone（85 ms）、redb 写事务缓存。
影响：与 clangd、WebKit、页面缓存竞争；cold 后 RSS 不会立刻回落（分配到高水位）。

### 不是瓶颈（避免过度优化）

- 模糊匹配：warm 1.4-4.9 ms/query，`nucleo` + bitmask 已经很快；
- redb：176 ms / 92 事务；
- walk：48 ms；
- 进度 IPC：25 个事件，不是风暴；
- `memchr`/`fst`/`sled` 替换：当前查询延迟与 redb 耗时都不是主要矛盾，收益远小于解析/读取优化。

---

## 7. 修复清单（按 ROI 排序）

| #   | 修复                                                                                                                                                         | 位置                                                      | 预期收益                                                                                                | 风险                                           | 工作量 |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- | ---------------------------------------------- | ------ |
| 1   | **读前过滤**：按扩展名 + 首 8 KiB 判二进制/语言，`has_symbols` 为假就不 `read_to_end`；`MAX_SYMBOL_FILE` 用 `>=`                                             | `worker.rs:344-376`                                       | 少读 61% 文件 / 86% 字节（本工程 1.26 GB → 0.18 GB）；cold -0.5-1.5 s（冷盘更多）；峰值 RSS -150-200 MB | 低（未知扩展名多读 8 KiB 再决定）              | S      |
| 2   | **索引专用有界 rayon 池**（如 `cores-2=6`），查询继续用全局池                                                                                                | `worker.rs:296-334`                                       | 索引期间查询 p95 75-111 ms → <10 ms、p99 171-261 → <20；cold +0-15%                                     | 低                                             | S      |
| 3   | **修 workspace / 多窗口持久化**：`store_key` 支持 extra roots（带根名前缀）；同一进程内按 `(root,cache)` 复用 `Store`，或第二个窗口只读打开并 merge          | `filter.rs:164-170`、`store.rs:29-59`、`lib.rs:90-107`    | 这些场景下每次启动 10-12 s → ~0.3-1 s，**100% 消除全量重建**                                            | 中（key 映射、redb 生命周期）                  | M      |
| 4   | **缩小符号解析范围**：只解析源文件（C/C++/头文件，或再加 Go 等可配置）；或改为按需解析 / 用 clangd `workspace/symbol`                                        | `worker.rs:366-374`、`outline.rs:299`                     | 本工程：只解析 C/C++ → parse 46.8 s → ~20 s，cold ~5-6 s；完全去掉符号解析 → cold ~1.3-1.8 s（-80%+）   | 中高（Search Everywhere 符号覆盖、LSP 可用性） | M-L    |
| 5   | **降低 warm 重建成本**：持久化用 string arena / 每文件一段 symbol blob，`FileEntry` 惰性构造；避免 `FileRecord→Sym` 双份                                     | `store.rs:86-124`、`data.rs:70-122`                       | warm 317 → 150-200 ms（debug 1.06 s → ~0.5 s）；RSS -20-30%                                             | 中                                             | M      |
| 6   | **默认排除构建/依赖目录**：`build`、`cmake-build-*`、`out`、`dist`、`vendor`、`third_party`、`bower_components`；`MAX_SYMBOL_FILE` 2 MiB → 512 KiB           | `ide-core/src/settings.rs:560`、`worker.rs:18`            | 对没有 `.gitignore` 的 CMake 工程可少 20-60% 文件；本工程已被 gitignore 覆盖，收益小                    | 低（可被用户覆盖）                             | S      |
| 7   | **watcher 移出打开路径**：先返回，再在后台线程注册目录 watch 并 replay 期间事件                                                                              | `watch.rs:152-190`、`project.rs:184`                      | 打开工程 -0.5-2.5 s                                                                                     | 中（事件漏报）                                 | M      |
| 8   | **进度/任务语义**：本项目索引用独立 task id + 标题（如 “Project index”），`IndexState` 带百分比/阶段（load/scan/parse）；LSP 进度标为 “clangd”；合并两条任务 | `worker.rs:120-126`、`tasks.ts:77-85`、`handler.rs:57-68` | 用户能区分两个索引；warm 加载阶段可见；无性能收益                                                       | 低                                             | S      |
| 9   | **内存/字符串**：符号名/容器 interning 或 flat arena，去掉 `Option<Box<str>>`，`PathBuf` 只存一份                                                            | `data.rs:53-122`                                          | warm RSS 155 → 100-110 MB；cold 峰值同步下降                                                            | 中                                             | M      |
| 10  | **战略替代**：只保留文件名索引（walk，不读内容），C/C++ 符号搜索交给 clangd index；或对打开过的文件懒解析                                                    | 架构层                                                    | cold 从 9.9 s → ~0.1 s（只剩 walk + redb）；彻底消除重复索引                                            | 高（依赖 LSP、首次查询延迟）                   | L      |

---

## 8. Quick wins（1 天内可做）

1. **先探测再读**：`read` 8 KiB → NUL/语言/`has_symbols` → 不需要就 `continue`；`MAX_SYMBOL_FILE` 改 `>=`。本工程可少读 1,076 MB、28,146 个文件。（`worker.rs:344-376`）
2. **索引线程池隔离**：`rayon::ThreadPoolBuilder::new().num_threads(cores.saturating_sub(2))` 包住 `par_iter`，别让 45k 文件解析占满 UI 查询要用的池。（`worker.rs:299`）
3. **修 workspace 持久化**：`store_key` 对 extra roots 返回 `"<root-name>/<rel>"`，load 时按前缀还原；同一进程两个窗口共享 `Store`。（`filter.rs:164`、`store.rs:29`）
4. **默认排除 build/vendor**：只改默认值 + 文档，不动逻辑。（`settings.rs:560`）
5. **任务命名/百分比**：索引任务 id 改 `"project-index"`、标题 “Project index”，`fraction` 用 `done/total`，日志区分 load/scan，别和 clangd 的 “Indexing” 混在一起。（`tasks.ts:77`、`worker.rs:217`）
6. **别在 warm 上空写 redb**：`stale.is_empty() && todo.is_empty()` 时跳过两次 `persist`（本工程仅 1.4 ms，但能减少锁竞争/磨损）。（`worker.rs:250,273`）
7. **关闭工程不 join 索引线程**：`Drop` 里 `cancel + join` 会等当前 chunk/大文件解析完（最多数百 ms）；改成 cancel 后 detach，或把取消检查下沉到 tree-sitter 解析回调。（`lib.rs:291-298`）

---

## 9. 附：原始计数与复现

真实工程 cold（release，隔离 cache）：

```
files_found=45965  files_indexed=45965  symbols=519688
bytes_read=1257793409  files_binary=4829  binary_bytes=888698614
files_large(>2MiB)=456  files_no_symbols=23317  files_parsed=17363
parse_ms=31994  outline_query_ms=12703  (worker tags total 46810)
read_ms=1848  hash_ms=355  insert_ms=101  putclone_ms=85  persist_ms=176 (92 txns)
scan_ms=9914  index_ms=9815  rss_hwm=414 MB  threads=11
```

warm（同一 cache）：

```
records_loaded=45965  todo=0  parsed=0
load_decode_ms=96.6  load_insert_ms=139.4  walk_ms=38.5  persist_ms=1.4
scan_ms=317  rss_hwm=155 MB
```

应用日志（`~/.local/share/ai.l0ng.openclion.dev.wb879b047d6acc5a6/logs/openclion.2026-11-0*.log`）：

```
2026-11-02T03:24:36Z INFO ide_index::worker: indexed 18759 files (18759 parsed, 307003 symbols) in 12.145983583s
2026-11-02T03:28:25Z INFO ide_index::worker: indexed 18760 files (0 parsed, 307003 symbols) in 125.347344ms
2026-11-02T05:33:18Z INFO ide_index::worker: indexed 45950 files (45950 parsed, 519688 symbols) in 17.45982533s
2026-11-02T07:27:30Z WARN ide_index::store: index cache .../8bbce7cfe2816ebe/index.redb is in use; indexing in memory only
2026-11-03T01:14:00Z INFO ide_index::worker: indexed 45952 files (0 parsed, 519688 symbols) in 518.684157ms
2026-11-03T01:51:17Z INFO ide_index::worker: indexed 45952 files (0 parsed, 519688 symbols) in 328.032765ms
```

复现方式（已还原临时埋点）：

```bash
# warm（已有 cache）：317 ms release / 1.06 s debug
cargo test --release -p ide-index --no-run   # 用临时 perf_probe 驱动 ProjectIndex::open + wait_idle 后计时
# cold：删除 cache 目录后重跑
```

> 说明：为测量在 `crates/ide-index/src/worker.rs`、`crates/ide-index/src/lib.rs`、`crates/ide-language/src/syntax/mod.rs` 临时加过 `Instant` 计数与 `perf_probe`，调查结束后已全部还原；`git diff` 仅保留仓库原有改动。
