//! PPTX parser (OOXML).
//!
//! A PPTX is essentially a zip package containing ppt/slides/slideN.xml (one XML per slide).
//! The <a:t> tags in the XML hold the text content. We unpack it with the zip crate and extract the text from all slides.

use pivotsearch_contracts::{ParseResult, Parser, PivotsearchError, Result};
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

/// PPTX parser.
pub struct PptxParser;

impl Parser for PptxParser {
    fn extensions(&self) -> &[&str] {
        &["pptx"]
    }

    fn mimes(&self) -> &[&str] {
        &["application/vnd.openxmlformats-officedocument.presentationml.presentation"]
    }

    fn parse(&self, path: &Path) -> Result<ParseResult> {
        let file = std::fs::File::open(path).map_err(|e| PivotsearchError::FsIo {
            path: path.display().to_string(),
            source: e,
        })?;
        let mut archive = ZipArchive::new(file).map_err(|e| PivotsearchError::ParseFailed {
            path: path.display().to_string(),
            reason: format!("pptx zip: {e}"),
        })?;

        // Collect all slide file names and sort them (slide1, slide2, ...)
        let mut slide_names: Vec<String> = Vec::new();
        for i in 0..archive.len() {
            if let Ok(entry) = archive.by_index(i) {
                let name = entry.name().to_string();
                if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
                    slide_names.push(name);
                }
            }
        }
        slide_names.sort_by(|a, b| {
            // Sort numerically (slide1 < slide2 < slide10)
            let na = a.trim_start_matches("ppt/slides/slide").trim_end_matches(".xml");
            let nb = b.trim_start_matches("ppt/slides/slide").trim_end_matches(".xml");
            na.parse::<u32>().unwrap_or(0).cmp(&nb.parse::<u32>().unwrap_or(0))
        });

        let mut content = String::new();
        for slide_name in &slide_names {
            if let Ok(mut entry) = archive.by_name(slide_name) {
                let mut xml = String::new();
                if entry.read_to_string(&mut xml).is_ok() {
                    let texts = extract_text_runs(&xml);
                    if !texts.is_empty() {
                        if !content.is_empty() {
                            content.push_str("\n\n");
                        }
                        content.push_str(&texts.join(" "));
                    }
                }
            }
        }

        // Extract the title from core.xml (optional)
        let title = if let Ok(mut entry) = archive.by_name("docProps/core.xml") {
            let mut xml = String::new();
            if entry.read_to_string(&mut xml).is_ok() {
                extract_dc_field(&xml, "dc:title")
            } else {
                None
            }
        } else {
            None
        };

        Ok(ParseResult {
            content,
            title,
            ..Default::default()
        })
    }

    fn name(&self) -> &'static str {
        "PptxParser"
    }
}

/// Extracts all <a:t>...</a:t> text (OOXML text runs) from PPTX slide XML.
fn extract_text_runs(xml: &str) -> Vec<String> {
    let mut texts = Vec::new();
    let tag = "<a:t>";
    let tag_end = "</a:t>";
    let mut search_from = 0;
    while let Some(start) = xml[search_from..].find(tag) {
        let abs_start = search_from + start + tag.len();
        if let Some(end) = xml[abs_start..].find(tag_end) {
            let text = &xml[abs_start..abs_start + end];
            let decoded = text
                .replace("&amp;", "&")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"");
            if !decoded.trim().is_empty() {
                texts.push(decoded);
            }
            search_from = abs_start + end + tag_end.len();
        } else {
            break;
        }
    }
    texts
}

/// Extracts a Dublin Core field (dc:title, etc.) from core.xml.
fn extract_dc_field(xml: &str, field: &str) -> Option<String> {
    let open = format!("<{field}>");
    let close = format!("</{field}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)?;
    let val = xml[start..start + end].trim();
    if val.is_empty() {
        None
    } else {
        Some(val.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pptx_extensions() {
        assert_eq!(PptxParser.extensions(), &["pptx"]);
        // Legacy format not supported
        assert!(!PptxParser.extensions().contains(&"ppt"));
    }

    #[test]
    fn extract_text_runs_works() {
        let xml = r#"<xml><a:t>标题</a:t><a:r><a:t>内容</a:t></a:r></xml>"#;
        let texts = extract_text_runs(xml);
        assert_eq!(texts, vec!["标题", "内容"]);
    }
}
