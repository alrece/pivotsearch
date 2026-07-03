//! Tantivy schema: eight fields fixed at startup (immutable constraint).
//!
//! Field design based on openspec core-index-schema/spec.md.
//! Key point: the Tantivy schema is fixed once at startup; field evolution requires a reindex.

use tantivy::schema::{
    Field, IndexRecordOption, NumericOptions, Schema, SchemaBuilder, TextFieldIndexing, TextOptions,
};
use tantivy::tokenizer::{TextAnalyzer, TokenizerManager};

/// Constants for all field names (referenced by queries/assembly to avoid spelling mistakes).
pub mod field_names {
    pub const UID: &str = "uid";
    pub const CONTENT: &str = "content";
    pub const SNIPPET_TEXT: &str = "snippet_text";  // first 500 chars of content, stored, for highlighting
    pub const TITLE: &str = "title";
    pub const AUTHOR: &str = "author";
    pub const TYPE: &str = "type";
    pub const PARSER: &str = "parser";
    pub const SIZE: &str = "size";
    pub const LAST_MODIFIED: &str = "last_modified";
    pub const INDEX_ID: &str = "index_id";
}

/// Name under which the jieba tokenizer is registered in the TokenizerManager.
pub const JIEBA_TOKENIZER_NAME: &str = "jieba";

/// Collection of all Field handles in the schema (built once, reused globally).
#[derive(Clone)]
pub struct SchemaFields {
    pub uid: Field,
    pub content: Field,
    pub snippet_text: Field,
    pub title: Field,
    pub author: Field,
    pub r#type: Field,
    pub parser: Field,
    pub size: Field,
    pub last_modified: Field,
    pub index_id: Field,
}

/// TextOptions for the content/title/author fields: use jieba tokenization.
fn jieba_text_options(stored: bool) -> TextOptions {
    let indexing = TextFieldIndexing::default()
        .set_tokenizer(JIEBA_TOKENIZER_NAME)
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let mut opts = TextOptions::default().set_indexing_options(indexing);
    if stored {
        opts = opts.set_stored();
    }
    opts
}

/// Build the immutable Tantivy schema (9 fields, including multivalued author) + register the jieba tokenizer.
///
/// Returns (Schema, SchemaFields, TokenizerManager).
pub fn build_schema() -> (Schema, SchemaFields, TokenizerManager) {
    let mut builder = SchemaBuilder::new();

    // uid: STRING (exact match, no tokenization, stored for display)
    let uid = builder.add_text_field(
        field_names::UID,
        TextOptions::default().set_stored().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("raw")
                .set_index_option(IndexRecordOption::Basic),
        ),
    );
    // content: jieba tokenization, not stored (saves space; re-parsed at preview time)
    let content = builder.add_text_field(field_names::CONTENT, jieba_text_options(false));
    // snippet_text: first 500 chars of content, jieba tokenization + stored, for SnippetGenerator highlighting
    let snippet_text = builder.add_text_field(field_names::SNIPPET_TEXT, jieba_text_options(true));
    // title: jieba tokenization + stored
    let title = builder.add_text_field(field_names::TITLE, jieba_text_options(true));
    // author: jieba tokenization + stored (multivalued)
    let author = builder.add_text_field(field_names::AUTHOR, jieba_text_options(true));
    // type: exact match (extension filtering), stored
    let r#type = builder.add_text_field(
        field_names::TYPE,
        TextOptions::default()
            .set_stored()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("raw")
                    .set_index_option(IndexRecordOption::Basic),
            ),
    );
    // parser: exact match, stored
    let parser = builder.add_text_field(
        field_names::PARSER,
        TextOptions::default()
            .set_stored()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("raw")
                    .set_index_option(IndexRecordOption::Basic),
            ),
    );
    // size: I64 numeric (range queries + sorting)
    let size = builder.add_i64_field(field_names::SIZE, NumericOptions::default().set_indexed().set_stored().set_fast());
    // last_modified: I64 (range queries)
    let last_modified = builder.add_i64_field(
        field_names::LAST_MODIFIED,
        NumericOptions::default().set_indexed().set_stored().set_fast(),
    );
    // index_id: exact match (multi-index filtering), stored
    let index_id = builder.add_text_field(
        field_names::INDEX_ID,
        TextOptions::default()
            .set_stored()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("raw")
                    .set_index_option(IndexRecordOption::Basic),
            ),
    );

    let schema = builder.build();
    let fields = SchemaFields {
        uid,
        content,
        snippet_text,
        title,
        author,
        r#type,
        parser,
        size,
        last_modified,
        index_id,
    };

    // Register the jieba tokenizer
    let tokenizer_manager = TokenizerManager::default();
    tokenizer_manager.register(
        JIEBA_TOKENIZER_NAME,
        TextAnalyzer::from(crate::tokenizer::JiebaTokenizer::default()),
    );

    (schema, fields, tokenizer_manager)
}

// Maintain backward compatibility (other modules may reference TEXT/STORED/INDEXED)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_has_nine_fields() {
        let (schema, _fields, _) = build_schema();
        let field_count = schema.fields().count();
        assert_eq!(field_count, 10, "schema 应有 9 个 field（含 author 多值）");
    }

    #[test]
    fn fields_are_consistent() {
        let (schema, fields, _) = build_schema();
        assert_eq!(schema.get_field_name(fields.uid), field_names::UID);
        assert_eq!(schema.get_field_name(fields.content), field_names::CONTENT);
        assert_eq!(schema.get_field_name(fields.title), field_names::TITLE);
        assert_eq!(schema.get_field_name(fields.r#type), field_names::TYPE);
        assert_eq!(schema.get_field_name(fields.index_id), field_names::INDEX_ID);
    }
}
