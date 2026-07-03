//! Searcher trait + search request/response/result.

use crate::error::Result;
use crate::types::{IndexId, Uid};
use serde::{Deserialize, Serialize};

/// Search request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct SearchRequest {
    /// Query string (supports term/phrase/boolean/wildcard, AND by default).
    pub query: String,
    /// Restrict search to these index roots; None = search all.
    pub index_ids: Option<Vec<IndexId>>,
    /// Type filter (parser names, e.g. ["PdfParser"]); None = no filter.
    pub parsers: Option<Vec<String>>,
    /// Minimum file size in bytes; None = no lower bound.
    pub min_size: Option<i64>,
    /// Maximum file size in bytes; None = no upper bound.
    pub max_size: Option<i64>,
    /// Page number (0-based).
    pub page: usize,
    /// Case-sensitive (when true, performs an exact-case second-pass filter on recall results).
    #[serde(default)]
    pub case_sensitive: bool,
}


/// Search response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub total_hits: usize,
    pub results: Vec<SearchResult>,
    pub page: usize,
    pub page_count: usize,
}

/// A single search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub uid: Uid,
    pub path: String,
    pub title: String,
    /// Hit snippet produced by the SnippetGenerator (includes highlight markers).
    pub snippet: String,
    pub score: f32,
    pub size: i64,
    pub last_modified: i64,
    pub parser: String,
    pub index_id: IndexId,
}

/// Search engine abstraction (concrete implementation in the search crate).
pub trait Searcher: Send + Sync {
    /// Execute a search.
    fn search(&self, request: &SearchRequest) -> Result<SearchResponse>;

    /// Fetch preview data (re-parses the original file and returns the content needed for rendering).
    fn get_preview(&self, uid: &Uid) -> Result<PreviewData>;
}

/// Preview data (fetched when a result item is clicked).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewData {
    pub uid: Uid,
    pub path: String,
    pub parser: String,
    /// Re-parsed full text (or rendering instructions, e.g. PDF page images).
    pub content: String,
    pub exists: bool, // false = file has been deleted/moved (removable media scenario)
}

/// Number of results per page.
pub const PAGE_SIZE: usize = 50;
