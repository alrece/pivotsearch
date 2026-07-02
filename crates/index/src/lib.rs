//! # pivotsearch-index
//!
//! 索引层：Tantivy schema + Document 组装 + uid 算法 +（Phase 2）增量算法 + tree_index。
//!
//! 依赖方向：只依赖 contracts，不依赖其他能力 crate。

pub mod schema;
pub mod tokenizer;
pub mod doc_builder;

pub use schema::{build_schema, SchemaFields, field_names};
pub use doc_builder::{build_document, compute_uid, extract_path};

// Phase 2 将实现：
// - incremental.rs   mtime 比对 + unseenDocs diff + 归档跳过
// - tree_index.rs    SQLite 持久化 tree_index
