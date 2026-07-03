//! Archive traversal: unzip/untar to a temp directory, then recursively parse inner files.
//!
//! This is not a standard Parser (archives return multiple files); it is a utility function.
//! Called by the incremental index traverser: when it encounters an archive, it unpacks it
//! and recursively calls registry.parse on each inner file.
//!
//! Design (clean-room reimplementation of archive traversal from classic desktop search tools):
//! - Unzip/untar to a temp directory
//! - Call parser_registry.parse on each inner file
//! - Merge all inner files' text into a single ParseResult
//! - Clean up the temp directory when done

use pivotsearch_contracts::{ParseResult, ParserRegistry, PivotsearchError, Result};
use std::path::Path;

/// Determines whether a file is a supported archive type.
pub fn is_archive(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_lowercase())
        .unwrap_or_default();
    // Handle double extensions (tar.gz / tar.bz2) and single extensions
    name.ends_with(".zip")
        || name.ends_with(".tar")
        || name.ends_with(".tar.gz")
        || name.ends_with(".tgz")
        || name.ends_with(".tar.bz2")
        || name.ends_with(".tbz2")
}

/// Unpacks the archive and parses all inner files, merging their text.
///
/// The ParseResult.content of a single archive = the concatenation of all inner files' text.
/// parser_name is marked with each inner file's parser (the primary parser name).
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
            Err(_) => continue, // Skip unsupported files inside the archive
        }
    }

    Ok(ParseResult {
        content,
        parser_name: if main_parser.is_empty() { "ArchiveParser" } else { main_parser },
        ..Default::default()
    })
}

/// Unpacks the archive into the destination directory.
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
        "bz2" => extract_tar(path, dest, false), // Simplification: also try tar for bz2
        _ => Err(PivotsearchError::UnsupportedFormat(ext)),
    }
}

/// Unzips a zip archive.
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

/// Unpacks a tar archive (optionally gzip-compressed).
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

        // Create a zip containing two txt files
        let zip_file = std::fs::File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(zip_file);
        let opts = SimpleFileOptions::default();
        writer.start_file("a.txt", opts).unwrap();
        writer.write_all(b"alpha content").unwrap();
        writer.start_file("b.txt", opts).unwrap();
        writer.write_all(b"beta content").unwrap();
        writer.finish().unwrap();

        // Unpack
        let dest = tempfile::tempdir().unwrap();
        extract_zip(&zip_path, dest.path()).unwrap();
        assert!(dest.path().join("a.txt").exists());
        assert!(dest.path().join("b.txt").exists());
        assert_eq!(std::fs::read_to_string(dest.path().join("a.txt")).unwrap(), "alpha content");
    }
}
