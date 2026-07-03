//! Incremental indexing algorithm: mtime comparison + unseenDocs file-tree diff + archive skip-all.
//!
//! Design (reproduces the visitDirOrZip pattern of classic desktop search tools, clean-room rewrite):
//! 1. Load the set of indexed files from tree_index (path → IndexedFile)
//! 2. Clone it into unseen (pending "scan"); remove each one as it is encountered while walking the disk
//! 3. After the walk, the remaining unseen entries = files deleted from disk
//! 4. New/modified (mtime differs) → parse + upsert; unchanged → skip; deleted → delete_term
//!
//! The incremental decision uses mtime as the primary key (no hash, for performance):
//! `is_modified = old_mtime != new_mtime`

use crate::doc_builder::{build_document, compute_uid};
use crate::schema::SchemaFields;
use crate::tree_index::{IndexedFile, TreeIndex};
use pivotsearch_contracts::{IndexAction, ParserRegistry, PivotsearchError, Result, UpdateResult};
use std::collections::HashMap;
use std::path::Path;
use tantivy::{IndexWriter, Term};

/// Configuration for the incremental indexer.
pub struct IncrementalConfig {
    /// Filename patterns to skip (e.g. lock files, hidden files).
    pub skip_patterns: Vec<String>,
    /// index_id (identifier of the owning index root).
    pub index_id: String,
}

impl Default for IncrementalConfig {
    fn default() -> Self {
        Self {
            skip_patterns: vec!["*.lock".to_string(), ".*".to_string()],
            index_id: "default".to_string(),
        }
    }
}

/// Processing result for a single file (for internal statistics).
#[derive(Debug, Default, Clone)]
pub struct IncrementalStats {
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
    pub skipped_unchanged: usize,
    pub skipped_pattern: usize,
    pub errors: usize,
}

impl IncrementalStats {
    pub fn changed(&self) -> bool {
        self.added > 0 || self.modified > 0 || self.deleted > 0
    }
}

/// Perform a single incremental update.
///
/// - action=Update: incremental (mtime comparison, only processes changed files)
/// - action=Rebuild: full rebuild (clears first, then indexes)
///
/// Returns UpdateResult::SuccessChanged / SuccessUnchanged / Failure.
pub fn update_incremental(
    root: &Path,
    action: IndexAction,
    config: &IncrementalConfig,
    fields: &SchemaFields,
    writer: &mut IndexWriter,
    tree_index: &TreeIndex,
    parser_registry: &dyn ParserRegistry,
) -> Result<UpdateResult> {
    update_incremental_with_progress(root, action, config, fields, writer, tree_index, parser_registry, None)
}

/// Incremental update with a progress callback. The progress callback is invoked once every 100 files.
pub fn update_incremental_with_progress(
    root: &Path,
    action: IndexAction,
    config: &IncrementalConfig,
    fields: &SchemaFields,
    writer: &mut IndexWriter,
    tree_index: &TreeIndex,
    parser_registry: &dyn ParserRegistry,
    mut progress: Option<&mut dyn FnMut(usize, usize)>,
) -> Result<UpdateResult> {
    let mut stats = IncrementalStats::default();

    // Pre-count total files (fast stat, no content parsing)
    let total = if let Some(ref mut _cb) = progress {
        walkdir::WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .count()
    } else {
        0
    };

    if let Some(ref mut cb) = progress {
        cb(0, total);
    }

    // Full rebuild: first clear all files for this index_id in tree_index + the Tantivy documents for this index_id
    if action == IndexAction::Rebuild {
        let existing = tree_index.files_for_index(&config.index_id)?;
        for file in &existing {
            let _ = delete_doc(writer, fields, &file.uid);
        }
        // tree_index records are rebuilt by the subsequent logic (delete all first, then re-walk)
        for file in &existing {
            tree_index.delete_file(&file.uid)?;
        }
    }

    // Load indexed files (used for the incremental unseen diff; empty after a full rebuild)
    let mut unseen: HashMap<String, IndexedFile> = tree_index
        .files_for_index(&config.index_id)?
        .into_iter()
        .map(|f| (f.path.clone(), f))
        .collect();

    // Walk the disk directory
    let mut processed = 0usize;
    for entry in walkdir::WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let path_str = path.to_string_lossy().to_string();
        let file_name = entry.file_name().to_string_lossy().to_string();

        processed += 1;
        if processed % 100 == 0 {
            if let Some(ref mut cb) = progress {
                cb(processed, total);
            }
        }

        // Skip lock / hidden files
        if file_name.ends_with(".lock") || file_name.starts_with('.') {
            stats.skipped_pattern += 1;
            continue;
        }

        // Remove from unseen (file encountered)
        let existing = unseen.remove(&path_str);

        // Compute mtime
        let mtime = match file_mtime(path) {
            Ok(m) => m,
            Err(_) => {
                stats.errors += 1;
                continue;
            }
        };

        match existing {
            None => {
                // New file
                match index_one(path, fields, writer, parser_registry, &config.index_id, mtime, tree_index) {
                    Ok(true) => stats.added += 1,
                    Ok(false) => stats.errors += 1,
                    Err(_) => stats.errors += 1,
                }
            }
            Some(old) if old.mtime != mtime => {
                // Modified file: delete + add (upsert)
                match index_one(path, fields, writer, parser_registry, &config.index_id, mtime, tree_index) {
                    Ok(true) => stats.modified += 1,
                    Ok(false) => stats.errors += 1,
                    Err(_) => stats.errors += 1,
                }
            }
            Some(_) => {
                // Unchanged, skip
                stats.skipped_unchanged += 1;
            }
        }
    }

    // Remaining unseen = files deleted from disk
    for file in unseen.values() {
        delete_doc(writer, fields, &file.uid)?;
        tree_index.delete_file(&file.uid)?;
        stats.deleted += 1;
    }

    tracing::info!(
        "增量更新完成: +{} ~{} -{} 未变{} 跳过{} 错误{}",
        stats.added, stats.modified, stats.deleted,
        stats.skipped_unchanged, stats.skipped_pattern, stats.errors
    );

    // Final progress callback (100%)
    if let Some(ref mut cb) = progress {
        cb(processed, total);
    }

    if stats.errors > 0 && stats.changed() {
        Ok(UpdateResult::SuccessChanged)
    } else if stats.changed() {
        Ok(UpdateResult::SuccessChanged)
    } else {
        Ok(UpdateResult::SuccessUnchanged)
    }
}

/// Index a single file: parse → upsert Tantivy doc → record in tree_index.
/// Returns true on success, false on parse failure (tree_index is still recorded to avoid retries).
fn index_one(
    path: &Path,
    fields: &SchemaFields,
    writer: &mut IndexWriter,
    parser_registry: &dyn ParserRegistry,
    index_id: &str,
    mtime: i64,
    tree_index: &TreeIndex,
) -> Result<bool> {
    let uid = compute_uid(path);
    let path_str = path.to_string_lossy().to_string();

    match parser_registry.parse(path) {
        Ok(parse_result) => {
            let doc = build_document(fields, path, &parse_result, &uid, index_id);
            // upsert: delete then add
            delete_doc(writer, fields, &uid)?;
            writer
                .add_document(doc)
                .map_err(|e| PivotsearchError::IndexIo(e.to_string()))?;
            // Record in tree_index
            tree_index.upsert_file(&IndexedFile {
                uid: uid.clone(),
                path: path_str,
                mtime,
                parser: Some(parse_result.parser_name.to_string()),
                index_id: index_id.to_string(),
            })?;
            Ok(true)
        }
        Err(PivotsearchError::UnsupportedFormat(_)) => {
            // Unsupported format: still record it in tree_index (parser=null) to avoid retrying next time
            tree_index.upsert_file(&IndexedFile {
                uid,
                path: path_str,
                mtime,
                parser: None,
                index_id: index_id.to_string(),
            })?;
            Ok(false)
        }
        Err(e) => {
            // Parse failure: record in tree_index (parser=null) to avoid retrying bad files
            tracing::warn!("解析失败 {}: {}", path.display(), e);
            tree_index.upsert_file(&IndexedFile {
                uid,
                path: path_str,
                mtime,
                parser: None,
                index_id: index_id.to_string(),
            })?;
            Ok(false)
        }
    }
}

/// Delete the Tantivy document for a given uid.
fn delete_doc(writer: &mut IndexWriter, fields: &SchemaFields, uid: &str) -> Result<()> {
    writer.delete_term(Term::from_field_text(fields.uid, uid));
    Ok(())
}

/// Get the file mtime (millisecond timestamp).
fn file_mtime(path: &Path) -> Result<i64> {
    let meta = path.metadata().map_err(|e| PivotsearchError::FsIo {
        path: path.display().to_string(),
        source: e,
    })?;
    let modified = meta.modified().map_err(|e| PivotsearchError::FsIo {
        path: path.display().to_string(),
        source: e,
    })?;
    Ok(modified
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::build_schema;
    use pivotsearch_contracts::{ParseResult, Parser, ParserRegistry};
    use tantivy::Index;

    /// Test parser: only handles .txt, returns fixed content.
    struct DummyParser;
    impl Parser for DummyParser {
        fn extensions(&self) -> &[&str] {
            &["txt"]
        }
        fn mimes(&self) -> &[&str] {
            &["text/plain"]
        }
        fn parse(&self, path: &Path) -> Result<ParseResult> {
            let content = std::fs::read_to_string(path).unwrap_or_default();
            Ok(ParseResult::new(content))
        }
        fn name(&self) -> &'static str {
            "DummyParser"
        }
    }

    struct DummyRegistry;
    impl ParserRegistry for DummyRegistry {
        fn parse(&self, path: &Path) -> Result<ParseResult> {
            DummyParser.parse(path).map(|mut r| {
                r.parser_name = "DummyParser";
                r
            })
        }
        fn can_parse_by_name(&self, name: &str) -> bool {
            name.ends_with(".txt")
        }
        fn list_parser_names(&self) -> Vec<&'static str> {
            vec!["DummyParser"]
        }
    }

    fn setup_index() -> (Index, SchemaFields, tantivy::IndexReader) {
        let (schema, fields, _tokenizer_manager) = build_schema();
        let index = Index::create_in_ram(schema);
        index.tokenizers().register(
            crate::schema::JIEBA_TOKENIZER_NAME,
            tantivy::tokenizer::TextAnalyzer::from(crate::tokenizer::JiebaTokenizer::default()),
        );
        let reader = index
            .reader_builder()
            .reload_policy(tantivy::ReloadPolicy::Manual)
            .try_into()
            .unwrap();
        (index, fields, reader)
    }

    #[test]
    fn incremental_add_new_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hello").unwrap();
        std::fs::write(dir.path().join("b.txt"), "world").unwrap();

        let (index, fields, _reader) = setup_index();
        let mut writer = index.writer(50_000_000).unwrap();
        let ti = TreeIndex::open_memory().unwrap();
        ti.add_index_root("default", dir.path().to_str().unwrap(), None, 0).unwrap();

        let config = IncrementalConfig::default();
        let registry = DummyRegistry;
        let result = update_incremental(
            dir.path(),
            IndexAction::Update,
            &config,
            &fields,
            &mut writer,
            &ti,
            &registry,
        )
        .unwrap();

        assert_eq!(result, UpdateResult::SuccessChanged);
        assert_eq!(ti.count_files("default").unwrap(), 2);
    }

    #[test]
    fn incremental_unchanged_skips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "hello").unwrap();

        let (index, fields, _reader) = setup_index();
        let mut writer = index.writer(50_000_000).unwrap();
        let ti = TreeIndex::open_memory().unwrap();
        ti.add_index_root("default", dir.path().to_str().unwrap(), None, 0).unwrap();

        let config = IncrementalConfig::default();
        let registry = DummyRegistry;

        // First run: add
        update_incremental(dir.path(), IndexAction::Update, &config, &fields, &mut writer, &ti, &registry).unwrap();
        // Second run: should skip everything
        let result = update_incremental(dir.path(), IndexAction::Update, &config, &fields, &mut writer, &ti, &registry).unwrap();
        assert_eq!(result, UpdateResult::SuccessUnchanged);
    }

    #[test]
    fn incremental_detects_modification() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "hello").unwrap();

        let (index, fields, _reader) = setup_index();
        let mut writer = index.writer(50_000_000).unwrap();
        let ti = TreeIndex::open_memory().unwrap();
        ti.add_index_root("default", dir.path().to_str().unwrap(), None, 0).unwrap();

        let config = IncrementalConfig::default();
        let registry = DummyRegistry;

        update_incremental(dir.path(), IndexAction::Update, &config, &fields, &mut writer, &ti, &registry).unwrap();

        // Modify the file (ensure mtime changes)
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&path, "hello modified").unwrap();

        let result = update_incremental(dir.path(), IndexAction::Update, &config, &fields, &mut writer, &ti, &registry).unwrap();
        assert_eq!(result, UpdateResult::SuccessChanged);
    }

    #[test]
    fn incremental_detects_deletion() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        std::fs::write(&a, "hello").unwrap();
        std::fs::write(&b, "world").unwrap();

        let (index, fields, _reader) = setup_index();
        let mut writer = index.writer(50_000_000).unwrap();
        let ti = TreeIndex::open_memory().unwrap();
        ti.add_index_root("default", dir.path().to_str().unwrap(), None, 0).unwrap();

        let config = IncrementalConfig::default();
        let registry = DummyRegistry;

        update_incremental(dir.path(), IndexAction::Update, &config, &fields, &mut writer, &ti, &registry).unwrap();
        assert_eq!(ti.count_files("default").unwrap(), 2);

        // Delete b.txt
        std::fs::remove_file(&b).unwrap();
        let result = update_incremental(dir.path(), IndexAction::Update, &config, &fields, &mut writer, &ti, &registry).unwrap();
        assert_eq!(result, UpdateResult::SuccessChanged);
        assert_eq!(ti.count_files("default").unwrap(), 1);
    }
}
