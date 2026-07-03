//! PDF parser (pdfium-render, dynamically linked to PDFium).
//!
//! PDFium is the same PDF engine used by Chromium, with mature support for Chinese (CID fonts).
//! At runtime the system must provide the PDFium shared library. Phase 5 switches to static linking.

use pdfium_render::prelude::*;
use pivotsearch_contracts::{ParseResult, Parser, PivotsearchError, Result};
use std::path::Path;

/// PDF parser.
///
/// Holds an optional Pdfium instance (thread-safe).
/// On construction it tries to bind the system PDFium library; if missing, pdfium=None
/// and parse() returns a clear error.
pub struct PdfParser {
    pdfium: Option<Pdfium>,
}

impl Default for PdfParser {
    fn default() -> Self {
        let pdfium = Pdfium::bind_to_system_library()
            .ok()
            .map(Pdfium::new);
        Self { pdfium }
    }
}

impl PdfParser {
    /// Constructs with an explicit Pdfium instance (for tests / static linking).
    pub fn with_pdfium(pdfium: Pdfium) -> Self {
        Self { pdfium: Some(pdfium) }
    }
}

impl Parser for PdfParser {
    fn extensions(&self) -> &[&str] {
        &["pdf"]
    }

    fn mimes(&self) -> &[&str] {
        &["application/pdf"]
    }

    fn parse(&self, path: &Path) -> Result<ParseResult> {
        let pdfium = self.pdfium.as_ref().ok_or_else(|| {
            PivotsearchError::ParseFailed {
                path: path.display().to_string(),
                reason: "PDFium 动态库未找到，请安装 PDFium 或启用静态链接 feature".to_string(),
            }
        })?;

        let document = pdfium.load_pdf_from_file(path, None).map_err(|e| {
            PivotsearchError::ParseFailed {
                path: path.display().to_string(),
                reason: format!("PDFium: {e}"),
            }
        })?;

        let pages = document.pages();
        let mut content = String::new();
        for page in pages.iter() {
            let page_text = page.text().map_err(|e| PivotsearchError::ParseFailed {
                path: path.display().to_string(),
                reason: format!("PDFium text: {e}"),
            })?;
            let text = page_text.all();
            if !text.trim().is_empty() {
                if !content.is_empty() {
                    content.push_str("\n\n");
                }
                content.push_str(&text);
            }
        }

        Ok(ParseResult {
            content,
            ..Default::default()
        })
    }

    fn name(&self) -> &'static str {
        "PdfParser"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_pdf_only() {
        let p = PdfParser::default();
        assert_eq!(p.extensions(), &["pdf"]);
    }
}
