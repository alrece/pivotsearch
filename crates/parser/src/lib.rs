//! # pivotsearch-parser
//!
//! Parsing layer: parser registry + per-format parser implementations.

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
