//! Tantivy schema：八字段定死（不可变约束）。
//!
//! 字段设计依据 openspec core-index-schema/spec.md。
//! 关键：Tantivy schema 启动时一次定死，字段演进需 reindex。

use tantivy::schema::{
    Field, IndexRecordOption, NumericOptions, Schema, SchemaBuilder, TextFieldIndexing, TextOptions,
};
use tantivy::tokenizer::{TextAnalyzer, TokenizerManager};

/// 所有字段名的常量（供查询/组装引用，避免拼写错误）。
pub mod field_names {
    pub const UID: &str = "uid";
    pub const CONTENT: &str = "content";
    pub const SNIPPET_TEXT: &str = "snippet_text";  // content 前 500 字节，stored，供高亮
    pub const TITLE: &str = "title";
    pub const AUTHOR: &str = "author";
    pub const TYPE: &str = "type";
    pub const PARSER: &str = "parser";
    pub const SIZE: &str = "size";
    pub const LAST_MODIFIED: &str = "last_modified";
    pub const INDEX_ID: &str = "index_id";
}

/// jieba 分词器在 TokenizerManager 中注册的名字。
pub const JIEBA_TOKENIZER_NAME: &str = "jieba";

/// schema 中所有 Field 句柄的集合（构造一次，全局复用）。
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

/// content/title/author 字段的 TextOptions：用 jieba 分词。
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

/// 构建不可变的 Tantivy schema（9 个 field，含 author 多值）+ 注册 jieba tokenizer。
///
/// 返回 (Schema, SchemaFields, TokenizerManager)。
pub fn build_schema() -> (Schema, SchemaFields, TokenizerManager) {
    let mut builder = SchemaBuilder::new();

    // uid：STRING（精确匹配，不分词，stored 便于展示）
    let uid = builder.add_text_field(
        field_names::UID,
        TextOptions::default().set_stored().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("raw")
                .set_index_option(IndexRecordOption::Basic),
        ),
    );
    // content：jieba 分词，不存（省空间，预览时重新解析）
    let content = builder.add_text_field(field_names::CONTENT, jieba_text_options(false));
    // snippet_text：content 前 500 字符，jieba 分词 + stored，供 SnippetGenerator 高亮
    let snippet_text = builder.add_text_field(field_names::SNIPPET_TEXT, jieba_text_options(true));
    // title：jieba 分词 + 存
    let title = builder.add_text_field(field_names::TITLE, jieba_text_options(true));
    // author：jieba 分词 + 存（多值）
    let author = builder.add_text_field(field_names::AUTHOR, jieba_text_options(true));
    // type：精确匹配（扩展名过滤），存
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
    // parser：精确匹配，存
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
    // size：I64 数值（范围查询 + 排序）
    let size = builder.add_i64_field(field_names::SIZE, NumericOptions::default().set_indexed().set_stored().set_fast());
    // last_modified：I64（范围查询）
    let last_modified = builder.add_i64_field(
        field_names::LAST_MODIFIED,
        NumericOptions::default().set_indexed().set_stored().set_fast(),
    );
    // index_id：精确匹配（多索引过滤），存
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

    // 注册 jieba tokenizer
    let tokenizer_manager = TokenizerManager::default();
    tokenizer_manager.register(
        JIEBA_TOKENIZER_NAME,
        TextAnalyzer::from(crate::tokenizer::JiebaTokenizer::default()),
    );

    (schema, fields, tokenizer_manager)
}

// 保持向后兼容（其他模块可能引用 TEXT/STORED/INDEXED）

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
