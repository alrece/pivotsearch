//! # pivotsearch-index
//!
//! Index layer: Tantivy schema + Document assembly + incremental algorithm + tree_index (SQLite).

pub mod schema;
pub mod tokenizer;
pub mod doc_builder;
pub mod tree_index;
pub mod incremental;

pub use schema::{build_schema, SchemaFields, field_names, JIEBA_TOKENIZER_NAME};
pub use doc_builder::{build_document, compute_uid, extract_path};
pub use tree_index::{TreeIndex, IndexedFile, IndexRoot};
pub use incremental::{update_incremental, update_incremental_with_progress, IncrementalConfig, IncrementalStats};
