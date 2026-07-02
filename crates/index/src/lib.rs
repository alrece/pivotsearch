//! # pivotsearch-index
//!
//! 索引层：Tantivy schema + Document 组装 + 增量算法 + tree_index（SQLite）。
//!
//! Phase 0 占位：具体实现见 Phase 1 (T1/T4)。

// Phase 1 将实现：
// - schema.rs        Tantivy schema 八字段定死 + uid 算法
// - doc_builder.rs   Document 组装（content 追加 title/author/文件名）
// Phase 2 将实现：
// - incremental.rs   mtime 比对 + unseenDocs diff + 归档跳过
// - tree_index.rs    SQLite 持久化 tree_index

pub use pivotsearch_contracts::{Indexer, IndexAction, UpdateResult, IndexedDoc};
