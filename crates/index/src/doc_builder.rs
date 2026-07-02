//! Document 组装 + uid 算法。
//!
//! 依据 openspec core-index-schema spec：
//! - uid = `file://{canonical_path}`
//! - content 字段追加 title/author/文件名（带/去扩展名两版本），多值 add_text 拼接
//! - title 缺失退化为去扩展名文件名
//! - upsert = delete_term(uid) + add_document

use crate::schema::SchemaFields;
use pivotsearch_contracts::{ParseResult, Uid};
use std::path::Path;
use tantivy::TantivyDocument;

/// 计算 uid：`file://{canonical_path}`。
///
/// 对传入路径做规范化（解析符号链接和相对路径）。
/// 若 canonicalize 失败（文件已删除），退化为传入的字符串形式。
pub fn compute_uid(path: &Path) -> Uid {
    match path.canonicalize() {
        Ok(canon) => format!("file://{}", canon.display()),
        // 文件可能已被删除（监听 remove 事件场景），用原路径字符串兜底
        Err(_) => format!("file://{}", path.display()),
    }
}

/// 从 uid 反推路径（`file://{path}` → `{path}`）。
pub fn extract_path(uid: &str) -> Option<&str> {
    uid.strip_prefix("file://")
}

/// 组装一个 Tantivy Document。
///
/// content 字段策略（复刻经典桌面搜索工具的设计，净室）：
/// content 实际索引文本 = {正文} + title + authors... + 文件名(带扩展名) + 文件名(去扩展名)。
/// 多次 add_text 到同字段，Tantivy 会作为多值 token 流处理，等效拼接。
/// 追加文件名两版本是因为 jieba 不在点处切分，搜 report.docx 时不带扩展名的 "report" 也要命中。
pub fn build_document(
    fields: &SchemaFields,
    path: &Path,
    parse_result: &ParseResult,
    uid: &str,
    index_id: &str,
) -> TantivyDocument {
    let mut doc = TantivyDocument::new();

    // —— 展示字段（stored）——
    doc.add_text(fields.uid, uid);

    // title：缺失则退化为去扩展名文件名
    let title = parse_result
        .title
        .clone()
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string()
        });
    doc.add_text(fields.title, &title);

    // type = 扩展名（小写无点）
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    doc.add_text(fields.r#type, &ext);

    // parser 名
    doc.add_text(fields.parser, parse_result.parser_name);

    // author 多值
    for author in &parse_result.authors {
        doc.add_text(fields.author, author);
    }

    // 数值字段
    let size = path.metadata().map(|m| m.len() as i64).unwrap_or(0);
    doc.add_i64(fields.size, size);
    let mtime = path
        .metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    doc.add_i64(fields.last_modified, mtime);

    doc.add_text(fields.index_id, index_id);

    // —— snippet_text：content 前 500 字符，stored，供 SnippetGenerator 高亮 ——
    let snippet_source: String = parse_result.content.chars().take(500).collect();
    doc.add_text(fields.snippet_text, &snippet_source);

    // —— content 字段（多值拼接）——
    // 1. 正文
    doc.add_text(fields.content, &parse_result.content);

    // 2. title（再次加入 content，让标题词也能被正文搜到）
    doc.add_text(fields.content, &title);

    // 3. authors
    for author in &parse_result.authors {
        doc.add_text(fields.content, author);
    }

    // 4. misc metadata
    for meta in &parse_result.misc_metadata {
        doc.add_text(fields.content, meta);
    }

    // 5. 文件名（带扩展名）
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    doc.add_text(fields.content, file_name);

    // 6. 文件名（去扩展名）—— jieba 不在点处切分，确保不带扩展名也能搜到
    if !ext.is_empty() {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if !stem.is_empty() {
            doc.add_text(fields.content, stem);
        }
    }

    doc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::build_schema;

    #[test]
    fn uid_format() {
        let path = Path::new("/tmp/test.md");
        let uid = compute_uid(path);
        assert!(uid.starts_with("file://"));
        // canonicalize 后 /tmp 可能是 /private/tmp（macOS），所以只检查前缀
        assert!(uid.contains("test.md"));
    }

    #[test]
    fn extract_path_roundtrip() {
        let uid = "file:///home/foo/readme.md";
        assert_eq!(extract_path(uid), Some("/home/foo/readme.md"));
        assert_eq!(extract_path("not-a-uid"), None);
    }

    #[test]
    fn build_document_content_includes_metadata_and_filename() {
        let (schema, fields, _) = build_schema();
        // 用 schema 构造一个临时 index 来读取字段值
        let index = tantivy::Index::create_in_ram(schema.clone());

        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/sample.txt");
        std::fs::create_dir_all(path.parent().unwrap()).ok();
        std::fs::write(&path, "hello").ok();

        let parse_result = ParseResult {
            content: "正文内容".to_string(),
            title: Some("季度报告".to_string()),
            authors: vec!["张三".to_string()],
            misc_metadata: vec![],
            parser_name: "TestParser",
        };
        let uid = compute_uid(&path);

        let doc = build_document(&fields, &path, &parse_result, &uid, "idx-1");

        // 验证字段都写入了
        let uid_values: Vec<_> = doc.get_all(fields.uid).collect();
        assert_eq!(uid_values.len(), 1);

        let author_values: Vec<_> = doc.get_all(fields.author).collect();
        assert_eq!(author_values.len(), 1);

        // content 多值：正文 + title + author + 文件名(带ext) + 文件名(去ext) = 5
        let content_values: Vec<_> = doc.get_all(fields.content).collect();
        assert!(
            content_values.len() >= 5,
            "content 应含正文+title+author+文件名两版本，实际 {} 个",
            content_values.len()
        );

        // 清理
        std::fs::remove_file(&path).ok();
        drop(index);
    }
}

#[cfg(test)]
mod snippet_tests {
    use super::*;
    use crate::schema::build_schema;
    use tantivy::schema::Value;

    #[test]
    fn snippet_text_added_to_doc() {
        let (_schema, fields, _) = build_schema();
        let path = std::path::Path::new("test.md");
        let parse_result = ParseResult {
            content: "营收增长报告正文".to_string(),
            title: Some("标题".to_string()),
            ..Default::default()
        };
        let doc = build_document(&fields, path, &parse_result, "file://test.md", "idx");

        // snippet_text 应在 doc 中且有值
        let vals: Vec<_> = doc.get_all(fields.snippet_text).collect();
        assert!(!vals.is_empty(), "snippet_text 应有值");
        let text = vals[0].as_str();
        assert!(text.is_some(), "snippet_text 应能 as_str");
        assert!(text.unwrap().contains("营收"), "snippet_text 应含营收");
    }
}
