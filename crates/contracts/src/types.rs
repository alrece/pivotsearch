//! 公共数据结构。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 索引根 ID（唯一标识一个索引根目录）。
pub type IndexId = String;

/// 文档 UID，格式 `file://{canonical_path}`，作为主键。
pub type Uid = String;

/// 已索引文档的元信息（存储在 tree_index SQLite）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedDoc {
    /// 主键 `file://{canonical_path}`。
    pub uid: Uid,
    /// 规范化后的绝对路径。
    pub path: PathBuf,
    /// 文件修改时间（毫秒时间戳）。
    pub mtime: i64,
    /// 解析器名（如 "PdfParser"）；None 表示解析失败（仍记录避免重试）。
    pub parser: Option<String>,
    /// 所属索引根 ID。
    pub index_id: IndexId,
}

impl IndexedDoc {
    /// 计算路径的 UID：`file://{canonical_path}`。
    /// Phase 1 实现时用 std::fs::canonicalize 规范化。
    pub fn compute_uid(canonical_path: &str) -> Uid {
        format!("file://{canonical_path}")
    }
}
