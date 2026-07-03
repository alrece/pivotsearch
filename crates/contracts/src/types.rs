//! Common data structures.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Index root ID (uniquely identifies an index root directory).
pub type IndexId = String;

/// Document UID, format `file://{canonical_path}`, used as the primary key.
pub type Uid = String;

/// Metadata of an indexed document (stored in the tree_index SQLite).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedDoc {
    /// Primary key `file://{canonical_path}`.
    pub uid: Uid,
    /// Canonicalized absolute path.
    pub path: PathBuf,
    /// File modification time (millisecond timestamp).
    pub mtime: i64,
    /// Parser name (e.g. "PdfParser"); None means parsing failed (still recorded to avoid retry).
    pub parser: Option<String>,
    /// Owning index root ID.
    pub index_id: IndexId,
}

impl IndexedDoc {
    /// Compute the UID for a path: `file://{canonical_path}`.
    /// During Phase 1 the path is normalized via std::fs::canonicalize.
    pub fn compute_uid(canonical_path: &str) -> Uid {
        format!("file://{canonical_path}")
    }
}
