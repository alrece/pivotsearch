//! # pivotsearch-core
//!
//! Orchestration layer: PivotsearchEngine main entry point.
//!
//! Strict dependency-direction rule: this crate depends only on contracts traits and **never imports concrete implementations**.
//! Concrete implementations (parser/index/watcher/queue/search/ocr) are assembled and injected at the composition root by cli/src-tauri.

use pivotsearch_contracts::{
    Indexer, ParserRegistry, Searcher, Watcher,
};

/// pivotsearch engine main entry point.
///
/// Holds trait objects for each capability and orchestrates indexing and querying.
/// The composition root (cli/src-tauri) is responsible for constructing the concrete implementations and injecting them.
pub struct PivotsearchEngine {
    pub parser: Box<dyn ParserRegistry>,
    pub indexer: Box<dyn Indexer>,
    pub searcher: Box<dyn Searcher>,
    pub watcher: Box<dyn Watcher>,
}

impl PivotsearchEngine {
    /// Inject concrete implementations from the composition root.
    pub fn new(
        parser: Box<dyn ParserRegistry>,
        indexer: Box<dyn Indexer>,
        searcher: Box<dyn Searcher>,
        watcher: Box<dyn Watcher>,
    ) -> Self {
        Self { parser, indexer, searcher, watcher }
    }
}
