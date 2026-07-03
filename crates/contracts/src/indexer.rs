//! Indexer trait + IndexAction + UpdateResult.

use crate::error::Result;
use crate::types::IndexId;
use std::path::Path;

/// Index operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexAction {
    /// Incremental update (mtime comparison; only changed files are processed).
    Update,
    /// Full rebuild (clears and indexes from scratch).
    Rebuild,
}

/// The three states resulting from a single index update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateResult {
    /// Changed (added/modified/deleted); persisted.
    SuccessChanged,
    /// Unchanged (all file mtimes unchanged); persistence skipped to save IO.
    SuccessUnchanged,
    /// Failed (recorded but does not crash).
    Failure,
}

/// Indexer abstraction (concrete implementation in the index crate).
pub trait Indexer: Send + Sync {
    /// Add a new index root (creates the index directory + initializes tree_index).
    fn add_index(&self, path: &Path, display_name: Option<&str>) -> Result<IndexId>;

    /// Perform an index update (incremental or full).
    fn update(&self, index_id: &IndexId, action: IndexAction) -> Result<UpdateResult>;

    /// Remove an index root (deletes the Tantivy directory + tree_index records).
    fn remove_index(&self, index_id: &IndexId) -> Result<()>;

    /// List all index roots.
    fn list_indexes(&self) -> Result<Vec<IndexRootInfo>>;
}

/// Index root info (returned by list_indexes).
#[derive(Debug, Clone)]
pub struct IndexRootInfo {
    pub id: IndexId,
    pub path: String,
    pub display_name: Option<String>,
    pub file_count: u64,
    pub index_size_bytes: u64,
}
