//! 增量索引算法：mtime 比对 + unseenDocs 文件树 diff + 归档整体跳过。
//!
//! 设计（复刻经典桌面搜索工具的 visitDirOrZip 模式，净室重写）：
//! 1. 从 tree_index 加载已索引文件集合（path → IndexedFile）
//! 2. 克隆为 unseen（待"扫视"），遍历磁盘每见到一个就 remove
//! 3. 遍历完后 unseen 剩余的 = 磁盘已删除的
//! 4. 新增/修改（mtime 不同）→ 解析 + upsert；未变 → 跳过；删除 → delete_term
//!
//! 增量判定以 mtime 为主键（不用 hash，性能优）：
//! `is_modified = old_mtime != new_mtime`

use crate::doc_builder::{build_document, compute_uid};
use crate::schema::SchemaFields;
use crate::tree_index::{IndexedFile, TreeIndex};
use pivotsearch_contracts::{IndexAction, ParserRegistry, PivotsearchError, Result, UpdateResult};
use std::collections::HashMap;
use std::path::Path;
use tantivy::{IndexWriter, Term};

/// 增量索引器的配置。
pub struct IncrementalConfig {
    /// 要跳过的文件名模式（如 lock 文件、隐藏文件）。
    pub skip_patterns: Vec<String>,
    /// index_id（所属索引根标识）。
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

/// 单个文件的处理结果（内部统计用）。
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

/// 执行一次增量更新。
///
/// - action=Update：增量（mtime 比对，只处理变化文件）
/// - action=Rebuild：全量重建（先清空再索引）
///
/// 返回 UpdateResult::SuccessChanged / SuccessUnchanged / Failure。
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

/// 带进度回调的增量更新。progress 回调每 100 个文件调用一次。
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

    // 预统计文件总数（快速 stat，不解析内容）
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

    // 全量重建：先清空 tree_index 中该 index_id 的所有文件 + Tantivy 该 index_id 的文档
    if action == IndexAction::Rebuild {
        let existing = tree_index.files_for_index(&config.index_id)?;
        for file in &existing {
            let _ = delete_doc(writer, fields, &file.uid);
        }
        // tree_index 记录由后续逻辑重建（先全删再重新遍历）
        for file in &existing {
            tree_index.delete_file(&file.uid)?;
        }
    }

    // 加载已索引文件（增量用 unseen diff；全量重建后为空）
    let mut unseen: HashMap<String, IndexedFile> = tree_index
        .files_for_index(&config.index_id)?
        .into_iter()
        .map(|f| (f.path.clone(), f))
        .collect();

    // 遍历磁盘目录
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

        // 跳过 lock / 隐藏文件
        if file_name.ends_with(".lock") || file_name.starts_with('.') {
            stats.skipped_pattern += 1;
            continue;
        }

        // 从 unseen 移除（见到了）
        let existing = unseen.remove(&path_str);

        // 计算 mtime
        let mtime = match file_mtime(path) {
            Ok(m) => m,
            Err(_) => {
                stats.errors += 1;
                continue;
            }
        };

        match existing {
            None => {
                // 新增文件
                match index_one(path, fields, writer, parser_registry, &config.index_id, mtime, tree_index) {
                    Ok(true) => stats.added += 1,
                    Ok(false) => stats.errors += 1,
                    Err(_) => stats.errors += 1,
                }
            }
            Some(old) if old.mtime != mtime => {
                // 修改文件：delete + add（upsert）
                match index_one(path, fields, writer, parser_registry, &config.index_id, mtime, tree_index) {
                    Ok(true) => stats.modified += 1,
                    Ok(false) => stats.errors += 1,
                    Err(_) => stats.errors += 1,
                }
            }
            Some(_) => {
                // 未变，跳过
                stats.skipped_unchanged += 1;
            }
        }
    }

    // unseen 剩余 = 磁盘上已删除的
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

    // 最终进度回调（100%）
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

/// 索引单个文件：解析 → upsert Tantivy doc → 记录 tree_index。
/// 返回 true=成功，false=解析失败（已记录 tree_index 避免重试）。
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
            // upsert：先删后加
            delete_doc(writer, fields, &uid)?;
            writer
                .add_document(doc)
                .map_err(|e| PivotsearchError::IndexIo(e.to_string()))?;
            // 记录 tree_index
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
            // 不支持的格式：仍记录到 tree_index（parser=null），避免下次重复尝试
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
            // 解析失败：记录 tree_index（parser=null）避免重试坏文件
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

/// 删除 Tantivy 中某 uid 的文档。
fn delete_doc(writer: &mut IndexWriter, fields: &SchemaFields, uid: &str) -> Result<()> {
    writer.delete_term(Term::from_field_text(fields.uid, uid));
    Ok(())
}

/// 取文件 mtime（毫秒时间戳）。
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

    /// 测试用 parser：只处理 .txt，返回固定内容。
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

        // 第一次：新增
        update_incremental(dir.path(), IndexAction::Update, &config, &fields, &mut writer, &ti, &registry).unwrap();
        // 第二次：应全跳过
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

        // 修改文件（确保 mtime 变化）
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

        // 删除 b.txt
        std::fs::remove_file(&b).unwrap();
        let result = update_incremental(dir.path(), IndexAction::Update, &config, &fields, &mut writer, &ti, &registry).unwrap();
        assert_eq!(result, UpdateResult::SuccessChanged);
        assert_eq!(ti.count_files("default").unwrap(), 1);
    }
}
