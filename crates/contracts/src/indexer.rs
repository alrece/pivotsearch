//! Indexer trait + IndexAction + UpdateResult。

use crate::error::Result;
use crate::types::IndexId;
use std::path::Path;

/// 索引操作类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexAction {
    /// 增量更新（mtime 比对，只处理变化文件）。
    Update,
    /// 全量重建（清空后从头索引）。
    Rebuild,
}

/// 单次索引更新的结果三态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateResult {
    /// 有变化（新增/修改/删除），已持久化。
    SuccessChanged,
    /// 无变化（所有文件 mtime 未变），跳过持久化省 IO。
    SuccessUnchanged,
    /// 失败（记录但不崩溃）。
    Failure,
}

/// 索引器抽象（具体实现在 index crate）。
pub trait Indexer: Send + Sync {
    /// 添加新的索引根（创建索引目录 + 初始化 tree_index）。
    fn add_index(&self, path: &Path, display_name: Option<&str>) -> Result<IndexId>;

    /// 执行索引更新（增量或全量）。
    fn update(&self, index_id: &IndexId, action: IndexAction) -> Result<UpdateResult>;

    /// 移除索引根（删 Tantivy 目录 + tree_index 记录）。
    fn remove_index(&self, index_id: &IndexId) -> Result<()>;

    /// 列出所有索引根。
    fn list_indexes(&self) -> Result<Vec<IndexRootInfo>>;
}

/// 索引根信息（list_indexes 返回）。
#[derive(Debug, Clone)]
pub struct IndexRootInfo {
    pub id: IndexId,
    pub path: String,
    pub display_name: Option<String>,
    pub file_count: u64,
    pub index_size_bytes: u64,
}
