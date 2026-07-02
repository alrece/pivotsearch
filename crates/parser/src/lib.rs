//! # pivotsearch-parser
//!
//! 解析层：Parser 注册表 + 各格式解析器实现。
//!
//! Phase 0 占位：仅 re-export contracts，具体 parser 实现见 Phase 1 (T2)。

// Phase 1 将实现：
// - registry.rs  ParserRegistryImpl（两级选择：mime 优先 / 扩展名 fallback）
// - text.rs      TextParser（encoding_rs + chardetng）
// - markdown.rs  MarkdownParser（pulldown-cmark）
// - html.rs      HtmlParser（scraper）
// - pdf.rs       PdfParser（pdfium-render，静态链接）
// - docx.rs      DocxParser（docx-rs / ooxmlsdk）
// - xlsx.rs      SpreadsheetParser（calamine）

pub use pivotsearch_contracts::{Parser, ParseResult, ParserRegistry};
