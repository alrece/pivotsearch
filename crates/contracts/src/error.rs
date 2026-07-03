//! Unified error type for pivotsearch.

use thiserror::Error;

/// Unified error type returned by all pivotsearch components.
///
/// Design principle: distinguish retryable (transient IO failures) from
/// permanent (unsupported format, corrupted file); the caller can decide a
/// retry strategy accordingly.
#[derive(Debug, Error)]
pub enum PivotsearchError {
    /// Unsupported file format (e.g. legacy .doc/.ppt formats).
    /// Suggest the user convert to a modern format.
    #[error("unsupported format: .{0}, please convert to a modern format")]
    UnsupportedFormat(String),

    /// File parse failure (corrupted PDF, encoding errors, etc.).
    /// A problem with the file itself; retrying is pointless.
    #[error("parse failed for {path}: {reason}")]
    ParseFailed { path: String, reason: String },

    /// Index IO error (Tantivy read/write failure).
    /// Possibly transient; may be retried.
    #[error("index io error: {0}")]
    IndexIo(String),

    /// Filesystem IO error.
    #[error("fs io error at {path}: {source}")]
    FsIo {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// SQLite metadata error.
    #[error("sqlite error: {0}")]
    Sqlite(String),

    /// Index root path conflict (contains / is contained by an existing index).
    #[error("index path conflict: {0}")]
    PathConflict(String),

    /// Schema version mismatch (reindex required).
    #[error("schema version mismatch: indexed={indexed}, current={current}, reindex required")]
    SchemaMismatch { indexed: u32, current: u32 },

    /// Other uncategorized error.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Convenience Result alias.
pub type Result<T> = std::result::Result<T, PivotsearchError>;
