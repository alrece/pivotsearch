# 设计文档 — pivotsearch v1

> 技术设计细节，配合 proposal.md 与 specs/ 使用。

## 1. 总体架构

```
┌─────────────────────────────────────────────────────────────┐
│  Tauri 2 前端（Vue 3 + Element Plus）                        │
│  即时搜索框 / 结果虚拟列表 / 高亮 / 预览面板 / 索引管理        │
├─────────────────────────────────────────────────────────────┤
│  #[tauri::command] 桥接（src-tauri/）                         │
├─────────────────────────────────────────────────────────────┤
│  core 编排层（PivotsearchEngine 总入口）                      │
│  ┌──────────┬──────────┬──────────┬──────────┬──────────┐   │
│  │ parser   │ index    │ watcher  │ queue    │ search   │   │
│  │ 解析层    │ 索引层    │ 监听层    │ 队列层    │ 查询层    │   │
│  └──────────┴──────────┴──────────┴──────────┴──────────┘   │
│              都只依赖 contracts trait（依赖终点）              │
├─────────────────────────────────────────────────────────────┤
│  contracts: Parser / Indexer / Searcher / Watcher trait      │
│             + ParseResult / IndexedDoc / Task 数据结构        │
│             + PivotsearchError 错误类型                       │
└─────────────────────────────────────────────────────────────┘
```

依赖方向铁律：`cli`/`src-tauri` → `core` → `contracts` ← 各能力 crate。能力 crate 之间互不依赖。

## 2. Tantivy Schema 设计（core-index-schema）

| 字段 | Tantivy 类型 | Stored | Indexed | 说明 |
|---|---|---|---|---|
| `uid` | `Str` (raw tokenizer) | ✅ | ✅ | `file://{canonical_path}`，主键，用于 delete/update |
| `content` | `Text` (jieba tokenizer) | ❌ | ✅ | 正文 + title + author + 文件名拼接（多值 add_text） |
| `title` | `Text` | ✅ | ✅ | 标题（无则用去扩展名文件名） |
| `author` | `Text` 多值 | ✅ | ✅ | 作者列表 |
| `type` | `Str` (raw) | ✅ | ✅ | 扩展名，用于类型过滤 |
| `parser` | `Str` (raw) | ✅ | ✅ | 解析器名，预览时决定渲染方式 |
| `size` | `I64` | ✅ | ✅ | 字节大小，用于范围过滤 |
| `last_modified` | `I64` | ✅ | ✅ | 毫秒时间戳，用于范围过滤/排序 |
| `index_id` | `Str` (raw) | ✅ | ✅ | 所属索引根 ID，用于多索引过滤 |

**关键差异（vs DocFetcher/Lucene）**：
- `content` 不存（省空间），预览时重新解析原文件
- 无 term-vector offset，高亮统一用 `SnippetGenerator` 重切文本
- `last_modified` 用 I64 数值（DocFetcher 用 String），支持范围查询
- 新增 `index_id` 字段（DocFetcher 用 uid 前缀过滤，Tantivy 用独立字段更直接）

**uid 算法**：`format!("file://{}", canonical_path)`，canonical_path 用 `std::fs::canonicalize` 规范化。

## 3. Parser 注册表（parser-registry）

```rust
// contracts
pub trait Parser: Send + Sync {
    fn extensions(&self) -> &[&str];        // ["pdf", "PDF"]
    fn mimes(&self) -> &[&str];             // ["application/pdf"]
    fn parse(&self, path: &Path) -> Result<ParseResult, PivotsearchError>;
}

pub struct ParseResult {
    pub content: String,                    // 必需，纯文本
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub misc_metadata: Vec<String>,
    pub parser_name: &'static str,          // 由注册表注入
}
```

**两级选择**（复刻 DocFetcher ParseService 模式，净室重写）：
1. **mime 路径**（命中 mime 检测规则时）：`infer` crate 魔数检测 → 按匹配度排序 → 依次尝试直到成功（容错）
2. **扩展名路径**（fallback）：精确匹配扩展名取第一个 parser
3. **兜底**：`index_filenames` 开启时返回空 content 只索引文件名

**格式实现清单**：

| 格式 | crate | parser 名 | 备注 |
|---|---|---|---|
| PDF | `pdfium-render`（静态链接） | `PdfParser` | 含中文；扫描件走 OCR |
| DOCX | `docx-rs` 或 `ooxmlsdk` | `DocxParser` | OOXML |
| XLSX/XLS/CSV | `calamine` | `SpreadsheetParser` | 含老 xls |
| PPTX | `ooxmlsdk` | `PptxParser` | OOXML |
| Markdown | `pulldown-cmark` | `MarkdownParser` | CommonMark |
| HTML | `scraper` | `HtmlParser` | 正文提取，去 script/style |
| TXT/源代码 | `encoding_rs`+`chardetng` | `TextParser` | 编码检测，扩展名可配 |
| ePub | `epub` crate | `EpubParser` | zip+xhtml |
| 归档 | `zip`/`tar`/`sevenz-rust` | （穿透，非 parser） | 解包后递归解析内部文件 |
| 图片(OCR) | `kreuzberg-tesseract` | `ImageOcrParser` | feature gate 可选 |

**不支持**：.doc/.ppt（检测到时返回 `UnsupportedFormat` 错误，UI 提示转换）。

## 4. 增量索引算法（incremental-index）

```rust
// 伪代码（复刻 DocFetcher FileIndex.doUpdate + visitDirOrZip，净室重写）
fn update_index(root: &Path, tree_index: &mut TreeIndex, writer: &mut IndexWriter) -> UpdateResult {
    let root_meta = root.metadata()?;
    if tree_index.is_unmodified_archive(root, root_meta.modified()?) {
        return UpdateResult::SuccessUnchanged;  // ★归档整体跳过
    }
    let mut unseen_docs = tree_index.documents_at(root).clone();  // 当前已索引
    let mut unseen_subdirs = tree_index.subdirs_at(root).clone();
    
    for entry in walkdir::WalkDir::new(root) {
        match entry {
            File(name) => {
                let doc = unseen_docs.remove(name);
                if doc.is_none() {
                    // 新增：解析 + 索引
                    let parsed = parser_registry.parse(path)?;
                    writer.add_document(build_doc(parsed, uid, ...));
                } else if doc.unwrap().mtime != path.mtime() {
                    // 修改：delete_term + add
                    writer.delete_term(Term::from_field_text(uid_field, &uid));
                    let parsed = parser_registry.parse(path)?;
                    writer.add_document(build_doc(parsed, uid, ...));
                }
                // else: 未变，跳过
            }
            Dir(name) => {
                unseen_subdirs.remove(name);
                if tree_index.is_unmodified_archive(subdir, mtime) { continue; }  // 跳过未改归档
                update_index(subdir, tree_index, writer);  // 递归
            }
        }
    }
    
    // ★剩余即删除
    for (name, doc) in unseen_docs {
        writer.delete_term(Term::from_field_text(uid_field, &doc.uid));
        tree_index.remove_document(doc);
    }
    for (name, subdir) in unseen_subdirs {
        recursively_delete(subdir, tree_index, writer);
    }
    
    writer.commit()?;
    tree_index.persist_sqlite()?;  // SQLite 替代 Java 序列化
    UpdateResult::SuccessChanged
}
```

**tree_index SQLite schema**：
```sql
CREATE TABLE indexed_files (
    uid TEXT PRIMARY KEY,           -- file://{path}
    path TEXT NOT NULL,
    mtime INTEGER NOT NULL,         -- 毫秒时间戳
    parser TEXT,                    -- 解析器名
    index_id TEXT NOT NULL,         -- 所属索引根
    FOREIGN KEY (index_id) REFERENCES index_roots(id)
);
CREATE TABLE index_roots (
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    display_name TEXT,
    created_at INTEGER NOT NULL
);
```

## 5. 文件监听（file-watcher）

- **notify-debouncer-full**：1s 防抖窗口，单 flight 合并（编辑器保存触发 N 次事件只发 1 个 update task）
- **事件过滤** `accept()`：
  1. 忽略索引目录自身事件（防自反馈死循环）
  2. 忽略 Word 临时文件 `~$*.docx?`
  3. 应用用户 exclude 规则
  4. 不可解析文件（扩展名不在 parser 注册表）→ 忽略
  5. **mtime 二次校验**：MODIFIED 事件查 tree_index，mtime 相同则丢弃（notify 常对纯访问误报）
- **macOS FSEvents 注意**：不提供精确逐文件事件，可能合并/丢失。当"有变化→重扫"提示用，配合 mtime 校验兜底

## 6. 任务队列（indexing-queue）

```rust
pub enum IndexAction { Update, Rebuild }
pub enum CancelAction { Keep, Discard }

pub struct Task {
    pub index_id: String,
    pub action: IndexAction,
    pub state: TaskState,  // NotReady → Ready → Indexing → Finished
    pub cancel_action: Option<CancelAction>,
}
```

- **单工作线程**：`crossbeam-channel` 发 Task 到唯一 worker（Tantivy 单 writer 强约束）
- **去重**：队列里已有同 index_id 的 Ready Update task → 新的 Update 冗余，丢弃
- **重叠检测**：新索引根与 registry/queue 中已有索引 contains 互含 → 拒绝
- **多索引并发**：不同 index_id 各自独立 writer（不同目录），可在不同 OS 线程并发（但共享同一 worker 队列串行调度，避免锁复杂度；未来可优化为每索引一线程）

## 7. 搜索引擎（search-engine）

```rust
pub struct SearchRequest {
    pub query: String,
    pub index_ids: Option<Vec<String>>,  // None = 搜全部
    pub parsers: Option<Vec<String>>,    // 类型过滤
    pub min_size: Option<i64>,
    pub max_size: Option<i64>,
    pub page: usize,
}

pub struct SearchResponse {
    pub total_hits: usize,
    pub results: Vec<SearchResult>,
    pub page_count: usize,
}

pub struct SearchResult {
    pub uid: String,
    pub path: String,
    pub title: String,
    pub snippet: String,         // SnippetGenerator 产出，含高亮标记
    pub score: f32,
    pub size: i64,
    pub last_modified: i64,
    pub parser: String,
    pub index_id: String,
}
```

- **多索引合并**：每索引一个 `Searcher`，各跑 top-N，手动合并取全局 top-N
- **查询解析**：`tantivy::query::QueryParser`，AND 默认，`set_allow_leading_wildcard(true)`
- **过滤**：`index_id`/`type` 用 TermQuery，`size` 用 RangeQuery，组合成 BooleanQuery
- **分页**：`collector::Top(k = (page+1) * PAGE_SIZE)` 再切片，PAGE_SIZE=50
- **高亮**：`SnippetGenerator::snippet(&searcher, &query, &text)` → `snippet.highlighted()` 返回 ranges
- **中断**：搜索前检查 `AtomicBool`，置位则提前返回（替代 DocFetcher 抛异常）

## 8. OCR 管道（ocr-pipeline，feature gate `ocr`）

- **启用**：`cargo build --features ocr`，引入 `kreuzberg-tesseract`
- **图片识别**：jpg/png/tiff 直接喂 Tesseract
- **扫描件 PDF**：`pdfium-render` 渲染每页为图片 → Tesseract 识别
- **语言包**：首次启用 OCR 时按需下载 `chi_sim`/`eng` 等 `.traineddata` 到用户数据目录，不进默认包
- **Parser 注册**：`ImageOcrParser` 注册到 parser_registry，扩展名 jpg/jpeg/png/tiff，与其他 parser 走同样的两级选择
- **质量兜底**：OCR 失败（清晰度低/无文字）时返回空 content，不阻塞索引

## 9. Tauri 桥接（desktop-ui）

```rust
// src-tauri/ 命令
#[tauri::command]
async fn add_index(path: String, state: State<EngineState>) -> Result<String, String>;

#[tauri::command]
async fn search(query: String, filters: SearchFilters, state: State<EngineState>) -> Result<SearchResponse, String>;

#[tauri::command]
async fn get_preview(uid: String, state: State<EngineState>) -> Result<PreviewData, String>;

#[tauri::command]
async fn list_indexes(state: State<EngineState>) -> Result<Vec<IndexInfo>, String>;

#[tauri::command]
async fn remove_index(id: String, state: State<EngineState>) -> Result<(), String>;

#[tauri::command]
async fn rebuild_index(id: String, state: State<EngineState>) -> Result<(), String>;
```

- **后台线程**：索引跑在独立 `std::thread`，不阻塞命令返回
- **进度推送**：`app.emit("index-progress", ProgressPayload)`，前端 `listen("index-progress", ...)` 更新进度条
- **状态共享**：`tauri::Manager` 的 `State<Mutex<EngineState>>`

## 10. 关键风险与缓解

| 风险 | 缓解 |
|---|---|
| PDFium 三端静态链接复杂 | 各平台 CI 配置预编译 PDFium binary，`static-bindings` feature |
| Tesseract 包体膨胀 | feature gate 默认关，语言包按需下载 |
| Tantivy schema 不可变 | 设计阶段定死 8 字段，未来演进走 reindex + 版本号 |
| macOS FSEvents 不精确 | 当"重扫提示"用 + mtime 校验兜底 |
| 中文分词质量 | jieba-rs + 自定义词典 + 停用词表，中英混排用 ngram 兜底 |
| 老格式 .doc/.ppt 不支持 | UI 明确提示转换，文档说明 |
| 净室合规 | 每次产出后 grep 检查无 DocFetcher 标识符残留 |
