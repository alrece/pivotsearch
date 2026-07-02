//! Searcher trait + 搜索请求/响应/结果。

use crate::error::Result;
use crate::types::{IndexId, Uid};
use serde::{Deserialize, Serialize};

/// 搜索请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct SearchRequest {
    /// 查询字符串（支持 term/phrase/boolean/wildcard，AND 默认）。
    pub query: String,
    /// 限定搜索的索引根；None = 搜全部。
    pub index_ids: Option<Vec<IndexId>>,
    /// 类型过滤（parser 名，如 ["PdfParser"]）；None = 不过滤。
    pub parsers: Option<Vec<String>>,
    /// 文件大小下限（字节）；None = 无下限。
    pub min_size: Option<i64>,
    /// 文件大小上限（字节）；None = 无上限。
    pub max_size: Option<i64>,
    /// 页码（0-based）。
    pub page: usize,
    /// 大小写敏感（true 时对召回结果做精确大小写二次过滤）。
    #[serde(default)]
    pub case_sensitive: bool,
}


/// 搜索响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub total_hits: usize,
    pub results: Vec<SearchResult>,
    pub page: usize,
    pub page_count: usize,
}

/// 单条搜索结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub uid: Uid,
    pub path: String,
    pub title: String,
    /// SnippetGenerator 产出的命中片段（含高亮标记）。
    pub snippet: String,
    pub score: f32,
    pub size: i64,
    pub last_modified: i64,
    pub parser: String,
    pub index_id: IndexId,
}

/// 搜索引擎抽象（具体实现在 search crate）。
pub trait Searcher: Send + Sync {
    /// 执行搜索。
    fn search(&self, request: &SearchRequest) -> Result<SearchResponse>;

    /// 获取预览数据（重新解析原文件，返回渲染所需内容）。
    fn get_preview(&self, uid: &Uid) -> Result<PreviewData>;
}

/// 预览数据（点击结果项时获取）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewData {
    pub uid: Uid,
    pub path: String,
    pub parser: String,
    /// 重新解析的全文（或渲染指令，如 PDF 页面图片）。
    pub content: String,
    pub exists: bool, // false = 文件已删除/移动（可移动介质场景）
}

/// 每页结果数。
pub const PAGE_SIZE: usize = 50;
