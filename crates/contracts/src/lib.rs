//! # pivotsearch-contracts
//!
//! Contracts layer: trait definitions + data structures + error types.
//! This is the dependency endpoint — it depends on no internal crate; all
//! capability crates depend only on this crate.
//!
//! Dependency rule: the core orchestration layer depends only on the traits in
//! this crate and never imports a concrete implementation.

pub mod error;
pub mod parser;
pub mod indexer;
pub mod searcher;
pub mod watcher;
pub mod types;

pub use error::{PivotsearchError, Result};
pub use parser::{Parser, ParseResult, ParserRegistry, extract_path_from_uid};
pub use indexer::{Indexer, IndexAction, UpdateResult, IndexRootInfo};
pub use searcher::{Searcher, SearchRequest, SearchResponse, SearchResult, PreviewData, PAGE_SIZE};
pub use watcher::{Watcher, WatchEvent, WatchEventKind};
pub use types::{IndexId, Uid, IndexedDoc};
