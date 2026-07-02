//! PDF 解析器（pdfium-render，动态链接 PDFium）。
//!
//! PDFium 是 Chromium 同款 PDF 引擎，对中文（CID 字体）支持成熟。
//! 运行时需要系统提供 PDFium 动态库。Phase 5 改为静态链接。

use pdfium_render::prelude::*;
use pivotsearch_contracts::{ParseResult, Parser, PivotsearchError, Result};
use std::path::Path;

/// PDF 解析器。
///
/// 持有可选的 Pdfium 实例（线程安全）。
/// 构造时尝试绑定系统 PDFium 库；缺失时 pdfium=None，parse 时报清晰错误。
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
    /// 带显式 Pdfium 实例构造（测试/静态链接用）。
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
