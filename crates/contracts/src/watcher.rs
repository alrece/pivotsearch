//! Watcher trait + WatchEvent.

use crate::error::Result;
use crate::types::IndexId;
use std::path::Path;

/// File watcher event (a valid event after debouncing and filtering).
#[derive(Debug, Clone)]
pub struct WatchEvent {
    pub index_id: IndexId,
    pub kind: WatchEventKind,
    pub path: String,
}

/// Event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchEventKind {
    Create,
    Modify,
    Remove,
    /// Directory-level "something changed, rescan needed" hint (common with macOS FSEvents).
    RescanHint,
}

/// File watcher abstraction (concrete implementation in the watcher crate).
///
/// The watcher is only responsible for producing valid events; index updates
/// are consumed by the queue.
pub trait Watcher: Send + Sync {
    /// Start watching an index root directory.
    fn watch(&self, index_id: &str, path: &Path) -> Result<()>;

    /// Stop watching an index root directory.
    fn unwatch(&self, index_id: &str) -> Result<()>;

    /// List the index roots currently being watched.
    fn watched_indexes(&self) -> Vec<String>;
}
