//! # pivotsearch-search
//!
//! 查询层：单索引查询 + 高亮 + 分页（Phase 1）。
//! 多索引合并见 multi.rs（Phase 3）。

pub mod multi;

pub use multi::MultiIndexSearcher;

use pivotsearch_contracts::{
    PivotsearchError, Result, SearchRequest, SearchResponse, SearchResult,
};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::Value;
use tantivy::snippet::SnippetGenerator;
use tantivy::Index;

/// 搜索引擎字段句柄。
#[derive(Clone)]
pub struct SearchSchemaFields {
    pub uid: tantivy::schema::Field,
    pub content: tantivy::schema::Field,
    pub title: tantivy::schema::Field,
    pub author: tantivy::schema::Field,
    pub r#type: tantivy::schema::Field,
    pub parser: tantivy::schema::Field,
    pub size: tantivy::schema::Field,
    pub last_modified: tantivy::schema::Field,
    pub index_id: tantivy::schema::Field,
}

/// 单索引搜索引擎。
pub struct SimpleSearcher {
    index: Index,
    reader: tantivy::IndexReader,
    fields: SearchSchemaFields,
    query_parser: QueryParser,
}

impl SimpleSearcher {
    pub fn new(
        index: Index,
        fields: SearchSchemaFields,
        _tokenizer_manager: tantivy::tokenizer::TokenizerManager,
    ) -> Result<Self> {
        let reader = index
            .reader_builder()
            .reload_policy(tantivy::ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| PivotsearchError::IndexIo(format!("reader build: {e}")))?;

        // QueryParser 查询 content 字段；tokenizer 由 schema field 的 indexing options 决定（已配 jieba）
        let query_parser = QueryParser::for_index(&index, vec![fields.content]);

        Ok(Self {
            index,
            reader,
            fields,
            query_parser,
        })
    }

    /// 执行搜索（单索引），返回带高亮结果。
    pub fn search(&self, request: &SearchRequest) -> Result<SearchResponse> {
        let searcher = self.reader.searcher();
        let query = self
            .query_parser
            .parse_query(&request.query)
            .map_err(|e| PivotsearchError::IndexIo(format!("query parse: {e}")))?;

        let page_size = pivotsearch_contracts::PAGE_SIZE;
        let limit = (request.page + 1) * page_size;

        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(limit))
            .map_err(|e| PivotsearchError::IndexIo(format!("search: {e}")))?;

        let total_hits = top_docs.len();
        let start = request.page * page_size;
        let end = ((request.page + 1) * page_size).min(total_hits);

        let mut snippet_generator =
            SnippetGenerator::create(&searcher, &*query, self.fields.content)
                .map_err(|e| PivotsearchError::IndexIo(format!("snippet: {e}")))?;
        snippet_generator.set_max_num_chars(200);

        let mut results = Vec::new();
        for (_score, doc_address) in top_docs.iter().skip(start).take(end - start) {
            let doc: tantivy::TantivyDocument = searcher
                .doc(*doc_address)
                .map_err(|e| PivotsearchError::IndexIo(format!("doc fetch: {e}")))?;

            let uid = doc_get_text(&doc, self.fields.uid).unwrap_or_default();
            let path = uid
                .strip_prefix("file://")
                .map(|s| s.to_string())
                .unwrap_or_default();
            let title = doc_get_text(&doc, self.fields.title).unwrap_or_default();
            let parser = doc_get_text(&doc, self.fields.parser).unwrap_or_default();
            let index_id = doc_get_text(&doc, self.fields.index_id).unwrap_or_default();
            let size = doc
                .get_first(self.fields.size)
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let last_modified = doc
                .get_first(self.fields.last_modified)
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            let snippet_text = if title.is_empty() { path.clone() } else { title.clone() };
            let snippet = snippet_generator.snippet(&snippet_text).to_html();

            results.push(SearchResult {
                uid,
                path,
                title,
                snippet,
                score: 0.0,
                size,
                last_modified,
                parser,
                index_id,
            });
        }

        let page_count = ((total_hits as f64) / page_size as f64).ceil() as usize;

        Ok(SearchResponse {
            total_hits,
            results,
            page: request.page,
            page_count: page_count.max(1),
        })
    }

    /// reader 引用（供 cli 写入后 reload）。
    pub fn reader(&self) -> &tantivy::IndexReader {
        &self.reader
    }

    /// index 引用。
    pub fn index(&self) -> &Index {
        &self.index
    }
}

fn doc_get_text(
    doc: &tantivy::TantivyDocument,
    field: tantivy::schema::Field,
) -> Option<String> {
    doc.get_first(field)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
}
