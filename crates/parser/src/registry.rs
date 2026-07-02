//! Parser 注册表 + 两级选择策略。
//!
//! 设计（复刻经典桌面搜索工具的解析注册表模式，净室重写）：
//! 1. mime 路径：infer 魔数检测 → 按匹配度排序 → 依次尝试直到成功（容错）
//! 2. 扩展名路径：精确匹配扩展名取第一个 parser
//! 3. 兜底：返回 UnsupportedFormat

use pivotsearch_contracts::{ParseResult, Parser, ParserRegistry, PivotsearchError, Result};
use std::path::Path;

/// Parser 注册表的默认实现。
pub struct ParserRegistryImpl {
    parsers: Vec<Box<dyn Parser>>,
}

impl ParserRegistryImpl {
    /// 用全部内置 parser 构造注册表。
    pub fn with_builtin_parsers() -> Self {
        let parsers: Vec<Box<dyn Parser>> = vec![
            Box::new(crate::text::TextParser),
            Box::new(crate::markdown::MarkdownParser),
            Box::new(crate::html::HtmlParser),
            Box::new(crate::docx::DocxParser),
            Box::new(crate::xlsx::SpreadsheetParser),
            Box::new(crate::epub::EpubParser),
            Box::new(crate::pptx::PptxParser),
        ];
        Self { parsers }
    }

    /// 在默认基础上启用 PDF 解析。
    pub fn with_pdf(mut self) -> Self {
        self.parsers.push(Box::new(crate::pdf::PdfParser::default()));
        self
    }

    /// 按扩展名精确查找第一个匹配的 parser。
    fn find_by_extension(&self, file_name: &str) -> Option<&dyn Parser> {
        let ext = Path::new(file_name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());
        let ext = ext.as_deref()?;
        self.parsers
            .iter()
            .find(|p| p.extensions().contains(&ext))
            .map(|p| p.as_ref())
    }

    /// 按魔数 mime 检测，返回候选 parser（按扩展名额外匹配度排序）。
    fn find_by_mime(&self, path: &Path) -> Vec<&dyn Parser> {
        // 读前 8K 字节做魔数检测
        let bytes = std::fs::File::open(path)
            .and_then(|mut f| {
                use std::io::Read;
                let mut buf = vec![0u8; 8192];
                let n = f.read(&mut buf)?;
                buf.truncate(n);
                Ok(buf)
            })
            .ok();

        let detected = bytes
            .as_deref()
            .and_then(|b| infer::get(b).map(|t| t.mime_type().to_string()));

        let file_ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        let mut candidates: Vec<(&dyn Parser, usize)> = self
            .parsers
            .iter()
            .map(|p| p.as_ref())
            .filter_map(|p| {
                let mime_match = detected
                    .as_ref()
                    .map(|m| p.mimes().iter().any(|pm| *pm == m))
                    .unwrap_or(false);
                let ext_match = file_ext
                    .as_deref()
                    .map(|fe| p.extensions().contains(&fe))
                    .unwrap_or(false);
                // mime 或扩展名任一匹配即为候选
                if mime_match || ext_match {
                    let score = (mime_match as usize) * 2 + (ext_match as usize);
                    Some((p, score))
                } else {
                    None
                }
            })
            .collect();

        // 按分数降序（mime 优先）
        candidates.sort_by(|a, b| b.1.cmp(&a.1));
        candidates.into_iter().map(|(p, _)| p).collect()
    }
}

impl ParserRegistry for ParserRegistryImpl {
    fn parse(&self, path: &Path) -> Result<ParseResult> {
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // 归档穿透：zip/tar 解包后递归解析内部文件
        if crate::archive::is_archive(path) {
            return crate::archive::parse_archive(path, self);
        }

        // 路径 1：魔数 mime 检测 → 多 parser 容错尝试
        let candidates = self.find_by_mime(path);
        if !candidates.is_empty() {
            for parser in &candidates {
                match parser.parse(path) {
                    Ok(mut result) => {
                        result.parser_name = parser.name();
                        return Ok(result);
                    }
                    Err(PivotsearchError::UnsupportedFormat(_)) => continue,
                    Err(e) => return Err(e),
                }
            }
            // 所有候选都失败，落到扩展名路径
        }

        // 路径 2：扩展名精确匹配
        if let Some(parser) = self.find_by_extension(file_name) {
            let mut result = parser.parse(path)?;
            result.parser_name = parser.name();
            return Ok(result);
        }

        // 路径 3：兜底
        let ext = Path::new(file_name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        Err(PivotsearchError::UnsupportedFormat(ext.to_string()))
    }

    fn can_parse_by_name(&self, file_name: &str) -> bool {
        self.find_by_extension(file_name).is_some()
    }

    fn list_parser_names(&self) -> Vec<&'static str> {
        self.parsers.iter().map(|p| p.name()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_can_parse_known_extensions() {
        let reg = ParserRegistryImpl::with_builtin_parsers();
        assert!(reg.can_parse_by_name("readme.md"));
        assert!(reg.can_parse_by_name("notes.txt"));
        assert!(reg.can_parse_by_name("page.html"));
        assert!(reg.can_parse_by_name("data.xlsx"));
        assert!(!reg.can_parse_by_name("unknown.xyz"));
    }

    #[test]
    fn registry_lists_parser_names() {
        let reg = ParserRegistryImpl::with_builtin_parsers();
        let names = reg.list_parser_names();
        assert!(names.contains(&"TextParser"));
        assert!(names.contains(&"MarkdownParser"));
        assert!(names.contains(&"HtmlParser"));
    }
}
