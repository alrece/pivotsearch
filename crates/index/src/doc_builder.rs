//! Document assembly + uid algorithm.
//!
//! Per the openspec core-index-schema spec:
//! - uid = `file://{canonical_path}`
//! - the content field appends title/author/filename (both with and without extension), concatenated via multivalued add_text
//! - a missing title falls back to the filename without its extension
//! - upsert = delete_term(uid) + add_document

use crate::schema::SchemaFields;
use pivotsearch_contracts::{ParseResult, Uid};
use std::path::Path;
use tantivy::TantivyDocument;

/// Compute the uid: `file://{canonical_path}`.
///
/// Canonicalizes the given path (resolving symlinks and relative paths).
/// If canonicalize fails (the file was already deleted), falls back to the string form of the input path.
pub fn compute_uid(path: &Path) -> Uid {
    match path.canonicalize() {
        Ok(canon) => format!("file://{}", canon.display()),
        // The file may already have been deleted (handling a remove event); fall back to the original path string
        Err(_) => format!("file://{}", path.display()),
    }
}

/// Reverse-derive the path from a uid (`file://{path}` → `{path}`).
pub fn extract_path(uid: &str) -> Option<&str> {
    uid.strip_prefix("file://")
}

/// Assemble a Tantivy Document.
///
/// content field strategy (reproduces the design of classic desktop search tools, clean-room):
/// the text actually indexed in content = {body} + title + authors... + filename(with extension) + filename(without extension).
/// Multiple add_text calls into the same field are treated by Tantivy as a multivalued token stream, equivalent to concatenation.
/// Both filename variants are appended because jieba does not split at the dot; when searching for report.docx, the extension-less "report" should also match.
pub fn build_document(
    fields: &SchemaFields,
    path: &Path,
    parse_result: &ParseResult,
    uid: &str,
    index_id: &str,
) -> TantivyDocument {
    let mut doc = TantivyDocument::new();

    // —— Display fields (stored) ——
    doc.add_text(fields.uid, uid);

    // title: falls back to the filename without its extension if missing
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

    // type = extension (lowercase, no dot)
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    doc.add_text(fields.r#type, &ext);

    // parser name
    doc.add_text(fields.parser, parse_result.parser_name);

    // author (multivalued)
    for author in &parse_result.authors {
        doc.add_text(fields.author, author);
    }

    // Numeric fields
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

    // —— snippet_text: first 500 chars of content, stored, for SnippetGenerator highlighting ——
    let snippet_source: String = parse_result.content.chars().take(500).collect();
    doc.add_text(fields.snippet_text, &snippet_source);

    // —— content field (multivalued concatenation) ——
    // 1. body
    doc.add_text(fields.content, &parse_result.content);

    // 2. title (added to content again so title terms are also searchable via the body)
    doc.add_text(fields.content, &title);

    // 3. authors
    for author in &parse_result.authors {
        doc.add_text(fields.content, author);
    }

    // 4. misc metadata
    for meta in &parse_result.misc_metadata {
        doc.add_text(fields.content, meta);
    }

    // 5. filename (with extension)
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    doc.add_text(fields.content, file_name);

    // 6. filename (without extension) — jieba does not split at the dot; ensures it is searchable without the extension
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
        // After canonicalize, /tmp may become /private/tmp (macOS), so only check the prefix
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
        // Build a temporary index from the schema to read field values
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

        // Verify the fields were written
        let uid_values: Vec<_> = doc.get_all(fields.uid).collect();
        assert_eq!(uid_values.len(), 1);

        let author_values: Vec<_> = doc.get_all(fields.author).collect();
        assert_eq!(author_values.len(), 1);

        // content is multivalued: body + title + author + filename(with ext) + filename(without ext) = 5
        let content_values: Vec<_> = doc.get_all(fields.content).collect();
        assert!(
            content_values.len() >= 5,
            "content 应含正文+title+author+文件名两版本，实际 {} 个",
            content_values.len()
        );

        // Cleanup
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

        // snippet_text should be present in the doc and have a value
        let vals: Vec<_> = doc.get_all(fields.snippet_text).collect();
        assert!(!vals.is_empty(), "snippet_text 应有值");
        let text = vals[0].as_str();
        assert!(text.is_some(), "snippet_text 应能 as_str");
        assert!(text.unwrap().contains("营收"), "snippet_text 应含营收");
    }
}
