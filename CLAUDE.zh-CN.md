# CLAUDE.md


[English](CLAUDE.md) | 中文
本文件是 Claude Code 的快速入口。与 `AGENTS.md` 的关系：如果冲突，以 `AGENTS.md` 为准。

## 项目一句话

`pivotsearch` 是跨平台（Win/macOS/Linux）本地全文搜索桌面应用，AnyTXT 的开源替代，用 Rust（Tantivy + Tauri）现代栈复刻 DocFetcher 的核心设计并补齐 OCR。

## 阅读顺序

1. `AGENTS.md` — 仓库宪法，架构事实 + 开发约定 + 合规红线（最高权威）
2. `.loop/STATE.yaml` — 当前迭代状态（单一事实源）
3. `openspec/changes/pivotsearch-v1-local-search/` — 规格驱动开发
4. `.planning/ROADMAP.md` — Phase 路线图
5. `docs/` — 技术文档

## 关键约定速查

- **依赖方向铁律**：`core` 编排层只依赖 `contracts` trait，绝不 import 具体实现；只有 `cli`/`src-tauri` 能 import 具体实现
- **Tantivy 约束**：schema 不可变（变更需 reindex）；单 writer 强约束（同索引目录同时只能一个 writer）
- **净室红线**：禁止复制 DocFetcher Java 代码/类名/标识符，只复刻设计逻辑；产出后跑 `grep -ri "docfetcher" crates/ src/ src-tauri/` 验证
- **原生依赖**：PDFium 静态链接；Tesseract 可选 + 语言包按需下载；.doc/.ppt v1 不支持
- **中文输出**：面向用户的所有说明用简体中文，代码/命令/路径/函数名保留原文

## 命令索引

```bash
cargo check && cargo test                    # 编译 + 测试
cargo clippy --all-targets -- -D warnings    # lint
cargo tauri dev                              # 桌面端开发
grep -ri "docfetcher" crates/ src/ src-tauri/ # 净室检查（无输出 = 通过）
```

## Loop Engineering

仅在 `/loop:*` 命令显式调用时介入。状态读取 `.loop/STATE.yaml`，事件追加 `.loop/timeline.jsonl`。详见 `AGENTS.md` 的 "Loop Engineering 协作规范" 章节。
