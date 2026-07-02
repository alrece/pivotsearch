# pivotsearch

> 跨平台本地全文搜索桌面应用 · AnyTXT 的开源替代 · 三端可用（Windows / macOS / Linux）

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![CI](https://github.com/yourname/pivotsearch/actions/workflows/ci.yml/badge.svg)](.github/workflows/ci.yml)

`pivotsearch` 是一个完全本地运行、离线可用的全文搜索工具。它索引你硬盘上的文档内容（PDF / Word / Excel / PowerPoint / Markdown / HTML / 纯文本 / ePub / 源代码），让你像用 Google 一样秒级搜索本地文件——不发送任何数据到云端。

## 核心特性

- ⚡ **秒级检索** — 基于 Tantivy 倒排索引，查询毫秒级返回
- 🔄 **增量后台索引** — 文件系统监听 + mtime 比对，改动即更新，不用全量重建
- 📄 **多格式解析** — PDF / Office(docx/xlsx/pptx) / Markdown / HTML / TXT / ePub / 源代码 / 归档(zip/tar 穿透)
- 🇨🇳 **中文友好** — 内置 jieba 分词 + 停用词过滤，处理好中英混排和 GBK/Big5 遗留编码
- 🗂️ **多索引管理** — 支持多个索引根，跨目录合并搜索
- 🖥️ **三端原生** — Windows / macOS / Linux 同一份体验（Tauri 2，体积小、启动快）
- 🔒 **完全离线** — 索引、搜索全部本地完成，零数据外泄

## 技术栈

- **后端**：Rust + [Tantivy](https://github.com/quickwit-oss/tantivy)（全文引擎）+ jieba-rs（中文分词）+ notify（文件监听）+ SQLite（元数据）
- **前端**：[Tauri 2](https://v2.tauri.app/) + Vue 3 + TypeScript + Element Plus
- **PDF**：pdfium-render（动态链接 Google PDFium）
- **OCR**：可选（feature gate，基于 Tesseract）

详见 [docs/03-tech-selection.md](docs/03-tech-selection.md)。

## 安装

### 从源码构建

```bash
# 前置：Rust 1.75+、Node 20+、pnpm
git clone https://github.com/yourname/pivotsearch.git
cd pivotsearch
pnpm install

# 开发模式（启动 Tauri 桌面应用）
pnpm tauri dev

# 打包
pnpm tauri build
```

### CLI 模式（开发调试）

```bash
cargo run --bin pivotsearch -- index /path/to/docs          # 索引一个目录
cargo run --bin pivotsearch -- search "关键词"               # 搜索
```

## 使用

### 桌面应用

启动后：
1. 点击"索引管理"，添加要索引的目录
2. 后台自动索引（进度实时显示）
3. 在搜索框输入关键词，即时出结果

### CLI

```bash
pivotsearch index /path/to/docs                    # 索引目录
pivotsearch search "季度报告"                       # 搜索中文
pivotsearch search "React"                          # 搜索英文
```

## 支持的格式

| 格式 | 扩展名 | 支持 |
|---|---|---|
| PDF | .pdf | ✅（需 PDFium 库） |
| Word | .docx | ✅ |
| Excel | .xlsx / .xls / .csv | ✅ |
| PowerPoint | .pptx | ✅ |
| Markdown | .md / .markdown | ✅ |
| HTML | .html / .htm | ✅ |
| 纯文本/源代码 | .txt / .rs / .py / .js / .json / .yaml 等 | ✅ |
| ePub | .epub | ✅ |
| 归档 | .zip / .tar / .tar.gz（穿透索引） | ✅ |
| 图片（OCR） | .jpg / .png / .tiff | ⚠️ 可选（feature gate） |
| Word 老格式 | .doc | ❌ 请转 .docx |
| PowerPoint 老格式 | .ppt | ❌ 请转 .pptx |

## 架构

```
crates/
├── contracts/    依赖终点：Parser/Indexer/Searcher/Watcher trait
├── parser/       解析层：9 格式 parser + 注册表两级选择
├── index/        索引层：Tantivy schema + 增量算法 + SQLite tree_index
├── watcher/      监听层：notify + 防抖 + 事件过滤
├── queue/        队列层：单工作线程 + Task 状态机
├── search/       查询层：单索引 + 多索引合并 + 高亮
├── ocr/          OCR（feature gate 可选）
├── core/         编排层（只依赖 contracts）
└── cli/          CLI（组装根）
src-tauri/        Tauri 桌面后端（命令桥接）
src/              Vue 3 前端
```

依赖方向铁律：编排层只依赖 contracts trait，具体实现互不依赖。详见 [AGENTS.md](AGENTS.md)。

## 项目状态

- ✅ Phase 0-4 完成：脚手架 / 核心索引 / 增量监听 / 全格式 / 桌面 UI
- 🔄 Phase 5 进行中：三端打包 CI + 文档 + 中文调优
- 33+ 单元测试通过，净室合规

状态详见 [`.loop/STATE.yaml`](.loop/STATE.yaml)，路线图见 [`.planning/ROADMAP.md`](.planning/ROADMAP.md)。

## 开发

```bash
cargo check && cargo test          # 后端编译 + 测试
pnpm build                         # 前端构建
make cleanroom                     # 净室合规检查
make deps-check                    # 依赖方向验证
pnpm tauri dev                     # 桌面端开发
```

项目用 Loop Engineering 方法论管理（`.loop/` + `openspec/` + `.planning/`），规格驱动开发。详见 [AGENTS.md](AGENTS.md)。

## 许可证

[Apache License 2.0](LICENSE)

## 致谢

本项目借鉴经典桌面搜索工具（如 DocFetcher）的核心设计逻辑（mtime 增量、文件树 diff、Parser 注册表），用现代 Rust 组件栈重新实现。
