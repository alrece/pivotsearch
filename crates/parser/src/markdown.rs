//! Markdown parser (pulldown-cmark 0.13).

use pivotsearch_contracts::{ParseResult, Parser, PivotsearchError, Result};
use pulldown_cmark::{Event, Options, Parser as CmarkParser, Tag, TagEnd};
use std::path::Path;

/// Markdown parser. Extracts plain text (stripping markup symbols).
pub struct MarkdownParser;

impl Parser for MarkdownParser {
    fn extensions(&self) -> &[&str] {
        &["md", "markdown"]
    }

    fn mimes(&self) -> &[&str] {
        &["text/markdown"]
    }

    fn parse(&self, path: &Path) -> Result<ParseResult> {
        let markdown = std::fs::read_to_string(path).map_err(|e| PivotsearchError::FsIo {
            path: path.display().to_string(),
            source: e,
        })?;

        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_STRIKETHROUGH);

        let parser = CmarkParser::new_ext(&markdown, options);

        let mut content = String::new();
        let mut title: Option<String> = None;
        let mut in_heading: Option<pulldown_cmark::HeadingLevel> = None;
        let mut heading_text = String::new();

        for event in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    in_heading = Some(level);
                    heading_text.clear();
                }
                Event::End(TagEnd::Heading(_level)) => {
                    let trimmed = heading_text.trim().to_string();
                    if !trimmed.is_empty() && title.is_none() {
                        title = Some(trimmed);
                    }
                    if !content.is_empty() {
                        content.push('\n');
                    }
                    content.push_str(&heading_text);
                    in_heading = None;
                }
                Event::Start(Tag::Paragraph) | Event::End(TagEnd::Paragraph) => {}
                Event::Text(text) => {
                    if let Some(level) = in_heading {
                        if level == pulldown_cmark::HeadingLevel::H1 {
                            heading_text.push_str(&text);
                        }
                    }
                    content.push_str(&text);
                }
                Event::Code(code) => {
                    content.push_str(&code);
                    content.push(' ');
                }
                Event::SoftBreak | Event::HardBreak => {
                    content.push('\n');
                }
                _ => {}
            }
        }

        Ok(ParseResult {
            content,
            title,
            ..Default::default()
        })
    }

    fn name(&self) -> &'static str {
        "MarkdownParser"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_markdown_with_title() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doc.md");
        std::fs::write(&path, "# 季度报告\n\n本季度营收增长20%。").unwrap();
        let result = MarkdownParser.parse(&path).unwrap();
        assert_eq!(result.title.as_deref(), Some("季度报告"));
        assert!(result.content.contains("营收增长"));
    }
}
