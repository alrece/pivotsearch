//! Parser registry + two-tier selection strategy.
//!
//! Design (clean-room reimplementation of the parser registry pattern from classic desktop search tools):
//! 1. MIME path: infer magic-number detection → sort by match score → try in order until one succeeds (fault tolerance)
//! 2. Extension path: exact extension match → pick the first matching parser
//! 3. Fallback: return UnsupportedFormat

use pivotsearch_contracts::{ParseResult, Parser, ParserRegistry, PivotsearchError, Result};
use std::path::Path;

/// Default implementation of the parser registry.
pub struct ParserRegistryImpl {
    parsers: Vec<Box<dyn Parser>>,
}

impl ParserRegistryImpl {
    /// Builds the registry with all built-in parsers.
    pub fn with_builtin_parsers() -> Self {
        let parsers: Vec<Box<dyn Parser>> = vec![
            Box::new(crate::text::TextParser),
            Box::new(crate::markdown::MarkdownParser),
            Box::new(crate::html::HtmlParser),
            Box::new(crate::docx::DocxParser),
            Box::new(crate::xlsx::SpreadsheetParser),
            Box::new(crate::epub::EpubParser),
            Box::new(crate::pptx::PptxParser),
        ];
        Self { parsers }
    }

    /// Enables PDF parsing on top of the defaults.
    pub fn with_pdf(mut self) -> Self {
        self.parsers.push(Box::new(crate::pdf::PdfParser::default()));
        self
    }

    /// Finds the first matching parser by exact extension lookup.
    fn find_by_extension(&self, file_name: &str) -> Option<&dyn Parser> {
        let ext = Path::new(file_name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());
        let ext = ext.as_deref()?;
        self.parsers
            .iter()
            .find(|p| p.extensions().contains(&ext))
            .map(|p| p.as_ref())
    }

    /// Detects MIME type via magic number and returns candidate parsers (further sorted by extension match score).
    fn find_by_mime(&self, path: &Path) -> Vec<&dyn Parser> {
        // Read the first 8K bytes for magic-number detection
        let bytes = std::fs::File::open(path)
            .and_then(|mut f| {
                use std::io::Read;
                let mut buf = vec![0u8; 8192];
                let n = f.read(&mut buf)?;
                buf.truncate(n);
                Ok(buf)
            })
            .ok();

        let detected = bytes
            .as_deref()
            .and_then(|b| infer::get(b).map(|t| t.mime_type().to_string()));

        let file_ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        let mut candidates: Vec<(&dyn Parser, usize)> = self
            .parsers
            .iter()
            .map(|p| p.as_ref())
            .filter_map(|p| {
                let mime_match = detected
                    .as_ref()
                    .map(|m| p.mimes().iter().any(|pm| *pm == m))
                    .unwrap_or(false);
                let ext_match = file_ext
                    .as_deref()
                    .map(|fe| p.extensions().contains(&fe))
                    .unwrap_or(false);
                // A MIME or extension match qualifies as a candidate
                if mime_match || ext_match {
                    let score = (mime_match as usize) * 2 + (ext_match as usize);
                    Some((p, score))
                } else {
                    None
                }
            })
            .collect();

        // Sort by score descending (MIME takes priority)
        candidates.sort_by(|a, b| b.1.cmp(&a.1));
        candidates.into_iter().map(|(p, _)| p).collect()
    }
}

impl ParserRegistry for ParserRegistryImpl {
    fn parse(&self, path: &Path) -> Result<ParseResult> {
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Archive traversal: unpack zip/tar and recursively parse inner files
        if crate::archive::is_archive(path) {
            return crate::archive::parse_archive(path, self);
        }

        // Path 1: magic-number MIME detection → fault-tolerant multi-parser attempts
        let candidates = self.find_by_mime(path);
        if !candidates.is_empty() {
            for parser in &candidates {
                match parser.parse(path) {
                    Ok(mut result) => {
                        result.parser_name = parser.name();
                        return Ok(result);
                    }
                    Err(PivotsearchError::UnsupportedFormat(_)) => continue,
                    Err(e) => return Err(e),
                }
            }
            // All candidates failed; fall through to the extension path
        }

        // Path 2: exact extension match
        if let Some(parser) = self.find_by_extension(file_name) {
            let mut result = parser.parse(path)?;
            result.parser_name = parser.name();
            return Ok(result);
        }

        // Path 3: fallback
        let ext = Path::new(file_name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        Err(PivotsearchError::UnsupportedFormat(ext.to_string()))
    }

    fn can_parse_by_name(&self, file_name: &str) -> bool {
        self.find_by_extension(file_name).is_some()
    }

    fn list_parser_names(&self) -> Vec<&'static str> {
        self.parsers.iter().map(|p| p.name()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_can_parse_known_extensions() {
        let reg = ParserRegistryImpl::with_builtin_parsers();
        assert!(reg.can_parse_by_name("readme.md"));
        assert!(reg.can_parse_by_name("notes.txt"));
        assert!(reg.can_parse_by_name("page.html"));
        assert!(reg.can_parse_by_name("data.xlsx"));
        assert!(!reg.can_parse_by_name("unknown.xyz"));
    }

    #[test]
    fn registry_lists_parser_names() {
        let reg = ParserRegistryImpl::with_builtin_parsers();
        let names = reg.list_parser_names();
        assert!(names.contains(&"TextParser"));
        assert!(names.contains(&"MarkdownParser"));
        assert!(names.contains(&"HtmlParser"));
    }
}
