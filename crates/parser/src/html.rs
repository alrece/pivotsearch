//! HTML parser (scraper). Extracts body text, stripping script/style/nav.

use chardetng::{Iso2022JpDetection, Utf8Detection};
use pivotsearch_contracts::{ParseResult, Parser, PivotsearchError, Result};
use scraper::{ElementRef, Html, Selector};
use std::path::Path;

/// HTML parser.
#[derive(Default)]
pub struct HtmlParser;

impl Parser for HtmlParser {
    fn extensions(&self) -> &[&str] {
        &["html", "htm", "xhtml", "shtml"]
    }

    fn mimes(&self) -> &[&str] {
        &["text/html", "application/xhtml+xml"]
    }

    fn parse(&self, path: &Path) -> Result<ParseResult> {
        let bytes = std::fs::read(path).map_err(|e| PivotsearchError::FsIo {
            path: path.display().to_string(),
            source: e,
        })?;

        // Encoding detection
        let mut detector = chardetng::EncodingDetector::new(Iso2022JpDetection::Allow);
        detector.feed(&bytes, true);
        let encoding = detector.guess(None, Utf8Detection::Allow);
        let (html_str, _, _) = encoding.decode(&bytes);

        let document = Html::parse_document(&html_str);

        // title
        let title = document
            .select(&Selector::parse("title").unwrap())
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .filter(|t| !t.is_empty());

        // author
        let author = document
            .select(&Selector::parse(r#"meta[name="author"]"#).unwrap())
            .next()
            .and_then(|el| el.value().attr("content"))
            .map(|s| s.to_string());

        // Body: strip script/style/nav/header/footer/aside, traverse via ElementRef
        let body_sel = Selector::parse("body").unwrap();
        let skip_sel = Selector::parse("script, style, nav, header, footer, aside, noscript").unwrap();

        let mut content = String::new();
        if let Some(body) = document.select(&body_sel).next() {
            // Collect ids of all elements to skip (using ElementRef matches)
            // Simplification: traverse all elements under body, skip those matching skip_sel, take their direct text nodes
            collect_text(&body, &skip_sel, &mut content);
        }

        Ok(ParseResult {
            content,
            title,
            authors: author.into_iter().collect(),
            ..Default::default()
        })
    }

    fn name(&self) -> &'static str {
        "HtmlParser"
    }
}

/// Recursively collects an element's text, skipping elements that match skip_sel.
fn collect_text(element: &ElementRef, skip_sel: &Selector, out: &mut String) {
    for child_node in element.children() {
        // Try to wrap as ElementRef
        if let Some(child_el) = ElementRef::wrap(child_node) {
            // Skip elements matching skip_sel
            if skip_sel.matches(&child_el) {
                continue;
            }
            // Recurse
            collect_text(&child_el, skip_sel, out);
        } else if let Some(text) = child_node.value().as_text() {
            let t = text.trim();
            if !t.is_empty() {
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push(' ');
                }
                out.push_str(t);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_html_basic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("page.html");
        std::fs::write(
            &path,
            r#"<html><head><title>测试页面</title><meta name="author" content="张三"></head>
            <body><nav>导航</nav><p>正文内容</p><script>ignore</script></body></html>"#,
        ).unwrap();
        let result = HtmlParser.parse(&path).unwrap();
        assert_eq!(result.title.as_deref(), Some("测试页面"));
        assert_eq!(result.authors, vec!["张三"]);
        assert!(result.content.contains("正文内容"));
        assert!(!result.content.contains("ignore"));
        assert!(!result.content.contains("导航"));
    }
}
