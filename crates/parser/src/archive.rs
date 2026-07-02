//! 归档穿透：zip/tar 解包到临时目录后递归解析内部文件。
//!
//! 这不是标准 Parser（归档返回多文件），而是工具函数。
//! 由增量索引遍历器调用：遇到归档文件时解包，对内部每个文件递归调用 registry.parse。
//!
//! 设计（复刻经典桌面搜索工具的归档穿透，净室）：
//! - zip/tar 解包到临时目录
//! - 对每个内部文件调用 parser_registry.parse
//! - 合并所有内部文件文本为一个 ParseResult
//! - 用完清理临时目录

use pivotsearch_contracts::{ParseResult, ParserRegistry, PivotsearchError, Result};
use std::path::{Path, PathBuf};

/// 判断文件是否为支持的归档类型。
pub fn is_archive(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_lowercase())
        .unwrap_or_default();
    // 处理双扩展名（tar.gz / tar.bz2）和单扩展名
    name.ends_with(".zip")
        || name.ends_with(".tar")
        || name.ends_with(".tar.gz")
        || name.ends_with(".tgz")
        || name.ends_with(".tar.bz2")
        || name.ends_with(".tbz2")
}

/// 解包归档并解析内部所有文件，合并文本。
///
/// 单个归档的 ParseResult.content = 所有内部文件文本拼接。
/// parser_name 标记为归档内各文件的 parser（主要 parser 名）。
pub fn parse_archive(path: &Path, registry: &dyn ParserRegistry) -> Result<ParseResult> {
    let temp_dir = tempfile::tempdir().map_err(|e| PivotsearchError::FsIo {
        path: path.display().to_string(),
        source: e,
    })?;

    extract_archive(path, temp_dir.path())?;

    let mut content = String::new();
    let mut main_parser = "";

    for entry in walkdir::WalkDir::new(temp_dir.path()).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let inner_path = entry.path();
        match registry.parse(inner_path) {
            Ok(result) => {
                if !result.content.is_empty() {
                    if !content.is_empty() {
                        content.push_str("\n\n");
                    }
                    content.push_str(&result.content);
                    if main_parser.is_empty() {
                        main_parser = result.parser_name;
                    }
                }
            }
            Err(_) => continue, // 归档内不支持的文件跳过
        }
    }

    Ok(ParseResult {
        content,
        parser_name: if main_parser.is_empty() { "ArchiveParser" } else { main_parser },
        ..Default::default()
    })
}

/// 解包归档到目标目录。
fn extract_archive(path: &Path, dest: &Path) -> Result<()> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "zip" => extract_zip(path, dest),
        "tar" | "tgz" => extract_tar(path, dest, false),
        "gz" => {
            // .tar.gz
            extract_tar(path, dest, true)
        }
        "bz2" => extract_tar(path, dest, false), // 简化：bz2 也尝试 tar
        _ => Err(PivotsearchError::UnsupportedFormat(ext)),
    }
}

/// 解压 zip。
fn extract_zip(path: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(path).map_err(|e| PivotsearchError::FsIo {
        path: path.display().to_string(),
        source: e,
    })?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| PivotsearchError::ParseFailed {
        path: path.display().to_string(),
        reason: format!("zip extract: {e}"),
    })?;

    archive.extract(dest).map_err(|e| PivotsearchError::ParseFailed {
        path: path.display().to_string(),
        reason: format!("zip extract: {e}"),
    })?;
    Ok(())
}

/// 解包 tar（可选 gzip）。
fn extract_tar(path: &Path, dest: &Path, gzip: bool) -> Result<()> {
    let file = std::fs::File::open(path).map_err(|e| PivotsearchError::FsIo {
        path: path.display().to_string(),
        source: e,
    })?;

    let tar: Box<dyn std::io::Read> = if gzip {
        Box::new(flate2::read::GzDecoder::new(file))
    } else {
        Box::new(file)
    };

    tar::Archive::new(tar)
        .unpack(dest)
        .map_err(|e| PivotsearchError::ParseFailed {
            path: path.display().to_string(),
            reason: format!("tar extract: {e}"),
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn is_archive_detection() {
        assert!(is_archive(Path::new("data.zip")));
        assert!(is_archive(Path::new("backup.tar")));
        assert!(is_archive(Path::new("backup.tar.gz")));
        assert!(!is_archive(Path::new("readme.md")));
        assert!(!is_archive(Path::new("report.pdf")));
    }

    #[test]
    fn extract_zip_works() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("test.zip");

        // 创建含两个 txt 的 zip
        let zip_file = std::fs::File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(zip_file);
        let opts = SimpleFileOptions::default();
        writer.start_file("a.txt", opts).unwrap();
        writer.write_all(b"alpha content").unwrap();
        writer.start_file("b.txt", opts).unwrap();
        writer.write_all(b"beta content").unwrap();
        writer.finish().unwrap();

        // 解包
        let dest = tempfile::tempdir().unwrap();
        extract_zip(&zip_path, dest.path()).unwrap();
        assert!(dest.path().join("a.txt").exists());
        assert!(dest.path().join("b.txt").exists());
        assert_eq!(std::fs::read_to_string(dest.path().join("a.txt")).unwrap(), "alpha content");
    }
}
