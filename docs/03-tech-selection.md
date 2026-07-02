# 03 — 技术选型

> 每个选型记录"选什么 + 为何 + 备选 + 风险"。调研基于 2026-07 的生态状态。

## 全文引擎：Tantivy 0.24

**选**：[Tantivy](https://github.com/quickwit-oss/tantivy) 0.24
**为何**：Lucene 的 Rust 重写，性能与 Lucene 相当，纯 Rust 无 JVM，适合桌面嵌入。社区报告"insanely fast"。
**备选**：Apache Lucene（需 JVM，重）；Meilisearch（服务端导向）；SQLite FTS5（中文分词弱）
**风险**：schema 不可变（需设计期定死）；多 writer 不安全（单 worker 约束）

## 中文分词：jieba-rs + 自写 Tokenizer

**选**：[jieba-rs](https://crates.io/crates/jieba-rs) + 自定义 Tantivy Tokenizer
**为何**：Python jieba 的 Rust 移植，活跃。不直接用 `tantivy-jieba`（版本滞后，跟踪 tantivy 0.20），而是参考其实现自写 Tokenizer，版本可控。
**备选**：cang-jie（需 Rust 2024）；tantivy-jieba（版本错位风险）
**风险**：未登录词召回有限，需自定义词典；中英混排切分不均，考虑 ngram 兜底

## PDF：pdfium-render（静态链接）

**选**：[pdfium-render](https://github.com/ajrcarey/pdfium-render)，静态链接 Google PDFium
**为何**：纯 Rust（pdf-extract/lopdf）对 CID 字体（中文）乱码，质量差。PDFium 是 Chromium 同款，中文支持成熟。静态链接后用户无运行时依赖。
**备选**：pdf-extract（纯 Rust，中文差）；lopdf（底层，需自写解码）
**风险**：引入 C++ 依赖，三端 CI 需配置预编译 PDFium binary；少数 PDF 无 ToUnicode CMap 仍提取不全（PDF 规范限制，非 Rust 独有）

## Office 现代格式

**docx**：docx-rs 或 [ooxmlsdk](https://crates.io/crates/ooxmlsdk)
**xlsx/xls/csv**：[calamine](https://github.com/tafia/calamine)（事实标准，纯 Rust，含老 xls）
**pptx**：ooxmlsdk
**为何**：OOXML 是开放标准，Rust 有成熟解析
**风险**：pptx 生态较薄，文本提取需验证

## Office 老格式 .doc/.ppt：v1 不支持

**决策**：不实现，检测到提示转换为 .docx/.pptx
**理由**：Rust 生态无成熟纯解析器（cfb 只给容器，WordBinary 需逆向数万行）；AnyTXT/DocFetcher 用 POI(Java)，不可接受 JVM
**缓解**：UI 明确提示；文档说明可用 LibreOffice headless 预转换

## Markdown：pulldown-cmark

**选**：[pulldown-cmark](https://crates.io/crates/pulldown-cmark)
**为何**：事实标准，CommonMark 高合规，pull parser 性能好
**风险**：无

## HTML：scraper + lol_html

**选**：[scraper](https://crates.io/crates/scraper)（正文提取）+ [lol_html](https://crates.io/crates/lol_html)（大文件流式）
**为何**：scraper 基于 html5ever（浏览器级），CSS 选择器方便提取 body 去 script/style。lol_html 适合大 HTML 流式处理。
**风险**：无

## 编码检测：encoding_rs + chardetng

**选**：[encoding_rs](https://docs.rs/encoding_rs/) + [chardetng](https://crates.io/crates/chardetng)
**为何**：WHATWG 标准实现，Firefox 同款 chardetng 比 ICU 准。处理 GBK/GB18030/Big5 中文遗留编码。
**风险**：无

## ePub：epub crate

**选**：epub crate（zip + xhtml）
**为何**：ePub 本质 zip+xhtml，crate 可用
**风险**：中低，需验证复杂 ePub

## OCR：kreuzberg-tesseract（feature gate）

**选**：[kreuzberg-tesseract](https://crates.io/crates/kreuzberg-tesseract)（内置静态编译），feature gate 默认关
**为何**：Tesseract 是 OCR 事实标准，无纯 Rust 替代。内置静态编译消除运行时依赖。语言包按需下载避免默认包膨胀。
**备选**：leptess（需系统装 libtesseract，不适合分发）；tesseract-rs
**风险**：包体增大（启用时）；跨平台分发训练数据；OCR 质量依赖原图清晰度

## 文件监听：notify + notify-debouncer-full

**选**：[notify](https://github.com/notify-rs/notify) 6.x + [notify-debouncer-full](https://crates.io/crates/notify-debouncer-full)
**为何**：跨平台（inotify/FSEvents/ReadDirectoryChangesW）事实标准。debouncer-full 智能合并事件 + rename 配对。
**风险**：macOS FSEvents 不精确（合并/丢失事件），需当"重扫提示"用 + mtime 校验兜底

## 元数据：SQLite（rusqlite）

**选**：[rusqlite](https://crates.io/crates/rusqlite)
**为何**：DocFetcher 用 Java 序列化（跨版本脆弱 + StackOverflow 风险）。SQLite 跨平台、可查询、Rust 生态成熟。存 tree_index（{path, mtime, parser}）。
**风险**：无

## 其他

| 用途 | crate |
|---|---|
| mime 检测 | infer（魔数） |
| 文件遍历 | walkdir |
| 归档穿透 | zip / tar / sevenz-rust |
| 并发 | crossbeam-channel / parking_lot |
| 日志 | tracing |
| 错误 | thiserror + anyhow |
| 异步 | tokio |
| 前端框架 | Vue 3 + TypeScript + Vite |
| 桌面壳 | Tauri 2 |
| UI 组件 | Element Plus |
| 状态管理 | Pinia |

## 受控原生依赖汇总

| 依赖 | 用途 | 分发策略 |
|---|---|---|
| PDFium | PDF 中文解析 | 构建时静态链接（pdfium-render static-bindings feature） |
| Tesseract + Leptonica | OCR | feature gate，kreuzberg-tesseract 内置静态编译；语言包按需下载 |
