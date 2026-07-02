//! pivotsearch 统一错误类型。

use thiserror::Error;

/// pivotsearch 所有组件返回的统一错误类型。
///
/// 设计原则：区分可重试（IO 瞬时失败）与永久（格式不支持、文件损坏），
/// 调用方可据此决定重试策略。
#[derive(Debug, Error)]
pub enum PivotsearchError {
    /// 不支持的文件格式（如 .doc/.ppt 老格式）。
    /// 提示用户转换为现代格式。
    #[error("unsupported format: .{0}, please convert to a modern format")]
    UnsupportedFormat(String),

    /// 文件解析失败（损坏的 PDF、编码错误等）。
    /// 文件本身的问题，重试无意义。
    #[error("parse failed for {path}: {reason}")]
    ParseFailed { path: String, reason: String },

    /// 索引 IO 错误（Tantivy 读写失败）。
    /// 可能瞬时，可重试。
    #[error("index io error: {0}")]
    IndexIo(String),

    /// 文件系统 IO 错误。
    #[error("fs io error at {path}: {source}")]
    FsIo {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// SQLite 元数据错误。
    #[error("sqlite error: {0}")]
    Sqlite(String),

    /// 索引根路径冲突（包含/被包含已有索引）。
    #[error("index path conflict: {0}")]
    PathConflict(String),

    /// schema 版本不匹配（需 reindex）。
    #[error("schema version mismatch: indexed={indexed}, current={current}, reindex required")]
    SchemaMismatch { indexed: u32, current: u32 },

    /// 其他未分类错误。
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// 便捷 Result 别名。
pub type Result<T> = std::result::Result<T, PivotsearchError>;
