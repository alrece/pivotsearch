# Changelog


[English](CHANGELOG.md) | 中文
本项目的所有重要变更记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

## [Unreleased]

## [0.4.0] - 2026-07-03

### Changed
- **应用图标重做**：采用品牌绿色 3D "PS" 图标，套用 macOS squircle（连续曲率圆角，半径 22.4%）规范
  - 50 个图标文件全部重新生成（macOS `.icns` / Windows `.ico` / Linux PNG / iOS 18 张 / Android 15 张）
  - 1px 极窄白边，PS 字母几乎顶满圆角画板
  - 圆角边缘抗锯齿处理（Gaussian 平滑）
  - 三端图标风格统一

### Fixed
- 修复 dev 模式下图标未正确显示 squircle 圆角的问题（圆角半径参数从 13% 修正为标准 22.4%）

## [0.3.1] - 2026-07-03

### Fixed
- macOS ad-hoc 签名修复（CI 中 `codesign --force --deep --sign -`）

## [0.3.0] - 2026-07-03

### Added
- **psearch CLI 工具**：供 AI Agent / CloudPivot 调用，JSON 输出
  - `psearch search "query" --json`（Agent 核心调用接口）
  - `psearch index/list/remove/rebuild/preview/status`
  - 数据目录与桌面 app 共享
  - 随 app 安装部署（Tauri sidecar）+ macOS 符号链接注册
- **索引进度条**：新建/重建时底部显示百分比 + 文件数（如 `[Documents] 正在索引... 45%`）
- **索引详情查看**：双击索引行弹出详情（文件类型分布 + 最近修改文件列表）
- **大小写敏感搜索**：搜索栏 Aa 切换按钮
- **复制路径 / 打开目录**：搜索结果每项的快捷操作按钮
- **可拖动分隔栏**：结果列表和预览面板宽度可自由调整
- **目录选择器**：系统原生目录选择对话框添加索引
- **macOS ad-hoc 签名**：解决 Safari 下载 DMG 的 Gatekeeper 拦截

### Changed
- 界面仿 AnyTXT 三栏布局（顶搜索栏 + 左结果列表 + 右预览面板 + 底状态栏）
- 品牌名统一为 PivotSearch
- 搜索结果标题改为显示带后缀文件名
- 三端 CI/Release workflow（含 sidecar 编译 + 产物上传）

### Fixed
- 重启后索引列表丢失（state 从磁盘恢复）
- 重复添加同路径报错（open-or-create）
- snippet 高亮为空（snippet_text 字段 + 手动高亮）
- Tauri 版本 mismatch（NPM/Rust 对齐到 2.11.x）

## [0.2.0] - 2026-07-02

### Added
- 三端安装包 CI（macOS .dmg / Linux .deb+.AppImage / Windows .msi+.exe）
- GitHub Release 自动创建（含安装说明）

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

[Unreleased]: https://github.com/alrece/pivotsearch/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/alrece/pivotsearch/releases/tag/v0.3.0
[0.2.0]: https://github.com/alrece/pivotsearch/releases/tag/v0.2.0
[0.1.0]: https://github.com/alrece/pivotsearch/releases/tag/v0.1.0
[0.1.0-alpha]: https://github.com/alrece/pivotsearch/releases/tag/v0.1.0-alpha
