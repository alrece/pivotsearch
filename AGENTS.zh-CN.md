# AGENTS.md


[English](AGENTS.md) | 中文
本文件为本仓库的协作基线文档，面向代码代理和实施协作者提供统一工作指引。如果与其他协作文档（CLAUDE.md 等）冲突，以本文件为准。

## 项目概述

`pivotsearch` 是一个跨平台（Windows / macOS / Linux）的本地全文搜索桌面应用，定位为 AnyTXT Searcher 的开源替代品。它借鉴 DocFetcher（Java/Lucene）的核心设计逻辑——mtime 驱动的增量索引、文件树 diff、Parser 注册表、一索引根一目录——用现代 Rust 组件栈（Tantivy + Tauri）做工程整合，并补齐 DocFetcher 缺失的 OCR 能力。

核心理念：

- **本地优先，离线可用**：所有索引与搜索完全在用户机器上完成，不发送任何数据到外部服务（OCR 语言包下载除外）
- **增量索引**：基于 mtime + 文件树 diff 的增量算法，配合文件系统监听，实现"改动即更新"
- **多格式解析**：PDF / Office(docx/xlsx/pptx) / Markdown / HTML / 纯文本 / ePub / 源代码，以纯 Rust 为主体，对 PDF 中文与 OCR 受控引入原生依赖
- **中文友好**：内置 jieba 分词，处理好中英混排、GBK/Big5 遗留编码

当前仓库规划的核心能力（8 个 capability）：

- 索引引擎：Tantivy schema 设计、uid 主键、Document 组装（`core-index-schema`）
- 解析层：Parser 注册表 + 两级选择（mime 优先 / 扩展名 fallback）+ 各格式解析器（`parser-registry`）
- 增量索引：mtime 比对 + unseenDocs 文件树 diff + 归档整体跳过 + SQLite 元数据持久化（`incremental-index`）
- 文件监听：notify 跨平台监听 + 防抖 + 事件过滤 + mtime 二次校验去噪（`file-watcher`）
- 索引队列：单工作线程任务队列 + Task 状态机 + 多索引并发（`indexing-queue`）
- 搜索引擎：多索引合并 + 查询解析 + 分页 + 高亮 + 中断（`search-engine`）
- OCR 管道：Tesseract 集成 + 图片/扫描件识别 + 语言包按需下载（`ocr-pipeline`，feature gate 可选）
- 桌面 UI：Tauri 2 前端 + 即时搜索 + 结果高亮 + 预览面板 + 索引管理（`desktop-ui`）

## AI 协作入口

- `AGENTS.md`：仓库级事实基线，统一维护架构事实、开发约定和交付原则
- `CLAUDE.md`：Claude Code 专属快速入口，只保留使用方式和命令索引
- `.loop/`：Loop Engineering 宏观闭环状态（详见下文）

维护约定：

- 涉及仓库事实、目录结构、命令、开发规则的修改，优先更新 `AGENTS.md`
- 涉及 Claude 使用方式的修改，再更新 `CLAUDE.md`
- 如果 `AGENTS.md` 与其他协作文档冲突，以 `AGENTS.md` 为准

## Loop Engineering 协作规范

本节仅在用户显式执行 `/loop:*` 命令时生效，用来约束代理如何在本仓库中使用 `Loop Engineering` skill。

### 铁律（MUST）

- 以 `.loop/STATE.yaml` 为单一事实源；当前 `phase`、`step`、`iteration`、`blocker`、phase 状态均从该文件实时读取，不能臆测或复用旧缓存
- 每次执行 `/loop:run` 都重新读取 `STATE.yaml`
- 有产出的环节 `spec` / `design` / `plan` / `execute` / `review` / `ship` 在产出后都必须执行 `adversarial_gate`（确定性检查 + 净室 grep + 编译验证）
- `execute` 必须是 plan 级逐条检查，而不是阶段级一次性放行
- 所有状态变更都要原子写回 `.loop/STATE.yaml`（先写 `.tmp` 再 rename），并追加 `.loop/timeline.jsonl`，保证可审计、可恢复
- 所有自动决策都要追加记录到 `.loop/decisions.jsonl`
- 所有面向用户的输出使用简体中文；代码、命令、路径、配置 key、函数名与专有名词保留原文

### 禁止事项（MUST NOT）

- 不得让对抗检查未通过的产物进入下游
- 不得跳过 Gate 5（loop 与 GSD `.planning/STATE.md` 状态一致性校验）
- 不得自行加 `--force`；只有用户显式传入时才能跳过安全门
- 不得破坏 `STATE.yaml` 既有结构（`phase_status`、`artifacts`、`history` 等字段）

### 安全门（Gate 1-5）

- **Gate 1**：`STATE.yaml` 当前 phase 的 `blockers` 非空时停止
- **Gate 2**：上一 phase 的 artifacts 必须在磁盘真实存在
- **Gate 3**：从 Phase 4 推进到 Phase 5 时，`qa-report.md` 不得含 `FAIL`
- **Gate 4**：`.loop/adversarial/last-verdict.json` 必须 `passed=true`
- **Gate 5**：`execute` 推进前，必须校验 loop 状态与 GSD `STATE.md` 一致

## 技术栈

### Rust 后端（核心）

- 全文引擎：`tantivy` 0.24（倒排索引 + 查询）
- 中文分词：`jieba-rs` + 自写 Tantivy Tokenizer（不依赖 `tantivy-jieba` 版本同步）
- PDF：`pdfium-render`（静态链接 Google PDFium，保证中文质量）
- Office：`calamine`（xlsx/xls/csv）、`docx-rs` 或 `ooxmlsdk`（docx）、`ooxmlsdk`（pptx）
- Markdown：`pulldown-cmark`
- HTML：`scraper`（正文提取）、`lol_html`（大文件流式）
- 纯文本/编码：`encoding_rs` + `chardetng`（GBK/Big5 检测）
- ePub：`epub` crate（zip + xhtml）
- OCR：`kreuzberg-tesseract`（内置静态编译，feature gate 可选）
- mime 检测：`infer`（魔数）
- 文件监听：`notify` 6.x + `notify-debouncer-full`
- 文件遍历：`walkdir`
- 归档穿透：`zip` / `tar` / `sevenz-rust`
- 元数据存储：`rusqlite`（SQLite，替代 Java 序列化）
- 并发：`crossbeam-channel` / `parking_lot`
- 日志：`tracing`
- 错误：`thiserror` + `anyhow`
- 异步：`tokio`

### 前端（Tauri 2）

- Vue 3 + TypeScript + Vite
- Element Plus 组件库
- Pinia 状态管理
- 即时搜索（debounce 输入）+ 虚拟列表 + 高亮渲染 + 预览面板

### 不支持（v1 范围外）

- **MS Office 老格式 .doc / .ppt**：Rust 生态无成熟纯 Rust 解析器，逆向二进制格式不现实；检测到时 UI 提示用户转换为 .docx / .pptx
- **.xls**：可用 `calamine`，在范围内

## 运行约束（铁律）

### 依赖方向铁律

- 编排层（`crates/core`）只依赖 `crates/contracts` 的 trait 定义，**绝不 import 具体实现**（parser/index/watcher/queue/search/ocr）
- `crates/contracts` 是**依赖终点**（不依赖任何其他内部 crate）
- 唯一"知道一切"、能 import 具体实现的层是 `crates/cli` 与 `src-tauri/`（组装根）
- 各能力 crate（parser/index/watcher/queue/search/ocr）互不依赖，只通过 contracts trait 交互

```
cli / src-tauri  →  core  →  contracts  ←  parser / index / watcher / queue / search / ocr
```

### Tantivy 关键约束（与 Lucene 的差异）

- **schema 不可变**：Tantivy schema 启动时一次定死；字段演进需要 reindex，没有 Lucene 的灵活性。任何 schema 变更必须在 spec 显式评估 reindex 成本
- **单 writer 强约束**：同一索引目录同时只能一个 writer，单工作线程模型（`indexing-queue`）在 Tantivy 下是**强约束**，不要改成多 worker 并发写同一索引。不同索引根可各自独立 writer 并发
- **无原生 upsert**：`update` = `delete_term(uid)` + `add_document`，且 `delete_term` 在 commit 后才对 reader 生效——update 完必须 reopen reader
- **无 term-vector offset**：Lucene 的 FastVectorHighlighter 双路径在 Tantivy 里合一，统一用 `SnippetGenerator` 重切文本

### 净室红线

`pivotsearch` 是对 DocFetcher（GPL v3）的**设计逻辑复刻**，不是代码复制。严格遵守：

- **禁止**复制 DocFetcher 的任何 Java 源码、类名、标识符、注释文本
- **允许**借鉴其公共设计模式：mtime 驱动增量、unseenDocs 集合 diff、Parser 注册表 + 两级选择、一索引根一目录、uid 主键、parser 名入索引字段
- 每次产出后对抗门必须跑净室 grep：`grep -ri "docfetcher\|DocFetcher\|net.sourceforge.docfetcher" crates/ src/ src-tauri/`，命中即未通过

### 原生依赖分发

- **PDFium**：构建时静态链接（`pdfium-render` 的 `static-bindings` feature），用户机器无运行时依赖
- **Tesseract**：`kreuzberg-tesseract` 内置静态编译；语言包（`chi_sim`/`eng` 等）做成首次启用 OCR 时按需下载，不进默认安装包
- **三端优先**：所有原生依赖必须 Windows/macOS/Linux 三端可打包，任一平台不支持需在 spec 显式标注 `platform-limit`

## 目录结构

```
pivotsearch/
├── Cargo.toml                    # workspace 根
├── crates/
│   ├── contracts/                # ★依赖终点：trait（Parser/Indexer/Searcher/Watcher）+ 数据结构 + 错误类型
│   ├── parser/                   # 解析层：Parser 注册表 + 各格式实现
│   ├── index/                    # 索引层：Tantivy 封装 + schema + Document 组装 + 增量 + tree_index（SQLite）
│   ├── watcher/                  # 监听层：notify + 防抖 + 事件过滤 + mtime 校验
│   ├── queue/                    # 任务队列：单工作线程 + Task 状态机 + 多索引并发
│   ├── search/                   # 查询层：多索引合并 + 查询解析 + 分页 + 高亮
│   ├── ocr/                      # OCR 层：Tesseract 集成（feature gate，可选）
│   ├── core/                     # 编排层：组装上述模块，提供 PivotsearchEngine 总入口
│   └── cli/                      # CLI binary（开发期调试）
├── src-tauri/                    # Tauri Rust 后端
├── src/                          # Vue 3 前端
│   ├── views/ components/ stores/
├── docs/                         # 技术文档
├── .loop/ openspec/ .planning/   # 工程方法论
├── AGENTS.md CLAUDE.md DESIGN.md README.md Makefile
```

## 常用命令

```bash
# 构建 / 检查 / 测试
cargo check                       # 全 workspace 编译检查
cargo build --release             # 发布构建
cargo test                        # 全 workspace 测试
cargo test -p pivotsearch-index   # 单 crate 测试
cargo clippy --all-targets -- -D warnings  # lint

# 前端
pnpm install                      # 安装前端依赖
pnpm dev                          # Vite 开发服务器
pnpm build                        # 前端构建

# Tauri
cargo tauri dev                   # 桌面端开发模式
cargo tauri build                 # 桌面端打包

# 净室合规检查（对抗门）
grep -ri "docfetcher\|net.sourceforge.docfetcher" crates/ src/ src-tauri/ && echo "FAIL: 命中 DocFetcher 标识" || echo "PASS: 净室合规"

# Loop Engineering（显式调用时才介入）
# /loop:init   /loop:status   /loop:run --next [--auto]   /loop:retro
```
