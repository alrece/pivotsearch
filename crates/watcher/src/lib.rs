//! # pivotsearch-watcher
//!
//! 监听层：notify + 防抖 + 事件过滤 + mtime 二次校验。
//!
//! Phase 0 占位：具体实现见 Phase 2 (T5)。

// Phase 2 将实现：
// - debounce.rs  notify-debouncer-full 集成（1s 单 flight 合并）
// - filter.rs    事件过滤（索引目录/Word 临时文件/不可解析文件 + mtime 二次校验）

pub use pivotsearch_contracts::{Watcher, WatchEvent, WatchEventKind};
