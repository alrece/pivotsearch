//! # pivotsearch-search
//!
//! Query layer: single-index query + highlighting + pagination (Phase 1).
//! Multi-index merging is in multi.rs (Phase 3).

pub mod multi;
pub mod snippet;

pub use multi::MultiIndexSearcher;
pub use snippet::{make_snippet, text_covers_query};

use pivotsearch_contracts::{
    PivotsearchError, Result, SearchRequest, SearchResponse, SearchResult,
};
use std::path::Path;
use std::sync::Arc;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::Value;
use tantivy::Index;

/// Optional loader that returns full document text for accurate snippet windows.
/// Wired by the composition root (cli / tauri) using the parser registry.
pub type ContentLoader = Arc<dyn Fn(&Path) -> Option<String> + Send + Sync>;

/// Search engine field handles.
#[derive(Clone)]
pub struct SearchSchemaFields {
    pub uid: tantivy::schema::Field,
    pub content: tantivy::schema::Field,
    pub snippet_text: tantivy::schema::Field,
    pub title: tantivy::schema::Field,
    pub author: tantivy::schema::Field,
    pub r#type: tantivy::schema::Field,
    pub parser: tantivy::schema::Field,
    pub size: tantivy::schema::Field,
    pub last_modified: tantivy::schema::Field,
    pub index_id: tantivy::schema::Field,
}

/// Single-index search engine.
pub struct SimpleSearcher {
    index: Index,
    reader: tantivy::IndexReader,
    fields: SearchSchemaFields,
    query_parser: QueryParser,
    content_loader: Option<ContentLoader>,
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

        let query_parser = QueryParser::for_index(&index, vec![fields.content]);

        Ok(Self {
            index,
            reader,
            fields,
            query_parser,
            content_loader: None,
        })
    }

    /// Attach a content loader so snippets can be taken from the full document
    /// when the stored `snippet_text` prefix does not cover the query.
    pub fn with_content_loader(mut self, loader: ContentLoader) -> Self {
        self.content_loader = Some(loader);
        self
    }

    /// Execute a search (single index), returning highlighted results.
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

            let stored_snippet =
                doc_get_text(&doc, self.fields.snippet_text).unwrap_or_default();

            if request.case_sensitive {
                let source_for_case = resolve_snippet_source(
                    &path,
                    &stored_snippet,
                    &request.query,
                    request.case_sensitive,
                    self.content_loader.as_ref(),
                );
                if !source_for_case.is_empty()
                    && !text_covers_query(&source_for_case, &request.query, true)
                {
                    continue;
                }
            }

            let snippet_source = resolve_snippet_source(
                &path,
                &stored_snippet,
                &request.query,
                request.case_sensitive,
                self.content_loader.as_ref(),
            );

            let snippet = if snippet_source.is_empty() {
                title.clone()
            } else {
                make_snippet(&snippet_source, &request.query, request.case_sensitive)
            };

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

    /// Reader reference (for cli to reload after writing).
    pub fn reader(&self) -> &tantivy::IndexReader {
        &self.reader
    }

    /// Index reference.
    pub fn index(&self) -> &Index {
        &self.index
    }
}

fn resolve_snippet_source(
    path: &str,
    stored_snippet: &str,
    query: &str,
    case_sensitive: bool,
    loader: Option<&ContentLoader>,
) -> String {
    if text_covers_query(stored_snippet, query, case_sensitive) {
        return stored_snippet.to_string();
    }
    if let Some(loader) = loader {
        if !path.is_empty() {
            if let Some(full) = loader(Path::new(path)) {
                if !full.is_empty() {
                    return full;
                }
            }
        }
    }
    stored_snippet.to_string()
}

fn doc_get_text(
    doc: &tantivy::TantivyDocument,
    field: tantivy::schema::Field,
) -> Option<String> {
    doc.get_first(field)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
}
