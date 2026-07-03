//! ePub parser.
//!
//! An ePub is essentially a zip package containing XHTML pages. We unpack it with the zip crate
//! and extract the body text (tags stripped) from all .xhtml/.html files.

use pivotsearch_contracts::{ParseResult, Parser, PivotsearchError, Result};
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

/// ePub parser.
pub struct EpubParser;

impl Parser for EpubParser {
    fn extensions(&self) -> &[&str] {
        &["epub"]
    }

    fn mimes(&self) -> &[&str] {
        &["application/epub+zip"]
    }

    fn parse(&self, path: &Path) -> Result<ParseResult> {
        let file = std::fs::File::open(path).map_err(|e| PivotsearchError::FsIo {
            path: path.display().to_string(),
            source: e,
        })?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| PivotsearchError::ParseFailed {
                path: path.display().to_string(),
                reason: format!("epub zip: {e}"),
            })?;

        let mut content = String::new();
        let mut title = None;

        for i in 0..archive.len() {
            let mut entry = match archive.by_index(i) {
                Ok(e) => e,
                Err(_) => continue,
            };
            let name = entry.name().to_string();

            // Extract XHTML/HTML body text
            if name.ends_with(".xhtml") || name.ends_with(".html") || name.ends_with(".htm") {
                let mut html = String::new();
                if entry.read_to_string(&mut html).is_ok() {
                    let text = strip_html_tags(&html);
                    if !text.trim().is_empty() {
                        if !content.is_empty() {
                            content.push_str("\n\n");
                        }
                        content.push_str(&text);
                    }
                    // Extract the title from the first HTML's <title>
                    if title.is_none() {
                        title = extract_title(&html);
                    }
                }
            }
        }

        Ok(ParseResult {
            content,
            title,
            ..Default::default()
        })
    }

    fn name(&self) -> &'static str {
        "EpubParser"
    }
}

/// Minimal HTML tag stripper (extracts plain text).
///
/// First removes script/style blocks (including their content), then strips all tags.
fn strip_html_tags(html: &str) -> String {
    // Step 1: remove script/style blocks (including content)
    let cleaned = remove_blocks(html, "<script", "</script>");
    let cleaned = remove_blocks(&cleaned, "<style", "</style>");

    // Step 2: strip all <...> tags
    let mut text = String::with_capacity(cleaned.len() / 2);
    let mut in_tag = false;
    for ch in cleaned.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(ch),
            _ => {}
        }
    }
    html_decode(&text)
}

/// Removes all open...close blocks (case-insensitive matching of the opening tag).
fn remove_blocks(input: &str, open: &str, close: &str) -> String {
    let lower = input.to_lowercase();
    let mut result = String::with_capacity(input.len());
    let mut cursor = 0;
    let mut search = lower.as_str();
    let mut offset = 0;
    while let Some(rel_start) = search.find(open) {
        let abs_start = offset + rel_start;
        result.push_str(&input[cursor..abs_start]);
        if let Some(rel_end) = search[rel_start..].find(close) {
            let abs_end = abs_start + rel_end + close.len();
            cursor = abs_end;
            offset = abs_end;
            search = &lower[abs_end..];
        } else {
            // No closing tag; keep the remainder
            cursor = input.len();
            break;
        }
    }
    result.push_str(&input[cursor..]);
    result
}

/// Extracts the contents of <title>.
fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title>")? + 7;
    let end = lower.find("</title>")?;
    if start < end {
        let title = html[start..end].trim();
        if !title.is_empty() {
            return Some(html_decode(title));
        }
    }
    None
}

/// Minimal HTML entity decoder.
fn html_decode(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epub_extensions() {
        assert_eq!(EpubParser.extensions(), &["epub"]);
    }

    #[test]
    fn strip_html_works() {
        let html = "<p>Hello <b>World</b></p><script>ignore</script>";
        let text = strip_html_tags(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
        assert!(!text.contains("ignore"));
    }

    #[test]
    fn extract_title_works() {
        assert_eq!(
            extract_title("<html><head><title>我的书</title></head>"),
            Some("我的书".to_string())
        );
        assert_eq!(extract_title("<html><body>no title</body>"), None);
    }
}
