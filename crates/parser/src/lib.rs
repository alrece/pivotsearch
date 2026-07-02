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
pub mod epub;
pub mod pptx;
pub mod archive;

pub use registry::ParserRegistryImpl;
pub use archive::{is_archive, parse_archive};
pub use pivotsearch_contracts::{Parser, ParseResult, ParserRegistry};
