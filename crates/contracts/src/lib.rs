//! # pivotsearch-contracts
//!
//! 契约层：trait 定义 + 数据结构 + 错误类型。
//! 这是依赖终点——不依赖任何内部 crate，所有能力 crate 只依赖本 crate。
//!
//! 依赖方向铁律：core 编排层只依赖本 crate 的 trait，绝不 import 具体实现。

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
