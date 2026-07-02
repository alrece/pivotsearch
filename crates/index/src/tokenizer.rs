//! jieba 中文分词器（自定义 Tantivy Tokenizer）。
//!
//! 不依赖 tantivy-jieba crate（版本滞后跟踪 tantivy 0.20，与 0.24 不兼容），
//! 而是参考其实现自写，版本完全可控。
//!
//! 设计：对中英混排文本，jieba 分词处理好中文部分，英文标识符整体保留。

use jieba_rs::Jieba;
use tantivy::tokenizer::{Token, TokenStream, Tokenizer};

/// jieba 中文分词器。
///
/// 必须实现 Clone（tantivy Tokenizer trait 要求）。
/// Jieba 内部用 Arc 共享词库数据，clone 廉价。
#[derive(Clone)]
pub struct JiebaTokenizer {
    /// 用 Arc 共享，clone 不复制词库。
    jieba: std::sync::Arc<Jieba>,
}

impl Default for JiebaTokenizer {
    fn default() -> Self {
        Self {
            jieba: std::sync::Arc::new(Jieba::new()),
        }
    }
}

impl Tokenizer for JiebaTokenizer {
    type TokenStream<'a> = JiebaTokenStream<'a>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        // jieba 分词，HMM 模式提升未登录词召回
        let segmented: Vec<&str> = self.jieba.cut(text, true);

        // 计算每个 token 的字节偏移（jieba 的 cut 返回引用切片，按原文本顺序）
        let mut tokens = Vec::with_capacity(segmented.len());
        let mut byte_offset = 0usize;
        for (position, word) in segmented.into_iter().enumerate() {
            let word_trimmed = word.trim();
            if word_trimmed.is_empty() {
                // 仍要推进 offset（空白的长度）
                byte_offset += word.len();
                continue;
            }
            // 找到 word 在 text 中从 byte_offset 开始的位置
            let offset_from = byte_offset;
            let offset_to = byte_offset + word.len();
            byte_offset = offset_to;

            tokens.push(Token {
                offset_from,
                offset_to,
                position,
                text: word_trimmed.to_lowercase(),
                position_length: 1,
            });
        }

        JiebaTokenStream {
            tokens,
            index: 0,
            _phantom: std::marker::PhantomData,
        }
    }
}

pub struct JiebaTokenStream<'a> {
    tokens: Vec<Token>,
    index: usize,
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> TokenStream for JiebaTokenStream<'a> {
    fn advance(&mut self) -> bool {
        if self.index < self.tokens.len() {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn token(&self) -> &Token {
        &self.tokens[self.index - 1]
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.tokens[self.index - 1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jieba_chinese_segmentation() {
        let mut tokenizer = JiebaTokenizer::default();
        let mut stream = tokenizer.token_stream("我爱自然语言处理");
        let mut words = Vec::new();
        while stream.advance() {
            words.push(stream.token().text.clone());
        }
        assert!(words.len() >= 3, "应切出至少 3 个词，实际: {:?}", words);
        assert!(
            words.iter().any(|w| w.contains("自然") || w.contains("语言")),
            "应包含'自然'或'语言'，实际: {:?}",
            words
        );
    }

    #[test]
    fn jieba_mixed_cn_en() {
        let mut tokenizer = JiebaTokenizer::default();
        let mut stream = tokenizer.token_stream("使用 React 开发前端");
        let mut words = Vec::new();
        while stream.advance() {
            words.push(stream.token().text.clone());
        }
        assert!(
            words.iter().any(|w| w == "react"),
            "应包含 'react'，实际: {:?}",
            words
        );
    }
}
