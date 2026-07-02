# 02 — 架构设计

## 总体架构

```
┌─────────────────────────────────────────────────────────────┐
│  Tauri 2 前端（Vue 3 + Element Plus）                        │
│  SearchView / IndexManage / Settings                         │
├─────────────────────────────────────────────────────────────┤
│  #[tauri::command] 桥接（src-tauri/）                         │
│  add_index / search / get_preview / list_indexes / ...       │
├─────────────────────────────────────────────────────────────┤
│  core 编排层（PivotsearchEngine 总入口）                      │
│  组装 parser + index + watcher + queue + search              │
├─────────────────────────────────────────────────────────────┤
│  contracts（依赖终点）                                       │
│  Parser / Indexer / Searcher / Watcher trait                 │
│  + ParseResult / IndexedDoc / Task / SearchRequest 数据结构  │
│  + PivotsearchError                                          │
╔═══════════════════════════════════════════════════════════════╗
║  各能力 crate（都只依赖 contracts，互不依赖）                  ║
║  ┌────────┬────────┬────────┬────────┬────────┬────────┐    ║
║  │ parser │ index  │watcher │ queue  │ search │  ocr   │    ║
║  │ 解析层  │ 索引层  │ 监听层  │ 队列层  │ 查询层  │(可选)  │    ║
║  └────────┴────────┴────────┴────────┴────────┴────────┘    ║
╚═══════════════════════════════════════════════════════════════╝
```

## 依赖方向铁律

```
cli / src-tauri  →  core  →  contracts  ←  parser / index / watcher / queue / search / ocr
```

- `contracts` 是依赖终点，不依赖任何内部 crate
- `core` 编排层只依赖 `contracts` trait，**绝不 import 具体实现**
- 只有 `cli`/`src-tauri`（组装根）能 import 具体实现
- 各能力 crate 互不依赖，只通过 contracts trait 交互

## Crate 职责

| crate | 职责 | 关键依赖 |
|---|---|---|
| `contracts` | trait + 数据结构 + 错误类型 | thiserror, serde |
| `parser` | Parser 注册表 + 各格式解析器 | pdfium-render, calamine, pulldown-cmark, scraper, encoding_rs, chardetng, epub |
| `index` | Tantivy 封装 + schema + Document 组装 + 增量 + tree_index(SQLite) | tantivy, jieba-rs, rusqlite, walkdir |
| `watcher` | notify 监听 + 防抖 + 过滤 + mtime 校验 | notify, notify-debouncer-full |
| `queue` | 任务队列 + Task 状态机 | crossbeam-channel, parking_lot |
| `search` | 多索引合并 + 查询解析 + 分页 + 高亮 | tantivy |
| `ocr` | Tesseract 集成（feature gate） | kreuzberg-tesseract |
| `core` | 编排层，PivotsearchEngine 总入口 | contracts |
| `cli` | CLI binary（开发期调试） | core + 全部实现 |

## 数据流

### 索引流（写入路径）
```
用户添加索引根
  → src-tauri add_index command
  → core.engine.add_index(path)
  → queue 入队 Create Task
  → 单 worker 取出：
      → index.update_index(path)
          → walkdir 遍历
          → parser_registry.parse(每文件)
          → tantivy IndexWriter add_document / delete_term
          → commit
          → tree_index SQLite 持久化
      → 返回 SuccessChanged/Unchanged
  → 进度 emit("index-progress")
  → watcher 开始监听 path
```

### 查询流（读取路径）
```
用户输入查询
  → src-tauri search command（debounce 200ms）
  → core.engine.search(query, filters)
  → search 多索引合并：
      → 每索引 Searcher 各跑 top-N
      → 合并全局 top-N
      → SnippetGenerator 生成高亮片段
  → 返回 SearchResponse
  → 前端渲染结果列表
```

### 增量流（监听路径）
```
文件变化
  → notify 收到事件
  → notify-debouncer-full 防抖（1s 单 flight）
  → watcher.filter 过滤（索引目录/临时文件/不可解析）
  → 查 tree_index mtime 二次校验
  → 通过 → queue 入队 Update Task
  → worker 执行增量算法（unseenDocs diff）
  → commit + 持久化
```

## Tantivy 关键约束（影响架构）

| 约束 | 影响 | 应对 |
|---|---|---|
| schema 不可变 | 字段定死后不能加 | 设计阶段定 8 字段，演进走 reindex |
| 单 writer 强约束 | 同目录同时只能一个 writer | queue 单工作线程串行，不同索引根可各自 writer |
| 无原生 upsert | update = delete + add | delete_term(uid) + add_document，commit 后生效 |
| 无 term-vector offset | Lucene FastVector 无对应 | SnippetGenerator 重切文本统一高亮 |

## 持久化布局

```
{user_data_dir}/pivotsearch/
├── indexes/
│   ├── {index_id_1}/          # 每索引根一目录
│   │   ├── tantivy/           # Tantivy 索引文件
│   │   └── tree_index.sqlite  # 该索引根的文件树元数据
│   └── {index_id_2}/
├── registry.sqlite            # 全局：index_roots 表
├── config.json                # 用户设置
└── tessdata/                  # OCR 语言包（按需下载，feature gate）
    ├── chi_sim.traineddata
    └── eng.traineddata
```

一索引根一目录（复刻 DocFetcher 设计），目录内 Tantivy 索引 + 独立 tree_index.sqlite。
