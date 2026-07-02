# pivotsearch — Project Context

> 项目上下文宪法，执行期不变。汇总项目概述/能力清单/约束/技术栈/合规红线。

## 项目概述

`pivotsearch` 是跨平台（Windows/macOS/Linux）本地全文搜索桌面应用，定位为 AnyTXT 的开源替代品。借鉴 DocFetcher（Java/Lucene）的核心设计逻辑，用现代 Rust 组件栈（Tantivy + Tauri）做工程整合，补齐 DocFetcher 缺失的 OCR 能力。

完全净室重写——不复制 DocFetcher 任何 Java 代码/类名/标识符，只复刻公共设计模式（mtime 增量、unseenDocs diff、Parser 注册表）。

## 核心定位

**白空间**：开源 + 三端 + 内容搜索 + 活跃维护 + OCR。市面无工具同时满足这五点（AnyTXT 闭源 Win-only、DocFetcher 停滞、Everything 仅文件名、Recoll Linux 优先）。pivotsearch 填补这个空白。

## 单一架构（非双端）

与 cloudpivot 不同，pivotsearch 是**单一桌面应用**：
- Rust 核心引擎（workspace 内 9 个 crate）
- Tauri 2 前端壳（Vue 3）
- 三端打包（Win/macOS/Linux），无服务端形态

## 8 个 Capability（v1 范围）

1. `core-index-schema` — Tantivy schema 八字段 + uid 主键 + Document 组装 + upsert 语义
2. `parser-registry` — Parser trait + 注册表 + 两级选择 + 10 类格式解析器
3. `incremental-index` — mtime 比对 + unseenDocs 文件树 diff + 归档整体跳过 + SQLite 持久化
4. `file-watcher` — notify 跨平台监听 + 防抖 + 事件过滤 + mtime 二次校验
5. `indexing-queue` — 单工作线程 + Task 状态机 + UPDATE/REBUILD + 多索引并发
6. `search-engine` — 多索引合并 + 查询解析（AND 默认）+ 分页 + SnippetGenerator 高亮 + 中断
7. `ocr-pipeline` — Tesseract 集成（feature gate）+ 图片/扫描件识别 + 语言包按需下载
8. `desktop-ui` — Tauri 2 + 即时搜索 + 结果高亮 + 预览 + 索引管理 + 筛选器 + 设置

## 关键约束

### 依赖方向铁律
- `core` 编排层只依赖 `contracts` trait，绝不 import 具体实现
- `contracts` 是依赖终点
- 只有 `cli`/`src-tauri` 能 import 具体实现（组装根）

### Tantivy 关键约束
- schema 不可变（变更需 reindex）
- 单 writer 强约束（同索引目录同时只能一个 writer）
- 无原生 upsert（delete_term + add）
- 无 term-vector offset（SnippetGenerator 替代双高亮）

### 受控原生依赖（用户已确认）
- PDFium（pdfium-render 静态链接）：保证中文 PDF 质量
- Tesseract（kreuzberg-tesseract，feature gate 可选）：OCR
- .doc/.ppt v1 不支持，提示转换

### 净室红线
禁止复制 DocFetcher Java 源码/类名/标识符。产出后 grep 检查零残留。

## 技术栈

### Rust 后端
- 全文引擎：tantivy 0.24
- 中文分词：jieba-rs + 自写 Tokenizer
- PDF：pdfium-render（静态链接）
- Office：calamine（xlsx/xls/csv）、docx-rs/ooxmlsdk（docx）、ooxmlsdk（pptx）
- Markdown：pulldown-cmark
- HTML：scraper / lol_html
- 编码：encoding_rs + chardetng
- ePub：epub crate
- OCR：kreuzberg-tesseract（feature gate）
- mime：infer
- 监听：notify + notify-debouncer-full
- 遍历：walkdir
- 归档：zip / tar / sevenz-rust
- 元数据：rusqlite（SQLite）
- 并发：crossbeam-channel / parking_lot
- 日志：tracing
- 错误：thiserror + anyhow
- 异步：tokio

### 前端
- Tauri 2 + Vue 3 + TypeScript + Vite
- Element Plus + Pinia

## 合规红线

净室重写。禁止复用 DocFetcher 的代码/Prompt 原文/类名/标识符。"复刻" = 复刻设计逻辑（mtime 增量、unseenDocs diff、Parser 注册表两级选择等公共模式）。许可证 Apache-2.0。
