# pivotsearch


[English](README.md) | 中文
> 跨平台本地全文搜索桌面应用 · AnyTXT 的开源替代 · Windows / macOS / Linux

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![CI](https://github.com/alrece/pivotsearch/actions/workflows/ci.yml/badge.svg)](https://github.com/alrece/pivotsearch/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/alrece/pivotsearch)](https://github.com/alrece/pivotsearch/releases)
[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-red.svg)](https://v2.tauri.app/)

`pivotsearch` 是一个完全本地运行、离线可用的全文搜索工具。它索引你硬盘上的文档内容（PDF / Word / Excel / PowerPoint / Markdown / HTML / 纯文本 / ePub / 源代码），让你像用 Google 一样秒级搜索本地文件——不发送任何数据到云端。

## 界面预览

仿 AnyTXT 经典三栏布局：

```
┌──────────────────────────────────────────────────────────────────┐
│ 🔍 pivotsearch │ [搜索框........] [范围▾] [类型▾] [搜索] [索引管理] │
├────────────────────────────────────┬─────────────────────────────┤
│ 找到 3 个结果                        │  report.md                  │
├────────────────────────────────────┤                             │
│ 📃 季度报告                          │  预览面板                    │
│ ...命中片段 营收 增长...              │                             │
│ /path/report.md · 2KB · 2024-01-01  │  本季度营收增长百分之二十     │
├────────────────────────────────────┤  超出预期目标。              │
│ 📝 技术方案                          │  技术部门贡献了主要增长。     │
│ ...命中片段 React 前端...            │                             │
│ /path/plan.docx · 5KB · ...         │  （关键词蓝色高亮）          │
├────────────────────────────────────┴─────────────────────────────┤
│ 📂 2 个索引目录 · Documents(42349文件) · Notes(567文件)             │
└──────────────────────────────────────────────────────────────────┘
```

## 核心特性

- ⚡ **秒级检索** — 基于 Tantivy 倒排索引，索引吞吐 **1087 文件/秒**
- 🔄 **增量后台索引** — 文件系统监听 + mtime 比对，改动即更新
- 📄 **9 种格式** — PDF / Word(docx) / Excel(xlsx) / PPT(pptx) / Markdown / HTML / TXT / ePub / 源代码 + 归档(zip/tar 穿透)
- 🇨🇳 **中文友好** — jieba 分词 + 停用词过滤 + GBK/Big5 编码检测
- 🖥️ **原生桌面应用** — Tauri 2 + Vue 3，三端打包（.app / .dmg / .exe / .deb）
- 🔍 **即时搜索 + 预览** — 输入即搜（300ms debounce）+ 点击预览全文 + 关键词高亮
- 📁 **目录选择器** — 系统原生目录选择对话框添加索引
- 🔒 **完全离线** — 零数据外泄
- 🔬 **OCR（可选）** — Tesseract 集成，图片/扫描件可搜（feature gate）

## 下载安装

### 从 Release 下载（推荐）

前往 [Releases](https://github.com/alrece/pivotsearch/releases) 下载对应平台的安装包：

| 平台 | 格式 |
|---|---|
| macOS (Apple Silicon) | `.dmg` |
| macOS (Intel) | `.dmg` |
| Windows | `.msi` / `.exe` |
| Linux | `.deb` / `.AppImage` |

> **macOS 用户注意**：由于本应用未使用 Apple 开发者证书签名，首次打开可能提示"无法验证开发者"。解决方法：
> - 右键点击 app → 选择「打开」→ 点击「打开」确认
> - 或在终端运行：`sudo xattr -rd com.apple.quarantine /Applications/pivotsearch.app`

### 从源码构建

```bash
git clone https://github.com/alrece/pivotsearch.git
cd pivotsearch
pnpm install
pnpm tauri build          # 打包
# 或开发模式：
pnpm tauri dev            # 热重载开发
```

**前置依赖**：Rust 1.75+、Node 20+、pnpm、（macOS）Xcode Command Line Tools

### PDF 支持（可选）

PDF 解析需要 PDFium 库。运行构建脚本下载：

```bash
./scripts/fetch-pdfium.sh    # 自动检测平台并下载
```

### OCR 支持（可选）

```bash
cargo build --features ocr   # 启用 OCR（首次编译约 15s，构建 Tesseract）
```

## 使用

1. 启动 pivotsearch
2. 点「索引管理」→「📁 浏览」选择要索引的目录
3. 等待索引完成（底部状态栏显示进度）
4. 在搜索框输入关键词，即时出结果

## 支持的格式

| 格式 | 扩展名 | 状态 |
|---|---|---|
| PDF | `.pdf` | ✅ 需 PDFium |
| Word | `.docx` | ✅ |
| Excel | `.xlsx` `.xls` `.csv` | ✅ |
| PowerPoint | `.pptx` | ✅ |
| Markdown | `.md` | ✅ |
| HTML | `.html` `.htm` | ✅ |
| 纯文本/源代码 | `.txt` `.rs` `.py` `.js` `.json` `.yaml` 等 | ✅ |
| ePub | `.epub` | ✅ |
| 归档（穿透索引） | `.zip` `.tar` `.tar.gz` | ✅ |
| 图片（OCR） | `.jpg` `.png` `.tiff` | ⚠️ 可选 feature |
| Word 老格式 | `.doc` | ❌ 请转 `.docx` |

## 技术栈

**后端（Rust）**：Tantivy 0.24（全文引擎）· jieba-rs（中文分词）· notify（文件监听）· SQLite（元数据）· pdfium-render（PDF）· kreuzberg-tesseract（OCR）

**前端**：Tauri 2 · Vue 3 · TypeScript · Element Plus · Pinia

详见 [技术选型文档](docs/03-tech-selection.md)。

## 架构

```
crates/
├── contracts/    契约层（trait 定义，依赖终点）
├── parser/       解析层（9 格式 parser + 注册表两级选择）
├── index/        索引层（Tantivy schema + 增量算法 + SQLite）
├── watcher/      监听层（notify + 防抖 + 事件过滤）
├── queue/        队列层（单工作线程 + Task 状态机）
├── search/       查询层（单索引 + 多索引合并 + 高亮）
├── ocr/          OCR（feature gate 可选）
├── core/         编排层（只依赖 contracts）
└── cli/          CLI 工具（开发调试）
src-tauri/        Tauri 桌面后端（命令桥接）
src/              Vue 3 前端
```

## CLI 模式

```bash
# 索引目录
cargo run --bin pivotsearch -- index /path/to/docs

# 搜索
cargo run --bin pivotsearch -- search "关键词"
```

## 开发

```bash
cargo check && cargo test     # 后端编译 + 测试（44 测试）
pnpm build                    # 前端构建
make cleanroom                # 净室合规检查
pnpm tauri dev                # 桌面端热重载开发
```

项目使用 Loop Engineering 方法论管理（`.loop/` + `openspec/` + `.planning/`），全程可审计。详见 [AGENTS.md](AGENTS.md)。

## 项目状态

**v0.4.0** — macOS squircle 规范应用图标 + 品牌绿色 3D "PS" 视觉重做。

**v0.3.0** — 三端安装包 + psearch CLI + 进度条 + 索引详情 + 大小写敏感搜索。

详见 [CHANGELOG.md](CHANGELOG.md)。

## 致谢

本项目借鉴经典桌面搜索工具（如 DocFetcher）的核心设计逻辑（mtime 增量、文件树 diff、Parser 注册表），用现代 Rust 组件栈重新实现，并补齐了 OCR 能力。

## 许可证

[Apache License 2.0](LICENSE)
