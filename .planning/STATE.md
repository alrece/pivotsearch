# pivotsearch — GSD State

## Project Status
- **Current Phase:** 0
- **Phase Status:** scaffolding
- **Last Updated:** 2026-07-02T12:30:00Z
- **Branch:** main

## Phase Tracking

| Phase | Name | Status | Tasks Total | Tasks Done |
|-------|------|--------|-------------|------------|
| 0 | 工程脚手架 | active | 1 (T0) | 0 |
| 1 | 核心索引闭环 | not_started | 3 (T1-T3) | 0 |
| 2 | 增量与监听 | not_started | 3 (T4-T6) | 0 |
| 3 | 解析补全+多索引 | not_started | 2 (T7-T8) | 0 |
| 4 | OCR+桌面UI | not_started | 3 (T9-T11) | 0 |
| 5 | 打磨与发布 | not_started | 3 (T12-T14) | 0 |

## Active Decisions（从 Loop Engineering 继承，不需重新讨论）

- 技术栈：Rust + Tantivy 0.24 + Tauri 2 + pdfium-render + kreuzberg-tesseract（feature gate）
- 纯 Rust 主体 + 三处受控原生依赖（用户 ExitPlanMode 审批确认）
- 元数据：SQLite 替代 Java 序列化
- 高亮：SnippetGenerator 单路径替代 Lucene 双高亮
- 中文分词：jieba-rs 自写 Tokenizer（不依赖 tantivy-jieba）
- 老格式 .doc/.ppt：v1 不支持，提示转换

## Notes
- Loop Engineering Phase 0 脚手架进行中
- 8 个 capability 的完整规格见 `openspec/changes/pivotsearch-v1-local-search/`
- 方法论参考 `/Users/alrece/GitHub/cloudpivot`，设计逻辑蓝本 `/Users/alrece/Github/DocFetcher`（净室）
