## Why

本地全文搜索市场存在"三难困境"：没有任何工具同时满足"开源 + 三端 + 内容搜索 + 活跃维护 + OCR"。AnyTXT（闭源、仅 Windows、性能退化）、DocFetcher 免费版（停滞于 2023-10、Java 重）、Everything（仅文件名）、Recoll（Linux 优先）各有短板。

pivotsearch v1 填补这个空白：交付一个跨平台（Win/macOS/Linux）、本地优先、离线可用的全文搜索桌面应用，用现代 Rust 组件栈（Tantivy + Tauri）复刻 DocFetcher 的核心设计逻辑（mtime 增量、Parser 注册表、文件树 diff）并补齐 OCR 能力。完全净室重写，不复制 DocFetcher 任何 Java 代码。

## What Changes

- **新增 Tantivy 索引引擎核心**：schema 定死（uid/type/parser/content/title/author/size/last_modified/index_id 八字段）、uid 主键（`file://{canonical_path}`）、upsert 语义（delete_term + add_document）、单 writer 强约束
- **新增多格式 Parser 注册表**：Parser trait + 两级选择（mime 优先 / 扩展名 fallback / 多 parser 容错尝试）。覆盖 PDF(pdfium-render) / docx / xlsx / pptx / xls / Markdown / HTML / TXT / ePub / 源代码。纯 Rust 主体，PDF 受控引入 PDFium
- **新增 mtime 增量索引**：mtime 比对判定变化（不用 hash，性能优）+ unseenDocs/unseenSubFolders 集合 diff（剩余即删除）+ 未改归档整体跳过 + tree_index 用 SQLite 持久化（替代 Java 序列化）
- **新增文件系统监听**：notify 跨平台（FSEvents/inotify/ReadDirectoryChangesW）+ notify-debouncer-full 防抖（1s 单 flight 合并）+ 事件过滤（索引目录/临时文件/不可解析文件）+ mtime 二次校验去噪
- **新增索引任务队列**：单工作线程串行执行（Tantivy 单 writer 强约束）+ Task 状态机（NOT_READY→READY→INDEXING→FINISHED）+ UPDATE/REBUILD 语义 + 不同索引根可各自独立 writer 并发
- **新增搜索引擎**：多索引合并（每索引独立 Searcher 合并 top-N）+ 查询解析（AND 默认、通配符、范围过滤）+ 分页（PAGE_SIZE=50）+ SnippetGenerator 高亮（替代 Lucene 双高亮路径）+ stopSearch 中断
- **新增 OCR 管道（feature gate 可选）**：Tesseract（kreuzberg-tesseract 内置静态编译）+ 图片识别（jpg/png/tiff）+ 扫描件 PDF（pdfium 渲染页→OCR）+ 语言包按需下载（不进默认包）。Parser 注册表新增 ImageOcrParser
- **新增 Tauri 2 桌面 UI**：即时搜索（debounce 200ms）+ 虚拟列表结果 + 高亮渲染 + 预览面板（重新解析原文件）+ 索引管理（添加/删除/重建索引根）+ 筛选器（类型/大小/索引）+ 设置（OCR 开关/扩展名配置）
- **明确不支持（v1 范围外）**：MS Office 老格式 .doc/.ppt（Rust 无成熟纯解析器，检测到时 UI 提示转换为 .docx/.pptx）

## Capabilities

### New Capabilities

- `core-index-schema`: Tantivy schema 设计（八字段）+ uid 算法（`file://{canonical_path}`）+ Document 组装（content 追加 title/author/文件名多值）+ upsert 语义（delete_term + add）
- `parser-registry`: Parser trait + 注册表 + 两级选择策略（mime 优先 / 扩展名 fallback / 多 parser 容错）+ 10 类格式解析器实现 + 净室设计（借鉴 DocFetcher ParseService 模式）
- `incremental-index`: mtime 比对（`is_modified = old.mtime != new.mtime`）+ unseenDocs 集合 diff 增量算法 + 未改归档整体跳过 + tree_index SQLite 持久化（{path, mtime, parser}）
- `file-watcher`: notify 跨平台监听 + notify-debouncer-full 防抖（1s 单 flight）+ 事件过滤（索引目录/Word 临时文件/不可解析文件）+ mtime 二次校验（查 tree_index 比对，过滤访问误报）
- `indexing-queue`: 单工作线程任务队列（crossbeam-channel）+ Task 状态机 + UPDATE/REBUILD + 多索引并发（每索引独立 writer）+ SUCCESS_UNCHANGED 跳过 save
- `search-engine`: 多索引合并（手动合并 top-N，每索引一个 Searcher）+ 查询解析（QueryParser，AND 默认）+ 分页（Top(k) 切片）+ SnippetGenerator 高亮 + stopSearch（AtomicBool）
- `ocr-pipeline`: Tesseract 集成（feature gate，kreuzberg-tesseract 静态编译）+ 图片 OCR + 扫描件 PDF（pdfium 渲染→OCR）+ 语言包按需下载 + ImageOcrParser 注册到 Parser 注册表
- `desktop-ui`: Tauri 2 前端 + #[tauri::command] 桥接（index/search/watch/status）+ 后台线程 + 进度 emit + Vue 3 即时搜索/结果列表/预览/索引管理/筛选器/设置

### Modified Capabilities

（无——这是 greenfield v1）
