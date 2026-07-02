//! ePub 解析器。
//!
//! ePub 本质是 zip 包，内含 XHTML 页面。用 zip crate 解开，
//! 提取所有 .xhtml/.html 文件的正文文本（去标签）。

use pivotsearch_contracts::{ParseResult, Parser, PivotsearchError, Result};
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

/// ePub 解析器。
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

            // 提取 XHTML/HTML 正文
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
                    // 从第一个 HTML 的 <title> 提取标题
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

/// 简易 HTML 标签去除（提取纯文本）。
///
/// 先移除 script/style 块（含内容），再去掉所有标签。
fn strip_html_tags(html: &str) -> String {
    // 第一步：移除 script/style 块（含内容）
    let cleaned = remove_blocks(html, "<script", "</script>");
    let cleaned = remove_blocks(&cleaned, "<style", "</style>");

    // 第二步：去掉所有 <...> 标签
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

/// 移除所有 open...close 块（大小写不敏感匹配开标签）。
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
            // 无闭合标签，保留剩余
            cursor = input.len();
            break;
        }
    }
    result.push_str(&input[cursor..]);
    result
}

/// 提取 <title> 内容。
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

/// 简易 HTML 实体解码。
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
