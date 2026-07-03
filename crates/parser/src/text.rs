//! Plain text / source code parser.
//!
//! Uses chardetng to detect encoding (GBK/Big5/UTF-8, etc.) and encoding_rs to transcode to UTF-8.

use chardetng::{Iso2022JpDetection, Utf8Detection};
use pivotsearch_contracts::{ParseResult, Parser, PivotsearchError, Result};
use std::path::Path;

/// Plain text parser. Also handles source code files (identified by extension; content treated as text).
#[derive(Default, Clone)]
pub struct TextParser;

impl Parser for TextParser {
    fn extensions(&self) -> &[&str] {
        &[
            "txt", "log", "csv", "tsv",
            "rs", "go", "py", "js", "ts", "java", "c", "cpp", "h", "hpp",
            "cs", "rb", "php", "swift", "kt", "scala", "sh", "bash", "zsh",
            "sql", "yaml", "yml", "toml", "ini", "cfg", "conf", "json", "xml",
        ]
    }

    fn mimes(&self) -> &[&str] {
        &["text/plain", "text/csv", "application/json", "application/xml", "application/yaml"]
    }

    fn parse(&self, path: &Path) -> Result<ParseResult> {
        let bytes = std::fs::read(path).map_err(|e| PivotsearchError::FsIo {
            path: path.display().to_string(),
            source: e,
        })?;

        // Encoding detection + transcoding
        let mut detector = chardetng::EncodingDetector::new(Iso2022JpDetection::Allow);
        detector.feed(&bytes, true);
        let encoding = detector.guess(None, Utf8Detection::Allow);
        let (content, _, _) = encoding.decode(&bytes);

        Ok(ParseResult::new(content.into_owned()))
    }

    fn name(&self) -> &'static str {
        "TextParser"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_utf8_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        std::fs::write(&path, "Hello 世界").unwrap();
        let result = TextParser.parse(&path).unwrap();
        assert_eq!(result.content, "Hello 世界");
    }

    #[test]
    fn parse_gbk_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gbk.txt");
        // GBK-encoded bytes for the two CJK characters asserted below
        std::fs::write(&path, [0xD6, 0xD0, 0xCE, 0xC4]).unwrap();
        let result = TextParser.parse(&path).unwrap();
        assert_eq!(result.content, "中文");
    }
}
