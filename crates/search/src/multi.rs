//! Multi-index merged search.
//!
//! One Tantivy Index per index root (one-index-root-per-directory design).
//! MultiIndexSearcher holds multiple (index_id, SimpleSearcher) pairs,
//! running a top-N query against each and merging them into a global top-N.
//!
//! Supported filters:
//! - index_ids: restrict the search to certain index roots (None = search all)
//! - parsers: filter by the parser field
//! - min_size/max_size: range filter on the size field

use crate::SimpleSearcher;
use pivotsearch_contracts::{Result, SearchRequest, SearchResponse, SearchResult};

/// Multi-index search engine.
pub struct MultiIndexSearcher {
    /// index_id → (SimpleSearcher, this index's original index for rebuilding the reader)
    searchers: Vec<(String, SimpleSearcher)>,
}

impl MultiIndexSearcher {
    pub fn new() -> Self {
        Self { searchers: Vec::new() }
    }

    /// Add a searcher for an index root.
    pub fn add(&mut self, index_id: String, searcher: SimpleSearcher) {
        self.searchers.push((index_id, searcher));
    }

    /// Number of index roots currently managed.
    pub fn index_count(&self) -> usize {
        self.searchers.len()
    }

    /// Search across multiple indexes.
    ///
    /// Runs a top-N query against each qualifying index, merges the results, then sorts globally to take the final page.
    pub fn search(&self, request: &SearchRequest) -> Result<SearchResponse> {
        let page_size = pivotsearch_contracts::PAGE_SIZE;
        let _limit = (request.page + 1) * page_size;

        // Determine which indexes to search
        let target_ids: Vec<&str> = match &request.index_ids {
            Some(ids) => ids.iter().map(|s| s.as_str()).collect(),
            None => self.searchers.iter().map(|(id, _)| id.as_str()).collect(),
        };

        let mut all_results: Vec<SearchResult> = Vec::new();
        let mut total_hits = 0usize;

        for (index_id, searcher) in &self.searchers {
            // Skip indexes not in the target list
            if !target_ids.contains(&index_id.as_str()) {
                continue;
            }
            // Query `limit` documents from each index
            let sub_request = SearchRequest {
                query: request.query.clone(),
                index_ids: None,
                parsers: request.parsers.clone(),
                min_size: request.min_size,
                max_size: request.max_size,
                page: 0,
                case_sensitive: request.case_sensitive,
            };
            match searcher.search(&sub_request) {
                Ok(response) => {
                    total_hits += response.total_hits;
                    all_results.extend(response.results);
                }
                Err(e) => {
                    // A single corrupted index is not fatal; skip it and log
                    tracing::warn!("索引 {} 查询失败，跳过: {}", index_id, e);
                }
            }
        }

        // Simple merge: keep result order (each index is already sorted by relevance internally); do not recompute scores
        // Phase 1 score=0, so keep the original order, take the first `limit`, then slice
        let start = request.page * page_size;
        let end = ((request.page + 1) * page_size).min(all_results.len());

        if start >= all_results.len() {
            return Ok(SearchResponse {
                total_hits,
                results: Vec::new(),
                page: request.page,
                page_count: ((total_hits as f64) / page_size as f64).ceil() as usize,
            });
        }

        let page_results: Vec<SearchResult> = all_results[start..end].to_vec();
        let page_count = ((total_hits as f64) / page_size as f64).ceil() as usize;

        Ok(SearchResponse {
            total_hits,
            results: page_results,
            page: request.page,
            page_count: page_count.max(1),
        })
    }
}

impl Default for MultiIndexSearcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    // Note: full tests for multi-index merging require constructing multiple Tantivy indexes;
    // here we only test the basic behavior of an empty searcher. Full integration tests are verified end-to-end in the cli.
    use super::*;

    #[test]
    fn empty_searcher_returns_empty() {
        let searcher = MultiIndexSearcher::new();
        assert_eq!(searcher.index_count(), 0);

        let request = SearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        let response = searcher.search(&request).unwrap();
        assert_eq!(response.total_hits, 0);
        assert!(response.results.is_empty());
    }
}
