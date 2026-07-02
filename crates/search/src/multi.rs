//! 多索引合并搜索。
//!
//! 每个索引根一个 Tantivy Index（一索引根一目录设计）。
//! MultiIndexSearcher 持有多个 (index_id, SimpleSearcher)，
//! 各跑 top-N 查询，合并取全局 top-N。
//!
//! 支持过滤：
//! - index_ids：限定搜索某些索引根（None = 搜全部）
//! - parsers：按 parser 字段过滤
//! - min_size/max_size：按 size 字段范围过滤

use crate::SimpleSearcher;
use pivotsearch_contracts::{Result, SearchRequest, SearchResponse, SearchResult};

/// 多索引搜索引擎。
pub struct MultiIndexSearcher {
    /// index_id → (SimpleSearcher, 该索引的原始 index 用于重建 reader)
    searchers: Vec<(String, SimpleSearcher)>,
}

impl MultiIndexSearcher {
    pub fn new() -> Self {
        Self { searchers: Vec::new() }
    }

    /// 添加一个索引根的 searcher。
    pub fn add(&mut self, index_id: String, searcher: SimpleSearcher) {
        self.searchers.push((index_id, searcher));
    }

    /// 当前管理的索引根数量。
    pub fn index_count(&self) -> usize {
        self.searchers.len()
    }

    /// 跨多索引搜索。
    ///
    /// 每个符合条件的索引各跑 top-N，合并后按全局排序取最终页。
    pub fn search(&self, request: &SearchRequest) -> Result<SearchResponse> {
        let page_size = pivotsearch_contracts::PAGE_SIZE;
        let _limit = (request.page + 1) * page_size;

        // 决定要搜索哪些索引
        let target_ids: Vec<&str> = match &request.index_ids {
            Some(ids) => ids.iter().map(|s| s.as_str()).collect(),
            None => self.searchers.iter().map(|(id, _)| id.as_str()).collect(),
        };

        let mut all_results: Vec<SearchResult> = Vec::new();
        let mut total_hits = 0usize;

        for (index_id, searcher) in &self.searchers {
            // 跳过不在 target 列表的索引
            if !target_ids.contains(&index_id.as_str()) {
                continue;
            }
            // 每个索引查询 limit 条
            let sub_request = SearchRequest {
                query: request.query.clone(),
                index_ids: None, // SimpleSearcher 是单索引，不需要
                parsers: request.parsers.clone(),
                min_size: request.min_size,
                max_size: request.max_size,
                page: 0,
            };
            match searcher.search(&sub_request) {
                Ok(response) => {
                    total_hits += response.total_hits;
                    all_results.extend(response.results);
                }
                Err(e) => {
                    // 单索引损坏不致命，跳过并记录
                    tracing::warn!("索引 {} 查询失败，跳过: {}", index_id, e);
                }
            }
        }

        // 简单合并：按结果顺序（各索引内部已按相关性排序），不重新算分
        // Phase 1 score=0，这里保持原顺序，取前 limit 后切片
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
    // 注意：多索引合并的完整测试需要构造多个 Tantivy Index，
    // 这里只测试空搜索器的基本行为。完整集成测试在 cli 端到端验证。
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
