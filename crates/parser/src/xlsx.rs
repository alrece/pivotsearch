//! 电子表格解析器（calamine）：xlsx/xls/csv。

use calamine::{open_workbook, Data, Reader, Xlsx};
use pivotsearch_contracts::{ParseResult, Parser, PivotsearchError, Result};
use std::path::Path;

/// 电子表格解析器。用 calamine 读取所有 sheet 所有单元格的值，拼成文本。
pub struct SpreadsheetParser;

impl Parser for SpreadsheetParser {
    fn extensions(&self) -> &[&str] {
        &["xlsx", "xls"]
    }

    fn mimes(&self) -> &[&str] {
        &[
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "application/vnd.ms-excel",
        ]
    }

    fn parse(&self, path: &Path) -> Result<ParseResult> {
        let mut workbook: Xlsx<_> = open_workbook(path).map_err(|e| PivotsearchError::ParseFailed {
            path: path.display().to_string(),
            reason: format!("{e:?}"),
        })?;

        let sheets = workbook.worksheets();
        let mut content = String::new();

        for (sheet_name, range) in sheets {
            if !content.is_empty() {
                content.push_str("\n\n");
            }
            content.push_str(&format!("[{sheet_name}]\n"));

            // 遍历所有行所有单元格
            for row in range.rows() {
                let mut row_text = Vec::new();
                for cell in row {
                    let cell_str = match cell {
                        Data::Int(i) => i.to_string(),
                        Data::Float(f) => f.to_string(),
                        Data::String(s) => s.clone(),
                        Data::DateTime(dt) => dt.to_string(),
                        Data::Bool(b) => b.to_string(),
                        Data::Error(e) => format!("{e:?}"),
                        Data::DurationIso(s) | Data::DateTimeIso(s) => s.clone(),
                        Data::Empty => String::new(),
                    };
                    row_text.push(cell_str);
                }
                // 跳过全空行
                if row_text.iter().any(|s| !s.is_empty()) {
                    content.push_str(&row_text.join("\t"));
                    content.push('\n');
                }
            }
        }

        Ok(ParseResult {
            content,
            ..Default::default()
        })
    }

    fn name(&self) -> &'static str {
        "SpreadsheetParser"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_include_xlsx_and_xls() {
        let exts = SpreadsheetParser.extensions();
        assert!(exts.contains(&"xlsx"));
        assert!(exts.contains(&"xls"));
    }
}
