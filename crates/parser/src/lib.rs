//! # pivotsearch-parser
//!
//! 解析层：Parser 注册表 + 各格式解析器实现。

pub mod registry;
pub mod text;
pub mod markdown;
pub mod html;
pub mod docx;
pub mod xlsx;
pub mod pdf;

pub use registry::ParserRegistryImpl;
pub use pivotsearch_contracts::{Parser, ParseResult, ParserRegistry};
