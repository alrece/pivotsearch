# pivotsearch

> 跨平台本地全文搜索桌面应用 · AnyTXT 的开源替代 · 三端可用（Windows / macOS / Linux）

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)

`pivotsearch` 是一个完全本地运行、离线可用的全文搜索工具。它索引你硬盘上的文档内容（PDF / Word / Excel / PowerPoint / Markdown / HTML / 纯文本 / ePub / 源代码），让你像用 Google 一样秒级搜索本地文件——不发送任何数据到云端。

## 为什么做这个

市面上的本地全文搜索工具都有遗憾：

| 工具 | 问题 |
|---|---|
| **AnyTXT Searcher** | 闭源、仅 Windows、几周后索引性能退化、漏文档 |
| **DocFetcher（免费版）** | 开源但实质停滞（最后大更新 2023-10）、Java 重、吃内存 |
| **Everything** | 只搜文件名，不搜内容 |
| **Recoll** | Linux 优先，Windows/macOS 体验弱 |

`pivotsearch` 填补这个空白：**现代化的、三端通用的、活跃维护的、带 OCR 的开源本地全文搜索**。

## 核心特性

- ⚡ **秒级检索** — 基于 Tantivy 倒排索引，查询毫秒级返回
- 🔄 **增量后台索引** — 文件系统监听 + mtime 比对，改动即更新，不用全量重建
- 📄 **多格式解析** — PDF / Office(docx/xlsx/pptx) / Markdown / HTML / TXT / ePub / 源代码
- 🇨🇳 **中文友好** — 内置 jieba 分词，处理好中英混排和 GBK/Big5 遗留编码
- 🔍 **OCR** — 图片和扫描件 PDF 也能搜（基于 Tesseract，语言包按需下载）
- 🖥️ **三端原生** — Windows / macOS / Linux 同一份体验（Tauri 2，体积小、启动快）
- 🔒 **完全离线** — 索引、搜索、OCR 全部本地完成，零数据外泄

## 技术栈

- **后端**：Rust + [Tantivy](https://github.com/quickwit-oss/tantivy)（全文引擎）+ [notify](https://github.com/notify-rs/notify)（文件监听）+ [pdfium-render](https://github.com/ajrcarey/pdfium-render)（PDF）+ [Tesseract](https://github.com/tesseract-ocr/tesseract)（OCR，可选）
- **前端**：[Tauri 2](https://v2.tauri.app/) + Vue 3 + TypeScript + Element Plus

详见 [docs/03-tech-selection.md](docs/03-tech-selection.md)。

## 安装

> 🚧 项目处于早期开发阶段，尚未发布预编译包。请从源码构建。

```bash
# 前置：Rust 1.75+、Node 20+、pnpm
git clone https://github.com/yourname/pivotsearch.git
cd pivotsearch
pnpm install
cargo tauri dev
```

## 使用

```bash
# CLI（开发期）
pivotsearch index /path/to/docs          # 索引一个目录
pivotsearch search "关键词"               # 搜索

# 桌面应用
# 启动后：添加索引目录 → 后台自动索引 → 搜索框输入即出结果
```

## 项目状态

当前迭代状态见 [`.loop/STATE.yaml`](.loop/STATE.yaml)。路线图见 [`.planning/ROADMAP.md`](.planning/ROADMAP.md)。

## 开发

项目用 Loop Engineering 方法论管理，规格驱动开发。详见 [AGENTS.md](AGENTS.md)。

```bash
cargo check && cargo test         # 编译 + 测试
cargo clippy --all-targets        # lint
pnpm dev                          # 前端开发服务器
cargo tauri dev                   # 桌面端开发模式
```

## 许可证

[Apache License 2.0](LICENSE)

## 致谢

本项目借鉴 [DocFetcher](http://docfetcher.sourceforge.io/) 的核心设计逻辑（mtime 增量、文件树 diff、Parser 注册表），用现代 Rust 组件栈重新实现。DocFetcher 是 Java/Lucene 生态的优秀作品，我们站在它的肩膀上。
