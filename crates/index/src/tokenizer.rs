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
/// 含停用词过滤（的、了、是等高频虚词），提升搜索精度。
#[derive(Clone)]
pub struct JiebaTokenizer {
    /// 用 Arc 共享，clone 不复制词库。
    jieba: std::sync::Arc<Jieba>,
    /// 停用词集合（Arc 共享，clone 廉价）。
    stop_words: std::sync::Arc<std::collections::HashSet<&'static str>>,
}

/// 中文常见停用词（虚词/代词/量词等高频低信息量词）。
const STOP_WORDS: &[&str] = &[
    "的", "了", "是", "在", "和", "也", "都", "就", "你", "我", "他", "她", "它",
    "这", "那", "有", "为", "以", "及", "或", "与", "但", "而", "所", "被", "把",
    "给", "向", "从", "到", "对", "于", "由", "按", "根据", "通过", "一个", "一种",
    "可以", "这个", "那个", "这些", "那些", "什么", "怎么", "哪里", "为什么",
    "着", "过", "吧", "呢", "啊", "吗", "嗯", "哦", "的话",
];

impl Default for JiebaTokenizer {
    fn default() -> Self {
        Self {
            jieba: std::sync::Arc::new(Jieba::new()),
            stop_words: std::sync::Arc::new(STOP_WORDS.iter().copied().collect()),
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
                byte_offset += word.len();
                continue;
            }
            // 停用词过滤（的、了、是等高频虚词）
            if self.stop_words.contains(word_trimmed) {
                byte_offset += word.len();
                continue;
            }
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

    #[test]
    fn jieba_filters_stop_words() {
        let mut tokenizer = JiebaTokenizer::default();
        let mut stream = tokenizer.token_stream("我的公司是一个好公司");
        let mut words = Vec::new();
        while stream.advance() {
            words.push(stream.token().text.clone());
        }
        // 停用词"的""是""一个"应被过滤
        assert!(!words.contains(&"的".to_string()), "的应被过滤");
        assert!(!words.contains(&"是".to_string()), "是应被过滤");
        assert!(!words.contains(&"一个".to_string()), "一个应被过滤");
        // 实义词保留
        assert!(words.contains(&"公司".to_string()), "公司应保留");
    }
}
