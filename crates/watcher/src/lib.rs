//! # pivotsearch-watcher
//!
//! File watching layer: notify + debouncing + event filtering + mtime second-pass verification.
//!
//! Design:
//! - notify-debouncer-full provides a 1s single-flight debounce (an editor save that fires N events emits only 1)
//! - Event filtering: skip lock/hidden files and the index directory itself (prevents a self-feedback loop)
//! - mtime second-pass verification: the watcher command caller holds the TreeIndex and, for modify events,
//!   compares mtime to suppress noise
//!
//! The watcher only emits valid WatchEvents; index updates are consumed from a queue (decoupled dependencies).

use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use notify::Watcher as _NotifyTrait;
use parking_lot::Mutex;
use pivotsearch_contracts::{Result, WatchEvent, WatchEventKind, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Event filter configuration.
#[derive(Clone, Default)]
pub struct WatchFilter {
    /// File name patterns to skip (lock/hidden files are skipped by default).
    pub skip_suffixes: Vec<String>,
    /// Directories to skip (e.g. the index directory itself, to prevent self-feedback).
    pub skip_dirs: Vec<PathBuf>,
}

impl WatchFilter {
    /// Determines whether a path should be filtered out (skipped).
    pub fn should_skip(&self, path: &Path) -> bool {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Skip lock files
        if name.ends_with(".lock") {
            return true;
        }
        // Skip hidden files
        if name.starts_with('.') {
            return true;
        }
        // Skip files under the specified directories
        for skip_dir in &self.skip_dirs {
            if path.starts_with(skip_dir) {
                return true;
            }
        }
        // Skip specified suffixes
        for suffix in &self.skip_suffixes {
            if name.ends_with(suffix.as_str()) {
                return true;
            }
        }
        false
    }
}

/// Default watcher implementation.
///
/// Holds a notify debouncer and maintains one watch per index_id.
/// Events are received via an internal channel, filtered, then forwarded to a user-supplied
/// callback or channel.
pub struct PivotWatcher {
    /// Debouncer handle (kept alive).
    debouncers: Mutex<HashMap<String, Debouncer<notify::RecommendedWatcher, FileIdMap>>>,
    /// Event filter.
    filter: Arc<WatchFilter>,
    /// index_id → watched path.
    paths: Mutex<HashMap<String, PathBuf>>,
}

impl PivotWatcher {
    pub fn new(filter: WatchFilter) -> Self {
        Self {
            debouncers: Mutex::new(HashMap::new()),
            filter: Arc::new(filter),
            paths: Mutex::new(HashMap::new()),
        }
    }

    /// Starts watching and sends events to the callback.
    ///
    /// notify-debouncer-full performs debouncing on a background thread; valid filtered events
    /// invoke the callback.
    pub fn watch_with_callback<F>(&self, index_id: &str, path: &Path, callback: F) -> Result<()>
    where
        F: Fn(WatchEvent) + Send + 'static,
    {
        let filter = self.filter.clone();
        let index_id_owned = index_id.to_string();

        // 1s debounce window (an editor save often fires multiple events)
        let mut debouncer = new_debouncer(
            Duration::from_secs(1),
            None,
            move |result: DebounceEventResult| {
                match result {
                    Ok(events) => {
                        for event in events {
                            // DebouncedEvent Deref<Target=Event>; event.paths is Vec<PathBuf>
                            let path = match event.paths.first() {
                                Some(p) => p.clone(),
                                None => continue,
                            };
                            // Filter
                            if filter.should_skip(&path) {
                                continue;
                            }
                            let kind = map_event_kind(&event.kind);
                            // Skip meaningless event variants
                            if kind.is_none() {
                                continue;
                            }
                            callback(WatchEvent {
                                index_id: index_id_owned.clone(),
                                kind: kind.unwrap(),
                                path: path.to_string_lossy().to_string(),
                            });
                        }
                    }
                    Err(errors) => {
                        tracing::warn!("watcher 错误: {:?}", errors);
                    }
                }
            },
        )
        .map_err(|e| pivotsearch_contracts::PivotsearchError::IndexIo(format!("watcher init: {e:?}")))?;

        debouncer
            .watcher()
            .watch(path, notify::RecursiveMode::Recursive)
            .map_err(|e| {
                pivotsearch_contracts::PivotsearchError::IndexIo(format!("watcher add: {e:?}"))
            })?;

        self.debouncers.lock().insert(index_id.to_string(), debouncer);
        self.paths.lock().insert(index_id.to_string(), path.to_path_buf());
        Ok(())
    }
}

impl Watcher for PivotWatcher {
    fn watch(&self, index_id: &str, path: &Path) -> Result<()> {
        // The Watcher trait's watch uses a no-op callback (actual watching is driven by watch_with_callback)
        // Here we provide a no-op default implementation; the real callback is set by the caller
        self.watch_with_callback(index_id, path, |_| {})
    }

    fn unwatch(&self, index_id: &str) -> Result<()> {
        self.debouncers.lock().remove(index_id);
        self.paths.lock().remove(index_id);
        Ok(())
    }

    fn watched_indexes(&self) -> Vec<String> {
        self.paths.lock().keys().cloned().collect()
    }
}

/// Maps notify EventKind → pivotsearch WatchEventKind.
fn map_event_kind(kind: &notify::EventKind) -> Option<WatchEventKind> {
    use notify::EventKind;
    match kind {
        EventKind::Create(_) => Some(WatchEventKind::Create),
        EventKind::Modify(notify::event::ModifyKind::Data(_)) => Some(WatchEventKind::Modify),
        EventKind::Modify(notify::event::ModifyKind::Name(_)) => Some(WatchEventKind::Create),
        EventKind::Remove(_) => Some(WatchEventKind::Remove),
        EventKind::Modify(_) => Some(WatchEventKind::Modify),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn filter_skips_lock_and_hidden() {
        let f = WatchFilter::default();
        assert!(f.should_skip(Path::new("/tmp/.tantivy-writer.lock")));
        assert!(f.should_skip(Path::new("/tmp/.hidden")));
        assert!(f.should_skip(Path::new("/tmp/data.lock")));
        assert!(!f.should_skip(Path::new("/tmp/readme.md")));
        assert!(!f.should_skip(Path::new("/tmp/notes.txt")));
    }

    #[test]
    fn filter_skips_index_dir() {
        let mut f = WatchFilter::default();
        f.skip_dirs.push(PathBuf::from("/tmp/idx"));
        assert!(f.should_skip(Path::new("/tmp/idx/segment1")));
        assert!(!f.should_skip(Path::new("/tmp/docs/readme.md")));
    }

    #[test]
    fn watcher_detects_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let watcher = PivotWatcher::new(WatchFilter::default());
        watcher
            .watch_with_callback("test-idx", dir.path(), move |event| {
                received_clone.lock().push(event);
            })
            .unwrap();

        // Create a file to trigger an event
        let path = dir.path().join("new.txt");
        std::fs::write(&path, "content").unwrap();

        // Wait for the debounce window (1s) + event propagation
        std::thread::sleep(Duration::from_millis(1500));

        let events = received.lock();
        assert!(
            !events.is_empty(),
            "应收到至少一个事件（创建 new.txt）"
        );
        // At least one event path should contain new.txt
        assert!(
            events.iter().any(|e| e.path.contains("new.txt")),
            "应有 new.txt 的事件，实际: {:?}",
            events.iter().map(|e| &e.path).collect::<Vec<_>>()
        );
    }

    #[test]
    fn watcher_filters_lock_files() {
        let dir = tempfile::tempdir().unwrap();
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let watcher = PivotWatcher::new(WatchFilter::default());
        watcher
            .watch_with_callback("test-idx", dir.path(), move |event| {
                received_clone.lock().push(event);
            })
            .unwrap();

        // Create a lock file (should be filtered)
        std::fs::write(dir.path().join(".tantivy-writer.lock"), "lock").unwrap();
        std::thread::sleep(Duration::from_millis(1500));

        let events = received.lock();
        assert!(
            events.iter().all(|e| !e.path.ends_with(".lock")),
            "lock 文件事件应被过滤"
        );
    }
}
