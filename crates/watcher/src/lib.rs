//! # pivotsearch-watcher
//!
//! 文件监听层：notify + 防抖 + 事件过滤 + mtime 二次校验。
//!
//! 设计：
//! - notify-debouncer-full 实现 1s 单 flight 防抖（编辑器保存触发 N 次事件只发 1 个）
//! - 事件过滤：跳过 lock/隐藏文件/索引目录自身（防自反馈死循环）
//! - mtime 二次校验：watcher 命令调用方持有 TreeIndex，对 modify 事件查 mtime 比对去噪
//!
//! watcher 只产出有效 WatchEvent，索引更新由 queue 消费（依赖解耦）。

use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use notify::Watcher as _NotifyTrait;
use parking_lot::Mutex;
use pivotsearch_contracts::{Result, WatchEvent, WatchEventKind, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// 事件过滤配置。
#[derive(Clone, Default)]
pub struct WatchFilter {
    /// 要跳过的文件名模式（lock/隐藏文件默认跳过）。
    pub skip_suffixes: Vec<String>,
    /// 要跳过的目录（如索引目录自身，防自反馈）。
    pub skip_dirs: Vec<PathBuf>,
}

impl WatchFilter {
    /// 判断一个路径是否应被过滤（跳过）。
    pub fn should_skip(&self, path: &Path) -> bool {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // 跳过 lock 文件
        if name.ends_with(".lock") {
            return true;
        }
        // 跳过隐藏文件
        if name.starts_with('.') {
            return true;
        }
        // 跳过指定目录下的文件
        for skip_dir in &self.skip_dirs {
            if path.starts_with(skip_dir) {
                return true;
            }
        }
        // 跳过指定后缀
        for suffix in &self.skip_suffixes {
            if name.ends_with(suffix.as_str()) {
                return true;
            }
        }
        false
    }
}

/// 默认 watcher 实现。
///
/// 持有 notify debouncer，对每个 index_id 维护一个 watch。
/// 事件通过内部 channel 接收、过滤、转发到用户提供的回调或 channel。
pub struct PivotWatcher {
    /// debouncer 句柄（保活用）。
    debouncers: Mutex<HashMap<String, Debouncer<notify::RecommendedWatcher, FileIdMap>>>,
    /// 事件过滤器。
    filter: Arc<WatchFilter>,
    /// index_id → 监听路径。
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

    /// 启动监听并把事件发送到 callback。
    ///
    /// notify-debouncer-full 在后台线程做防抖，过滤后的有效事件调用 callback。
    pub fn watch_with_callback<F>(&self, index_id: &str, path: &Path, callback: F) -> Result<()>
    where
        F: Fn(WatchEvent) + Send + 'static,
    {
        let filter = self.filter.clone();
        let index_id_owned = index_id.to_string();

        // 1s 防抖窗口（编辑器保存常触发多次事件）
        let mut debouncer = new_debouncer(
            Duration::from_secs(1),
            None,
            move |result: DebounceEventResult| {
                match result {
                    Ok(events) => {
                        for event in events {
                            // DebouncedEvent Deref<Target=Event>，event.paths 是 Vec<PathBuf>
                            let path = match event.paths.first() {
                                Some(p) => p.clone(),
                                None => continue,
                            };
                            // 过滤
                            if filter.should_skip(&path) {
                                continue;
                            }
                            let kind = map_event_kind(&event.kind);
                            // 跳过无意义的变体事件
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
        // Watcher trait 的 watch 用空回调（实际监听由 watch_with_callback 驱动）
        // 这里提供一个无操作的默认实现，真正的 callback 由调用方设置
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

/// notify EventKind → pivotsearch WatchEventKind。
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

        // 创建文件触发事件
        let path = dir.path().join("new.txt");
        std::fs::write(&path, "content").unwrap();

        // 等待防抖窗口（1s）+ 事件传播
        std::thread::sleep(Duration::from_millis(1500));

        let events = received.lock();
        assert!(
            !events.is_empty(),
            "应收到至少一个事件（创建 new.txt）"
        );
        // 至少有一个事件路径含 new.txt
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

        // 创建 lock 文件（应被过滤）
        std::fs::write(dir.path().join(".tantivy-writer.lock"), "lock").unwrap();
        std::thread::sleep(Duration::from_millis(1500));

        let events = received.lock();
        assert!(
            events.iter().all(|e| !e.path.ends_with(".lock")),
            "lock 文件事件应被过滤"
        );
    }
}
