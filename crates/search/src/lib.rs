//! # pivotsearch-search
//!
//! 查询层：多索引合并 + 查询解析 + 分页 + 高亮。
//!
//! Phase 0 占位：具体实现见 Phase 1 (T3) + Phase 3 (T8)。

// Phase 1 将实现：
// - query.rs      tantivy QueryParser（AND 默认 / 通配符 / 范围）+ jieba tokenizer 注册
// - highlight.rs  SnippetGenerator 高亮
// Phase 3 将实现：
// - multi.rs      多索引合并（每索引独立 Searcher，合并 top-N）

pub use pivotsearch_contracts::{Searcher, SearchRequest, SearchResponse, SearchResult, PreviewData, PAGE_SIZE};
