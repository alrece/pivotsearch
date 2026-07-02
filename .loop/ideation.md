# pivotsearch Ideation (Phase 1 产物)

本文件是 Loop Engineering Phase 1（ideate）的产物指针，汇总项目构想的核心理据与决策。

## 一句话定位

**AnyTXT 的开源替代品**：跨平台（Win/macOS/Linux）、本地优先、离线可用的全文搜索桌面应用，补齐 DocFetcher 停滞后留下的开源空白。

## 问题陈述

本地全文搜索市场存在明确的"三难困境"：

| 维度 | AnyTXT | DocFetcher（免费版） | Everything | Recoll |
|---|---|---|---|---|
| 开源 | ❌ 闭源 | ✅ GPL | ❌ 闭源 | ✅ GPL |
| 三端 | ❌ 仅 Win | ✅ 但 Java 重 | ❌ 仅 Win | ⚠️ Linux 优先 |
| 内容搜索 | ✅ | ✅ | ❌ 仅文件名 | ✅ |
| 活跃维护 | ✅ 但退化 | ❌ 停滞(2023-10) | ✅ | ⚠️ |
| OCR | ✅ | ❌ 无 | ❌ | ⚠️ |

没有任何一个同时满足"开源 + 三端 + 内容搜索 + 活跃 + OCR"。pivotsearch 填补这个空白。

## 前提（已确认）

1. **本地优先**：所有索引/搜索/OCR 完全本地，不发送数据到云端（OCR 语言包下载除外）
2. **现代栈**：Rust（Tantivy + Tauri），不用 Java/Electron，保证体积小、启动快
3. **设计借鉴**：复刻 DocFetcher 的核心设计逻辑（mtime 增量、Parser 注册表），不复制代码（净室红线）
4. **中文优先**：内置 jieba 分词，处理 GBK/Big5 遗留编码
5. **三端原生**：一份代码三端打包（Tauri 2）

## 推荐路径

**Approach：Rust 现代组件栈工程整合**

- 全文引擎：Tantivy 0.24（Lucene 灵感重写的 Rust 版）
- 解析层：纯 Rust crate 组合（calamine/pulldown-cmark/scraper...）+ pdfium-render(PDF) + 可选 Tesseract(OCR)
- 监听：notify + notify-debouncer-full
- 元数据：SQLite（替代 Java 序列化）
- UI：Tauri 2 + Vue 3

详见完整实施计划（已通过 ExitPlanMode 审批）。

## 关键技术决策

1. **PDF 用 pdfium-render 而非 pdf-extract**：纯 Rust 对 CID 字体（中文）乱码，PDFium 是 Chromium 同款保证质量
2. **OCR 可选 + 语言包按需下载**：Tesseract 静态编译会让默认包膨胀，做成 feature gate
3. **老 Office .doc/.ppt v1 不支持**：Rust 无成熟纯解析器，逆向不现实，UI 提示转换
4. **jieba-rs 自写 Tantivy Tokenizer**：不依赖 tantivy-jieba 版本同步
5. **SQLite 存 tree_index**：替代 DocFetcher 的 Java 序列化，跨平台可查询

## 工程方法论

参照 cloudpivot 项目（`/Users/alrece/GitHub/cloudpivot`）的 Loop Engineering 框架：
- `.loop/` 宏观闭环状态机（7-Phase）
- `openspec/` 规格驱动开发（Requirement + WHEN/THEN Scenario）
- `.planning/` GSD 规划执行（Roadmap → Phase → Plan → Task）
- 依赖方向铁律 + 净室红线 + 对抗门

## 8 个 Capability（v1 范围）

1. `core-index-schema` — Tantivy schema + uid + Document 组装
2. `parser-registry` — Parser 注册表 + 两级选择 + 各格式解析器
3. `incremental-index` — mtime 比对 + 文件树 diff + SQLite 元数据持久化
4. `file-watcher` — notify + 防抖 + 事件过滤 + mtime 校验
5. `indexing-queue` — 单工作线程 + Task 状态机 + 多索引并发
6. `search-engine` — 多索引合并 + 查询解析 + 分页 + 高亮
7. `ocr-pipeline` — Tesseract 集成（feature gate 可选）
8. `desktop-ui` — Tauri 2 前端

## 参考文档

- 设计逻辑蓝本：`/Users/alrece/Github/DocFetcher`（净室，只借鉴设计）
- 工程方法论参考：`/Users/alrece/GitHub/cloudpivot`
- 完整实施计划：本会话 ExitPlanMode 审批内容
- 技术选型报告：`docs/03-tech-selection.md`
- 架构设计：`docs/02-architecture.md`
