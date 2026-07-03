//! Parser trait + ParseResult + ParserRegistry.

use crate::error::Result;
use crate::types::Uid;
use std::path::Path;

/// Result of parsing a single file (pure data structure).
///
/// The parsing layer is decoupled from the writing layer: the Parser only
/// produces this struct, and the index crate assembles the Tantivy Document.
#[derive(Debug, Clone, Default)]
pub struct ParseResult {
    /// Plain text of the document body (required; may be empty — e.g. an
    /// image-only PDF with no text layer).
    pub content: String,
    /// Title (if absent, the index crate falls back to the file name without extension).
    pub title: Option<String>,
    /// List of authors.
    pub authors: Vec<String>,
    /// Other metadata (Subject/Keywords, etc.), appended to content for indexing.
    pub misc_metadata: Vec<String>,
    /// Parser name (injected by ParserRegistry, not self-set by the Parser).
    pub parser_name: &'static str,
}

impl ParseResult {
    pub fn new(content: String) -> Self {
        Self {
            content,
            title: None,
            authors: Vec::new(),
            misc_metadata: Vec::new(),
            parser_name: "",
        }
    }
}

/// File parser trait.
///
/// Each format implements a Parser and registers it in the ParserRegistry.
/// Selection strategy: mime first (magic-byte detection) → extension fallback
/// → multi-parser fault-tolerant attempts.
pub trait Parser: Send + Sync {
    /// Extensions this parser handles (lowercase, no dot), e.g. ["pdf"].
    fn extensions(&self) -> &[&str];

    /// Mime types this parser declares, e.g. ["application/pdf"].
    fn mimes(&self) -> &[&str];

    /// Parse a single file and produce a plain-text result.
    fn parse(&self, path: &Path) -> Result<ParseResult>;

    /// Parser name (used for ParseResult.parser_name injection and the index field).
    fn name(&self) -> &'static str;
}

/// Abstraction of the parser registry (concrete implementation in the parser crate).
/// This trait lets the core orchestration layer avoid depending on a concrete implementation.
pub trait ParserRegistry: Send + Sync {
    /// Select a parser via the two-level strategy and parse.
    /// 1. Mime detection hit → try in order of match quality (fault tolerance)
    /// 2. Extension fallback → first exact match
    /// 3. Last resort → UnsupportedFormat or index only the file name
    fn parse(&self, path: &Path) -> Result<ParseResult>;

    /// Whether the extension can be handled by any parser (used for watcher event filtering).
    fn can_parse_by_name(&self, file_name: &str) -> bool;

    /// List the names of all registered parsers (for debugging/settings page).
    fn list_parser_names(&self) -> Vec<&'static str>;
}

/// UID extraction used internally (recover path from uid).
pub fn extract_path_from_uid(uid: &Uid) -> Option<&str> {
    uid.strip_prefix("file://")
}
