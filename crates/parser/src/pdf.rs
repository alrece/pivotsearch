//! PDF parser (pdfium-render, dynamically linked to PDFium).
//!
//! PDFium is the same PDF engine used by Chromium, with mature support for Chinese (CID fonts).
//! At runtime we bind the shared library from (in order): `PDFIUM_DYNAMIC_LIB_PATH`,
//! paths next to the executable / app bundle, the repo `native/pdfium/lib` tree, then the system.

use pdfium_render::prelude::*;
use pivotsearch_contracts::{ParseResult, Parser, PivotsearchError, Result};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// PDF parser.
///
/// Holds an optional Pdfium instance (thread-safe).
/// On construction it tries multiple library locations; if none bind, pdfium=None
/// and parse() returns a clear error.
pub struct PdfParser {
    pdfium: Option<&'static Pdfium>,
}

/// Process-wide Pdfium handle (pdfium-render only allows one bind per process).
static PDFIUM: OnceLock<Option<Pdfium>> = OnceLock::new();

fn bind_pdfium_once() -> Option<&'static Pdfium> {
    PDFIUM
        .get_or_init(|| try_bind_pdfium().map(Pdfium::new))
        .as_ref()
}

fn try_bind_pdfium() -> Option<Box<dyn PdfiumLibraryBindings>> {
    for path in pdfium_candidate_paths() {
        if path.is_file() {
            if let Ok(bindings) = Pdfium::bind_to_library(&path) {
                tracing::info!("PDFium bound from {}", path.display());
                return Some(bindings);
            }
        }
        // Also try directory form (pdfium-render joins the platform library name)
        if path.is_dir() {
            let lib = Pdfium::pdfium_platform_library_name_at_path(&path);
            if lib.is_file() {
                if let Ok(bindings) = Pdfium::bind_to_library(&lib) {
                    tracing::info!("PDFium bound from {}", lib.display());
                    return Some(bindings);
                }
            }
        }
    }

    match Pdfium::bind_to_system_library() {
        Ok(bindings) => {
            tracing::info!("PDFium bound from system library");
            Some(bindings)
        }
        Err(e) => {
            tracing::warn!("PDFium library not found: {e}");
            None
        }
    }
}

fn pdfium_candidate_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();

    if let Ok(p) = std::env::var("PDFIUM_DYNAMIC_LIB_PATH") {
        if !p.is_empty() {
            out.push(PathBuf::from(p));
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // CLI / sidecar next to binary
            out.push(dir.join(Pdfium::pdfium_platform_library_name()));
            out.push(dir.join("pdfium").join(Pdfium::pdfium_platform_library_name()));
            // macOS .app: Contents/MacOS → Contents/Resources
            if let Some(contents) = dir.parent() {
                let resources = contents.join("Resources");
                out.push(resources.join(Pdfium::pdfium_platform_library_name()));
                out.push(
                    resources
                        .join("pdfium")
                        .join(Pdfium::pdfium_platform_library_name()),
                );
                out.push(resources.join("lib").join(Pdfium::pdfium_platform_library_name()));
            }
        }
    }

    // Dev / workspace checkout: crates/parser → ../../native/pdfium/lib
    let manifest_lib = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../native/pdfium/lib")
        .join(Pdfium::pdfium_platform_library_name());
    out.push(manifest_lib);

    // cwd-relative (when running from repo root)
    out.push(PathBuf::from("native/pdfium/lib").join(Pdfium::pdfium_platform_library_name()));

    out
}

impl Default for PdfParser {
    fn default() -> Self {
        Self {
            pdfium: bind_pdfium_once(),
        }
    }
}

impl PdfParser {
    /// Constructs with an explicit Pdfium instance (for tests).
    ///
    /// Note: pdfium-render only allows one bind per process; prefer [`Default`] in production.
    pub fn with_pdfium(_pdfium: Pdfium) -> Self {
        Self::default()
    }

    /// Whether a PDFium library was successfully bound.
    pub fn is_available(&self) -> bool {
        self.pdfium.is_some()
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
        let pdfium = self.pdfium.ok_or_else(|| PivotsearchError::ParseFailed {
            path: path.display().to_string(),
            reason: "PDFium 动态库未找到，请运行 scripts/fetch-pdfium.sh 或设置 PDFIUM_DYNAMIC_LIB_PATH"
                .to_string(),
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

    #[test]
    fn candidate_paths_non_empty() {
        assert!(!pdfium_candidate_paths().is_empty());
    }
}
