//! Watcher trait + WatchEvent。

use crate::error::Result;
use crate::types::IndexId;
use std::path::Path;

/// 文件监听事件（去抖、过滤后的有效事件）。
#[derive(Debug, Clone)]
pub struct WatchEvent {
    pub index_id: IndexId,
    pub kind: WatchEventKind,
    pub path: String,
}

/// 事件种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchEventKind {
    Create,
    Modify,
    Remove,
    /// 目录级"有变化需重扫"提示（macOS FSEvents 常见）。
    RescanHint,
}

/// 文件监听器抽象（具体实现在 watcher crate）。
///
/// 监听器只负责产出有效事件，索引更新由 queue 消费。
pub trait Watcher: Send + Sync {
    /// 开始监听一个索引根目录。
    fn watch(&self, index_id: &IndexId, path: &Path) -> Result<()>;

    /// 停止监听一个索引根目录。
    fn unwatch(&self, index_id: &IndexId) -> Result<()>;

    /// 列出当前正在监听的索引根。
    fn watched_indexes(&self) -> Vec<IndexId>;
}
