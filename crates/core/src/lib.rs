//! # pivotsearch-core
//!
//! 编排层：PivotsearchEngine 总入口。
//!
//! 依赖方向铁律：本 crate 只依赖 contracts trait，**绝不 import 具体实现**。
//! 具体实现（parser/index/watcher/queue/search/ocr）由 cli/src-tauri 组装根注入。

use pivotsearch_contracts::{
    Indexer, ParserRegistry, Searcher, Watcher,
};

/// pivotsearch 引擎总入口。
///
/// 持有各能力的 trait object，编排索引与查询。
/// 组装根（cli/src-tauri）负责构造具体实现并注入。
pub struct PivotsearchEngine {
    pub parser: Box<dyn ParserRegistry>,
    pub indexer: Box<dyn Indexer>,
    pub searcher: Box<dyn Searcher>,
    pub watcher: Box<dyn Watcher>,
}

impl PivotsearchEngine {
    /// 由组装根注入具体实现。
    pub fn new(
        parser: Box<dyn ParserRegistry>,
        indexer: Box<dyn Indexer>,
        searcher: Box<dyn Searcher>,
        watcher: Box<dyn Watcher>,
    ) -> Self {
        Self { parser, indexer, searcher, watcher }
    }
}
