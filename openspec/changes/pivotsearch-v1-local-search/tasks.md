# Implementation Tasks

> 8 个 capability 对应 T1-T14 任务，按 Phase 路线图组织（Phase 0-5）。
> 每个 task 标 P1（block ship）/ P2（same branch）+ 所属 capability + Files + Steps + Verify。

## Phase 0: 工程脚手架

- [x] **T0 (P1) — 方法论框架 + workspace 骨架**
  - Capability: 全局
  - Files: AGENTS.md, CLAUDE.md, README.md, LICENSE, DESIGN.md, .loop/*, openspec/*, .planning/*, Cargo.toml(workspace), crates/*/Cargo.toml, crates/*/src/lib.rs
  - Steps:
    1. 建立 .loop/ openspec/ .planning/ 全套方法论文件
    2. 写 8 个 capability 的 spec.md（Requirement + Scenario）
    3. 初始化 Cargo workspace（9 crate 骨架，每个 lib.rs 占位）
    4. 初始化 Tauri 项目骨架
  - Verify: `cargo check` 通过 + 净室 grep 无 DocFetcher 残留 + `.loop/STATE.yaml` 初始化完成

## Phase 1: 核心索引闭环（Lane A 后端核心）

- [ ] **T1 (P1) — 契约层 + Tantivy schema**
  - Capability: core-index-schema
  - Files: crates/contracts/src/{lib.rs,parser.rs,indexer.rs,searcher.rs,watcher.rs,types.rs,error.rs}, crates/index/src/{lib.rs,schema.rs,doc_builder.rs}
  - Steps:
    1. contracts: Parser/Indexer/Searcher/Watcher trait + ParseResult/IndexedDoc/Task/SearchRequest/SearchResponse 数据结构 + PivotsearchError（thiserror）
    2. index: Tantivy schema（8 字段）+ uid 算法 + Document 组装（content 追加 title/author/文件名）
    3. 单元测试：uid 规范化、Document 字段拼接
  - Verify: `cargo test -p pivotsearch-contracts -p pivotsearch-index`

- [ ] **T2 (P1) — 解析层 MVP（5 高频格式）**
  - Capability: parser-registry
  - Files: crates/parser/src/{lib.rs,registry.rs,text.rs,markdown.rs,html.rs,pdf.rs,docx.rs,xlsx.rs}
  - Steps:
    1. Parser trait 实现 + Registry（两级选择：mime 优先 / 扩展名 fallback）
    2. TextParser（encoding_rs+chardetng）、MarkdownParser（pulldown-cmark）、HtmlParser（scraper）
    3. PdfParser（pdfium-render）、DocxParser（docx-rs）、SpreadsheetParser（calamine）
    4. 单元测试：每种格式解析出正确文本
  - Verify: `cargo test -p pivotsearch-parser`

- [ ] **T3 (P1) — 查询闭环 + CLI**
  - Capability: search-engine
  - Files: crates/search/src/{lib.rs,query.rs,highlight.rs}, crates/cli/src/main.rs, crates/core/src/lib.rs
  - Steps:
    1. search: 单索引查询 + SnippetGenerator 高亮 + 分页 + jieba tokenizer 注册
    2. core: PivotsearchEngine 组装（parser+index+search）
    3. cli: `pivotsearch index <dir>` + `pivotsearch search <query>`
  - Verify: CLI 索引一个含 txt/md/docx/xlsx/pdf 的目录，查询返回带高亮结果

## Phase 2: 增量与监听（Lane A 增强）

- [ ] **T4 (P1) — 增量索引 + SQLite 元数据**
  - Capability: incremental-index
  - Files: crates/index/src/{incremental.rs,tree_index.rs}, crates/index/migrations/
  - Steps:
    1. tree_index SQLite schema（indexed_files + index_roots 表）+ rusqlite 封装
    2. 增量算法：mtime 比对 + unseenDocs/unseenSubdirs 集合 diff + 未改归档整体跳过
    3. 单元测试：新增/修改/删除三类变化 + 归档跳过
  - Verify: `cargo test -p pivotsearch-index incremental`

- [ ] **T5 (P1) — 文件监听**
  - Capability: file-watcher
  - Files: crates/watcher/src/{lib.rs,debounce.rs,filter.rs}
  - Steps:
    1. notify + notify-debouncer-full（1s 防抖单 flight）
    2. 事件过滤（索引目录/Word 临时文件/不可解析文件）
    3. mtime 二次校验（查 tree_index）
  - Verify: 编辑文件后触发 update task，纯访问不触发

- [ ] **T6 (P1) — 任务队列**
  - Capability: indexing-queue
  - Files: crates/queue/src/{lib.rs,task.rs,worker.rs}
  - Steps:
    1. 单工作线程（crossbeam-channel）+ Task 状态机
    2. UPDATE/REBUILD 语义 + 去重 + 重叠检测
    3. SUCCESS_UNCHANGED 跳过 save
  - Verify: 并发添加多索引根，任务串行执行无冲突

## Phase 3: 解析补全 + 多索引（Lane A 完善）

- [ ] **T7 (P2) — 全格式解析补全**
  - Capability: parser-registry
  - Files: crates/parser/src/{pptx.rs,epub.rs,archive.rs}
  - Steps:
    1. PptxParser（ooxmlsdk）、EpubParser（epub crate）
    2. 归档穿透（zip/tar/sevenz-rust，解包到临时目录递归解析）
    3. .doc/.ppt 检测 → 返回 UnsupportedFormat 错误
  - Verify: 索引含 pptx/epub/zip 的目录，归档内文件可搜

- [ ] **T8 (P2) — 多索引合并搜索**
  - Capability: search-engine
  - Files: crates/search/src/multi.rs
  - Steps:
    1. 多索引合并（每索引一个 Searcher，合并 top-N）
    2. index_id 字段过滤 + size range 过滤 + parser type 过滤
  - Verify: 多目录索引，跨索引查询 + 筛选器生效

## Phase 4: OCR + 桌面 UI（Lane B）

- [ ] **T9 (P2) — OCR 管道（feature gate）**
  - Capability: ocr-pipeline
  - Files: crates/ocr/src/{lib.rs,tesseract.rs,language_pack.rs}, crates/parser/src/ocr_parser.rs
  - Steps:
    1. kreuzberg-tesseract 集成（feature gate `ocr`）
    2. ImageOcrParser（jpg/png/tiff）+ 扫描件 PDF（pdfium 渲染→OCR）
    3. 语言包按需下载（chi_sim/eng）
  - Verify: `cargo build --features ocr` + 图片 OCR 出文本

- [ ] **T10 (P1) — Tauri 桥接 + 核心 UI**
  - Capability: desktop-ui
  - Files: src-tauri/src/{main.rs,commands.rs,state.rs}, src/views/SearchView.vue, src/components/{SearchBox.vue,ResultList.vue,PreviewPanel.vue}
  - Steps:
    1. #[tauri::command]：add_index/search/get_preview/list_indexes/remove_index/rebuild_index
    2. 后台线程索引 + 进度 emit
    3. Vue 搜索视图：即时搜索（debounce 200ms）+ 结果虚拟列表 + 高亮 + 预览面板
  - Verify: `cargo tauri dev` 端到端：添加索引→后台索引→即时搜索→预览

- [ ] **T11 (P1) — UI 完善**
  - Capability: desktop-ui
  - Files: src/views/{IndexManage.vue,Settings.vue}, src/components/{FilterPanel.vue,IndexCard.vue}
  - Steps:
    1. 索引管理面板（添加/删除/重建/状态显示）
    2. 筛选器（类型/大小/索引多选）
    3. 设置（OCR 开关/扩展名配置/排除规则）
  - Verify: 索引 CRUD + 筛选器 + OCR 开关切换

## Phase 5: 打磨与发布（Lane C）

- [ ] **T12 (P1) — 三端打包 CI**
  - Capability: 全局
  - Files: .github/workflows/release.yml, Makefile
  - Steps:
    1. GitHub Actions 三平台 matrix（Windows MSI/Replaces、macOS DMG、Linux AppImage）
    2. PDFium 静态链接处理（各平台预编译 binary）
    3. 索引性能压测（十万级文件）
  - Verify: 三端 CI 出包 + 性能基线达标

- [ ] **T13 (P2) — 文档与中文调优**
  - Capability: 全局
  - Files: docs/, README.md
  - Steps:
    1. 用户文档 + 开发文档完善
    2. 中文分词调优（自定义词典/停用词/ngram 兜底）
    3. retro + ship
  - Verify: 文档完整 + 中文检索质量基线达标

- [ ] **T14 (P2) — 净室合规 + 对抗门自动化**
  - Capability: 全局
  - Files: scripts/cleanroom-check.sh, .loop/adversarial/
  - Steps:
    1. 全量 grep 检查 DocFetcher 标识符零残留
    2. 对抗门脚本（编译 + 测试覆盖 + 净室 + spec 覆盖）
  - Verify: 对抗门全过 + 覆盖率 ≥95%
