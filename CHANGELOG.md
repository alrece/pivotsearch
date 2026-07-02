# Changelog

本项目的所有重要变更记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

## [Unreleased]

## [0.1.0] - 2026-07-02

### Added
- 跨平台桌面应用（Tauri 2 + Vue 3 + Rust），支持 macOS / Windows / Linux
- 9 种文件格式全文解析：PDF / Word(docx) / Excel(xlsx/xls/csv) / PPT(pptx) / Markdown / HTML / 纯文本+源代码 / ePub / 归档穿透(zip/tar)
- Tantivy 倒排索引引擎 + jieba 中文分词（含停用词过滤）
- 增量索引：mtime 比对 + unseenDocs 文件树 diff + SQLite 元数据持久化
- 文件系统监听：notify + 1s 防抖 + 事件过滤 + mtime 二次校验
- 单工作线程任务队列：Task 状态机 + UPDATE/REBUILD + 去重
- 多索引合并搜索：跨目录查询 + 文件类型/大小/索引根过滤
- snippet 高亮：snippet_text 字段 + 手动 query 词高亮
- 即时搜索 UI（300ms debounce）+ 结果列表 + 预览面板（重新解析原文件）
- 索引管理面板：系统原生目录选择器 + 添加/删除/重建索引
- GBK/Big5 遗留编码检测（chardetng + encoding_rs）
- OCR 管道（feature gate）：kreuzberg-tesseract + image 解码 + TesseractAPI
- PDFium 动态链接支持（bblanchon/pdfium-binaries）
- 三端 CI（GitHub Actions 矩阵）+ Release workflow（4 target）
- CLI 工具（`pivotsearch index <dir>` / `pivotsearch search <query>`）
- Loop Engineering 工程方法论（.loop/openspec/.planning 全程可审计）
- 44 单元测试（含 OCR 真实识别验证）

### Performance
- 索引吞吐：1087 文件/秒（基线测试）
- 索引体积：~164KB / 500 文件

### Known Limitations
- .doc / .ppt 老格式不支持（建议转换为 .docx / .pptx）
- PDFium 需运行 `scripts/fetch-pdfium.sh` 下载
- OCR 需 `--features ocr` 编译 + 语言包
- Windows/Linux 打包需在对应平台 CI 环境

## [0.1.0-alpha] - 2026-07-02

### Added
- 工程脚手架：9 crate workspace + 方法论框架 + 8 capability spec
- 核心索引闭环：Tantivy schema + Parser 注册表 + SimpleSearcher + CLI
- 增量与监听：SQLite tree_index + notify + 任务队列
- 全格式补全：epub/pptx/归档穿透 + 多索引合并
- Tauri 桌面 UI 骨架：Vue 3 前端 + #[tauri::command] 桥接
- CI + 文档：三端矩阵 + 中文停用词调优

[Unreleased]: https://github.com/alrece/pivotsearch/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/alrece/pivotsearch/releases/tag/v0.1.0
[0.1.0-alpha]: https://github.com/alrece/pivotsearch/releases/tag/v0.1.0-alpha
