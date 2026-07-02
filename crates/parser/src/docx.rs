//! DOCX 解析器（docx-rs）。
//!
//! docx-rs 主要面向生成，读取需递归遍历 Document→Paragraph→Run→Text。

use docx_rs::{
    read_docx, InsertChild, ParagraphChild, RunChild, DocumentChild,
};
use pivotsearch_contracts::{ParseResult, Parser, PivotsearchError, Result};
use std::path::Path;

/// DOCX 解析器。
pub struct DocxParser;

impl Parser for DocxParser {
    fn extensions(&self) -> &[&str] {
        &["docx"]
    }

    fn mimes(&self) -> &[&str] {
        &["application/vnd.openxmlformats-officedocument.wordprocessingml.document"]
    }

    fn parse(&self, path: &Path) -> Result<ParseResult> {
        let bytes = std::fs::read(path).map_err(|e| PivotsearchError::FsIo {
            path: path.display().to_string(),
            source: e,
        })?;

        let docx = read_docx(&bytes).map_err(|e| PivotsearchError::ParseFailed {
            path: path.display().to_string(),
            reason: format!("{e:?}"),
        })?;

        let mut content = String::new();
        for child in &docx.document.children {
            if let DocumentChild::Paragraph(p) = child {
                let para_text = extract_paragraph_text(p);
                if !para_text.is_empty() {
                    if !content.is_empty() {
                        content.push('\n');
                    }
                    content.push_str(&para_text);
                }
            }
        }

        Ok(ParseResult {
            content,
            ..Default::default()
        })
    }

    fn name(&self) -> &'static str {
        "DocxParser"
    }
}

/// 从 Paragraph 提取所有 Run/Insert/Hyperlink 里的 Text。
fn extract_paragraph_text(paragraph: &docx_rs::Paragraph) -> String {
    let mut text = String::new();
    for child in &paragraph.children {
        match child {
            ParagraphChild::Run(run) => {
                collect_run_text(&run.children, &mut text);
            }
            ParagraphChild::Insert(insert) => {
                for insert_child in &insert.children {
                    if let InsertChild::Run(run) = insert_child {
                        collect_run_text(&run.children, &mut text);
                    }
                }
            }
            ParagraphChild::Hyperlink(hyperlink) => {
                // Hyperlink.children 是 Vec<ParagraphChild>，递归提取
                for sub_child in &hyperlink.children {
                    if let ParagraphChild::Run(run) = sub_child {
                        collect_run_text(&run.children, &mut text);
                    }
                }
            }
            _ => {}
        }
    }
    text
}

/// 从 RunChild 列表提取 Text。
fn collect_run_text(run_children: &[RunChild], out: &mut String) {
    for rc in run_children {
        if let RunChild::Text(t) = rc {
            out.push_str(&t.text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_docx_not_doc() {
        let exts = DocxParser.extensions();
        assert!(!exts.contains(&"doc"));
        assert!(exts.contains(&"docx"));
    }
}
