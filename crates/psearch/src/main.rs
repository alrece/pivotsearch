//! # psearch CLI
//!
//! Command-line tool for pivotsearch, designed for AI Agent consumption.
//!
//! Usage:
//!   psearch [--lang <en|zh>] index <dir> [--name NAME] [--rebuild]
//!   psearch [--lang <en|zh>] search <query> [--json] [--index ID] [--type EXT] [--case-sensitive] [--page N] [--limit N]
//!   psearch [--lang <en|zh>] list [--json]
//!   psearch [--lang <en|zh>] remove <id>
//!   psearch [--lang <en|zh>] rebuild <id>
//!   psearch [--lang <en|zh>] preview <uid> [--json]
//!   psearch [--lang <en|zh>] status
//!   psearch [--lang <en|zh>] version
//!
//! The data directory is shared with the desktop app:
//!   ~/Library/Application Support/com.pivotsearch.app/indexes/
//!
//! `--lang` controls ONLY human-readable output (progress, status, error
//! explanations). JSON payloads always use fixed English keys regardless of
//! `--lang`, so AI Agent JSON parsing stays stable.

use clap::{Parser, Subcommand};
use pivotsearch_contracts::{IndexAction, ParserRegistry, SearchRequest};
use pivotsearch_index::{
    build_schema, incremental::*, schema::JIEBA_TOKENIZER_NAME, tokenizer::JiebaTokenizer,
    tree_index::*,
};
use pivotsearch_parser::ParserRegistryImpl;
use pivotsearch_search::{MultiIndexSearcher, SearchSchemaFields, SimpleSearcher};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tantivy::Index;

// ====================================================================
// CLI definition
// ====================================================================

#[derive(Parser)]
#[command(
    name = "psearch",
    version,
    about = "pivotsearch CLI — local full-text search for Agent use"
)]
struct Cli {
    /// Output language for human-readable text (JSON keys stay English).
    /// Default: en. Use `zh` for Chinese progress/status/error messages.
    #[arg(long, default_value = "en", global = true)]
    lang: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add or update an index directory
    Index {
        /// Directory path to index
        dir: String,
        /// Display name for the index
        #[arg(long)]
        name: Option<String>,
        /// Full rebuild (instead of incremental update)
        #[arg(long)]
        rebuild: bool,
    },
    /// Search file contents
    Search {
        /// Search keyword
        query: String,
        /// JSON output (Agent-friendly)
        #[arg(long)]
        json: bool,
        /// Restrict to specific index root IDs (repeatable)
        #[arg(long = "index", num_args = 0..)]
        index: Option<Vec<String>>,
        /// File type filter (e.g. pdf/docx/md)
        #[arg(long)]
        r#type: Option<String>,
        /// Case-sensitive search
        #[arg(long)]
        case_sensitive: bool,
        /// Page number (0-based, default 0)
        #[arg(long, default_value = "0")]
        page: usize,
        /// Max results (default 50)
        #[arg(long, default_value = "50")]
        limit: usize,
    },
    /// List all index roots
    List {
        /// JSON output
        #[arg(long)]
        json: bool,
    },
    /// Remove an index root
    Remove {
        /// Index root ID
        id: String,
    },
    /// Full-rebuild an index
    Rebuild {
        /// Index root ID
        id: String,
    },
    /// Preview full file content
    Preview {
        /// File UID (file://path)
        uid: String,
        /// JSON output
        #[arg(long)]
        json: bool,
    },
    /// Show status information
    Status,
    /// Show version
    Version,
}

// ====================================================================
// i18n — human-readable strings. JSON payloads are NOT affected.
// ====================================================================

/// Supported human-readable languages. Defaults to English.
enum Lang {
    En,
    Zh,
}

impl Lang {
    fn from_code(code: &str) -> Self {
        match code.to_lowercase().as_str() {
            "zh" | "zh-cn" | "zh_cn" | "chinese" | "cn" => Lang::Zh,
            _ => Lang::En,
        }
    }
}

/// Translation helper for human-readable CLI output. Add new strings here in
/// both languages; JSON output never goes through this.
struct T<'a>(&'a Lang);

impl<'a> T<'a> {
    /// Pick one of two static strings by language.
    fn s(&self, en: &'a str, zh: &'a str) -> &'a str {
        match self.0 {
            Lang::En => en,
            Lang::Zh => zh,
        }
    }

    /// Pick one of two formatted strings by language. Use this when the
    /// message contains interpolated values.
    fn f(&self, en: impl std::fmt::Display, zh: impl std::fmt::Display) -> String {
        match self.0 {
            Lang::En => en.to_string(),
            Lang::Zh => zh.to_string(),
        }
    }
}

// ====================================================================
// JSON output envelope (language-independent, fixed English keys)
// ====================================================================

#[derive(Serialize)]
struct OkResponse<T: Serialize> {
    ok: bool,
    data: T,
}

#[derive(Serialize)]
struct ErrorResponse {
    ok: bool,
    error: ErrorInfo,
}

#[derive(Serialize)]
struct ErrorInfo {
    code: String,
    message: String,
}

fn print_json_ok<T: Serialize>(data: T) {
    let resp = OkResponse { ok: true, data };
    println!("{}", serde_json::to_string_pretty(&resp).unwrap_or_default());
}

fn print_json_err(code: &str, message: &str) {
    let resp = ErrorResponse {
        ok: false,
        error: ErrorInfo {
            code: code.to_string(),
            message: message.to_string(),
        },
    };
    println!("{}", serde_json::to_string_pretty(&resp).unwrap_or_default());
}

// ====================================================================
// Data directory (shared with the desktop app)
// ====================================================================

const APP_IDENTIFIER: &str = "com.pivotsearch.app";

fn data_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join(APP_IDENTIFIER)
}

fn indexes_dir() -> PathBuf {
    data_dir().join("indexes")
}

/// Scan the indexes/ directory and recover all known indexes {index_id → index_dir}.
fn scan_indexes() -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();
    let dir = indexes_dir();
    if !dir.exists() {
        return map;
    }
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("idx-") {
                map.insert(name, entry.path());
            }
        }
    }
    map
}

/// Read index-root info from tree_index.sqlite.
fn read_index_info(index_dir: &Path) -> Option<(String, String, Option<String>, u64)> {
    let tree_path = index_dir.join("tree_index.sqlite");
    let ti = TreeIndex::open(&tree_path).ok()?;
    let roots = ti.list_index_roots().ok()?;
    let root = roots.first()?;
    let count = ti.count_files(&root.id).unwrap_or(0);
    Some((root.id.clone(), root.path.clone(), root.display_name.clone(), count))
}

// ====================================================================
// Helper: build a searcher
// ====================================================================

fn build_multi_searcher(index_dirs: &HashMap<String, PathBuf>, lang: &Lang) -> MultiIndexSearcher {
    let t = T(lang);
    let (_schema, fields, tm) = build_schema();
    let mut multi = MultiIndexSearcher::new();

    for (index_id, index_dir) in index_dirs {
        let tantivy_dir = index_dir.join("tantivy");
        let index = match Index::open_in_dir(&tantivy_dir) {
            Ok(i) => i,
            Err(e) => {
                eprintln!(
                    "{}",
                    t.s(
                        &format!("[warn] failed to open index {}: {}", index_id, e),
                        &format!("[警告] 打开索引 {} 失败: {}", index_id, e),
                    )
                );
                continue;
            }
        };
        index.tokenizers().register(
            JIEBA_TOKENIZER_NAME,
            tantivy::tokenizer::TextAnalyzer::from(JiebaTokenizer::default()),
        );
        let search_fields = SearchSchemaFields {
            uid: fields.uid,
            content: fields.content,
            snippet_text: fields.snippet_text,
            title: fields.title,
            author: fields.author,
            r#type: fields.r#type,
            parser: fields.parser,
            size: fields.size,
            last_modified: fields.last_modified,
            index_id: fields.index_id,
        };
        match SimpleSearcher::new(index, search_fields, tm.clone()) {
            Ok(s) => multi.add(index_id.clone(), s),
            Err(e) => eprintln!(
                "{}",
                t.s(
                    &format!("[warn] failed to build searcher {}: {}", index_id, e),
                    &format!("[警告] 构造 searcher {} 失败: {}", index_id, e),
                )
            ),
        }
    }
    multi
}

// ====================================================================
// Command implementations
// ====================================================================

fn cmd_index(dir: &str, name: &Option<String>, rebuild: bool, lang: &Lang) -> i32 {
    let t = T(lang);
    let root = PathBuf::from(dir);
    if !root.is_dir() {
        eprintln!(
            "{}",
            t.f(format!("error: not a directory: {dir}"), &format!("错误：不是目录: {dir}"))
        );
        return 2;
    }

    let index_id = format!("idx-{:x}", hash_path(dir));
    let display_name = name
        .clone()
        .or_else(|| {
            root.file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        });

    let index_dir = indexes_dir().join(&index_id);
    std::fs::create_dir_all(&index_dir).unwrap_or_else(|e| {
        eprintln!(
            "{}",
            t.s(
                &format!("failed to create index directory: {e}"),
                &format!("创建索引目录失败: {e}"),
            )
        );
        std::process::exit(1);
    });

    let (schema, fields, _) = build_schema();
    let tantivy_dir = index_dir.join("tantivy");
    std::fs::create_dir_all(&tantivy_dir).unwrap_or(());

    let tantivy_meta = tantivy_dir.join("meta.json");
    let index = if tantivy_meta.exists() {
        Index::open_in_dir(&tantivy_dir).unwrap_or_else(|e| {
            eprintln!(
                "{}",
                t.f(format!("failed to open index: {e}"), &format!("打开索引失败: {e}"))
            );
            std::process::exit(1);
        })
    } else {
        Index::create_in_dir(&tantivy_dir, schema).unwrap_or_else(|e| {
            eprintln!(
                "{}",
                t.f(format!("failed to create index: {e}"), &format!("创建索引失败: {e}"))
            );
            std::process::exit(1);
        })
    };
    index.tokenizers().register(
        JIEBA_TOKENIZER_NAME,
        tantivy::tokenizer::TextAnalyzer::from(JiebaTokenizer::default()),
    );

    let tree_path = index_dir.join("tree_index.sqlite");
    let tree_index = TreeIndex::open(&tree_path).unwrap_or_else(|e| {
        eprintln!(
            "{}",
            t.s(
                &format!("failed to open tree_index: {e}"),
                &format!("打开 tree_index 失败: {e}"),
            )
        );
        std::process::exit(1);
    });
    tree_index
        .add_index_root(&index_id, dir, display_name.as_deref(), now_millis())
        .unwrap_or_else(|e| {
            eprintln!(
                "{}",
                t.s(
                    &format!("failed to record index root: {e}"),
                    &format!("记录索引根失败: {e}"),
                )
            );
            std::process::exit(1);
        });

    let registry = ParserRegistryImpl::with_builtin_parsers().with_pdf();
    let action = if rebuild { IndexAction::Rebuild } else { IndexAction::Update };
    let config = IncrementalConfig {
        index_id: index_id.clone(),
        ..Default::default()
    };

    let mut writer = index.writer(50_000_000).unwrap_or_else(|e| {
        eprintln!(
            "{}",
            t.f(format!("failed to create writer: {e}"), &format!("创建 writer 失败: {e}"))
        );
        std::process::exit(1);
    });

    let result = update_incremental(&root, action, &config, &fields, &mut writer, &tree_index, &registry);
    let _ = writer.commit();

    match result {
        Ok(pivotsearch_contracts::UpdateResult::SuccessChanged) => {
            let count = tree_index.count_files(&index_id).unwrap_or(0);
            eprintln!(
                "{}",
                t.s(
                    &format!("✅ indexing complete: {dir} ({count} files)"),
                    &format!("✅ 索引完成: {dir} ({count} 文件)"),
                )
            );
            0
        }
        Ok(pivotsearch_contracts::UpdateResult::SuccessUnchanged) => {
            let count = tree_index.count_files(&index_id).unwrap_or(0);
            eprintln!(
                "{}",
                t.s(
                    &format!("✅ no changes: {dir} ({count} files)"),
                    &format!("✅ 无变化: {dir} ({count} 文件)"),
                )
            );
            0
        }
        Ok(pivotsearch_contracts::UpdateResult::Failure) => {
            eprintln!(
                "{}",
                t.s("⚠️  indexing partially failed", "⚠️  索引部分失败")
            );
            1
        }
        Err(e) => {
            eprintln!(
                "{}",
                t.s(
                    &format!("❌ indexing failed: {e}"),
                    &format!("❌ 索引失败: {e}"),
                )
            );
            1
        }
    }
}

fn cmd_search(
    query: &str,
    json: bool,
    index_filter: &Option<Vec<String>>,
    type_filter: &Option<String>,
    case_sensitive: bool,
    page: usize,
    _limit: usize,
    lang: &Lang,
) -> i32 {
    let t = T(lang);
    let index_dirs = scan_indexes();
    if index_dirs.is_empty() {
        if json {
            print_json_err(
                "NO_INDEX",
                t.s(
                    "no index found; run `psearch index <dir>` first",
                    "没有索引，请先运行 psearch index <dir>",
                ),
            );
        } else {
            eprintln!(
                "{}",
                t.s(
                    "no index found. Run: psearch index <dir>",
                    "没有索引。请先运行: psearch index <dir>",
                )
            );
        }
        return 3;
    }

    let multi = build_multi_searcher(&index_dirs, lang);

    // Type filter matches by extension suffix on the client side (more accurate
    // than mapping extensions to parser names).
    let _parsers = type_filter.as_ref().map(|ext| format!(".{}", ext.to_lowercase()));

    let request = SearchRequest {
        query: query.to_string(),
        index_ids: index_filter.clone(),
        parsers: None, // client-side type filtering below is more accurate
        min_size: None,
        max_size: None,
        page,
        case_sensitive,
    };

    match multi.search(&request) {
        Ok(resp) => {
            // Client-side type filter (by path extension).
            let filtered_results = if let Some(ext) = type_filter {
                let ext_lower = format!(".{}", ext.to_lowercase());
                resp.results
                    .iter()
                    .filter(|r| r.path.to_lowercase().ends_with(&ext_lower))
                    .cloned()
                    .collect::<Vec<_>>()
            } else {
                resp.results
            };

            let total = filtered_results.len();

            if json {
                // JSON payload is language-independent: fixed English keys.
                let data = serde_json::json!({
                    "total_hits": total,
                    "results": filtered_results,
                    "page": resp.page,
                    "page_count": resp.page_count,
                });
                print_json_ok(data);
            } else {
                println!(
                    "{}",
                    t.s(
                        &format!("search \"{query}\" matched {total} results:"),
                        &format!("搜索「{query}」命中 {total} 条结果："),
                    )
                );
                println!();
                for (i, r) in filtered_results.iter().enumerate() {
                    let fname = r.path.rsplit('/').next().unwrap_or(&r.path);
                    println!("{}. {}", i + 1, fname);
                    let path_label = t.s("path", "路径");
                    let type_label = t.s("type", "类型");
                    let size_label = t.s("size (bytes)", "大小: 字节");
                    let snippet_label = t.s("snippet", "片段");
                    println!("   {path_label}: {}", r.path);
                    println!("   {type_label}: {} | {size_label}: {}", r.parser, r.size);
                    let clean = r.snippet.replace("<b>", "").replace("</b>", "");
                    if !clean.is_empty() {
                        println!("   {snippet_label}: {}", clean);
                    }
                    println!();
                }
                if filtered_results.is_empty() {
                    println!("{}", t.s("(no results)", "（无结果）"));
                }
            }
            0
        }
        Err(e) => {
            if json {
                print_json_err("SEARCH_ERROR", &e.to_string());
            } else {
                eprintln!(
                    "{}",
                    t.f(format!("search failed: {e}"), &format!("搜索失败: {e}"))
                );
            }
            1
        }
    }
}

fn cmd_list(json: bool, lang: &Lang) -> i32 {
    let t = T(lang);
    let index_dirs = scan_indexes();
    let mut infos: Vec<serde_json::Value> = Vec::new();
    let mut text_lines: Vec<String> = Vec::new();

    for (_index_id, index_dir) in &index_dirs {
        if let Some((id, path, name, count)) = read_index_info(index_dir) {
            if json {
                infos.push(serde_json::json!({
                    "id": id,
                    "path": path,
                    "name": name,
                    "file_count": count,
                }));
            } else {
                let files_word = t.s("files", "文件");
                text_lines.push(format!(
                    "  {} [{}] ({} {files_word}) — {}",
                    id,
                    name.unwrap_or_default(),
                    count,
                    path
                ));
            }
        }
    }

    if json {
        print_json_ok(serde_json::json!({ "indexes": infos }));
    } else if text_lines.is_empty() {
        println!(
            "{}",
            t.s(
                "no index found. Run `psearch index <dir>` to add one.",
                "没有索引。运行 psearch index <dir> 添加。",
            )
        );
    } else {
        for line in &text_lines {
            println!("{line}");
        }
    }
    0
}

fn cmd_remove(id: &str, lang: &Lang) -> i32 {
    let t = T(lang);
    let index_dir = indexes_dir().join(id);
    if !index_dir.exists() {
        eprintln!(
            "{}",
            t.f(format!("index not found: {id}"), &format!("索引不存在: {id}"))
        );
        return 3;
    }
    let _ = std::fs::remove_dir_all(&index_dir);
    eprintln!(
        "{}",
        t.f(format!("✅ removed: {id}"), &format!("✅ 已删除: {id}"))
    );
    0
}

fn cmd_rebuild(id: &str, lang: &Lang) -> i32 {
    let t = T(lang);
    let index_dirs = scan_indexes();
    let index_dir = match index_dirs.get(id) {
        Some(d) => d.clone(),
        None => {
            eprintln!(
                "{}",
                t.f(format!("index not found: {id}"), &format!("索引不存在: {id}"))
            );
            return 3;
        }
    };

    let tree_path = index_dir.join("tree_index.sqlite");
    let ti = match TreeIndex::open(&tree_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!(
                "{}",
                t.s(
                    &format!("failed to open tree_index: {e}"),
                    &format!("打开 tree_index 失败: {e}"),
                )
            );
            return 1;
        }
    };
    let roots = ti.list_index_roots().unwrap_or_default();
    let root_info = match roots.iter().find(|r| r.id == id) {
        Some(r) => r.clone(),
        None => {
            eprintln!(
                "{}",
                t.s(
                    &format!("index root info not found: {id}"),
                    &format!("索引根信息未找到: {id}"),
                )
            );
            return 3;
        }
    };

    let (schema, fields, _) = build_schema();
    let tantivy_dir = index_dir.join("tantivy");
    let _ = std::fs::remove_dir_all(&tantivy_dir);
    std::fs::create_dir_all(&tantivy_dir).unwrap_or(());
    let index = Index::create_in_dir(&tantivy_dir, schema).unwrap_or_else(|e| {
        eprintln!(
            "{}",
            t.f(format!("failed to create index: {e}"), &format!("创建索引失败: {e}"))
        );
        std::process::exit(1);
    });
    index.tokenizers().register(
        JIEBA_TOKENIZER_NAME,
        tantivy::tokenizer::TextAnalyzer::from(JiebaTokenizer::default()),
    );

    let registry = ParserRegistryImpl::with_builtin_parsers().with_pdf();
    let mut writer = index.writer(50_000_000).unwrap_or_else(|e| {
        eprintln!(
            "{}",
            t.f(format!("failed to create writer: {e}"), &format!("创建 writer 失败: {e}"))
        );
        std::process::exit(1);
    });
    let config = IncrementalConfig {
        index_id: id.to_string(),
        ..Default::default()
    };
    let root = PathBuf::from(&root_info.path);
    let result = update_incremental(&root, IndexAction::Rebuild, &config, &fields, &mut writer, &ti, &registry);
    let _ = writer.commit();

    match result {
        Ok(_) => {
            eprintln!(
                "{}",
                t.s(
                    &format!("✅ rebuild complete: {}", root_info.path),
                    &format!("✅ 重建完成: {}", root_info.path),
                )
            );
            0
        }
        Err(e) => {
            eprintln!(
                "{}",
                t.f(format!("❌ rebuild failed: {e}"), &format!("❌ 重建失败: {e}"))
            );
            1
        }
    }
}

fn cmd_preview(uid: &str, json: bool, lang: &Lang) -> i32 {
    let t = T(lang);
    let path_str = uid.strip_prefix("file://").unwrap_or(uid);
    let path = PathBuf::from(path_str);

    if !path.exists() {
        if json {
            print_json_ok(serde_json::json!({
                "uid": uid, "path": path_str, "exists": false, "content": ""
            }));
        } else {
            println!(
                "{}",
                t.f(format!("file not found: {path_str}"), &format!("文件不存在: {path_str}"))
            );
        }
        return 0;
    }

    let registry = ParserRegistryImpl::with_builtin_parsers().with_pdf();
    match registry.parse(&path) {
        Ok(result) => {
            if json {
                print_json_ok(serde_json::json!({
                    "uid": uid,
                    "path": path_str,
                    "parser": result.parser_name,
                    "exists": true,
                    "content": result.content,
                    "title": result.title,
                }));
            } else {
                let file_label = t.s("file", "文件");
                let parser_label = t.s("parser", "解析器");
                println!("{file_label}: {}", path_str);
                println!("{parser_label}: {}", result.parser_name);
                println!("---");
                println!("{}", result.content);
            }
            0
        }
        Err(e) => {
            if json {
                print_json_err("PARSE_ERROR", &e.to_string());
            } else {
                eprintln!(
                    "{}",
                    t.f(format!("parse failed: {e}"), &format!("解析失败: {e}"))
                );
            }
            1
        }
    }
}

fn cmd_status(lang: &Lang) -> i32 {
    let t = T(lang);
    let d = data_dir();
    let idx_dir = indexes_dir();
    let index_dirs = scan_indexes();

    println!("psearch {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("{}: {}", t.s("data dir", "数据目录"), d.display());
    println!("{}: {}", t.s("index dir", "索引目录"), idx_dir.display());
    println!("{}: {}", t.s("index count", "索引数量"), index_dirs.len());
    println!();

    if index_dirs.is_empty() {
        println!(
            "{}",
            t.s(
                "(no index; run `psearch index <dir>` to add one)",
                "（无索引，运行 psearch index <dir> 添加）",
            )
        );
    } else {
        let files_word = t.s("files", "文件");
        for (_id, index_dir) in &index_dirs {
            if let Some((id, path, name, count)) = read_index_info(index_dir) {
                println!("  {} ({}): {} [{} {files_word}]", id, name.unwrap_or_default(), path, count);
            }
        }
    }
    0
}

// ====================================================================
// Helpers
// ====================================================================

fn hash_path(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ====================================================================
// Entry point
// ====================================================================

fn main() {
    let cli = Cli::parse();
    let lang = Lang::from_code(&cli.lang);

    let code = match cli.command {
        Commands::Index { dir, name, rebuild } => cmd_index(&dir, &name, rebuild, &lang),
        Commands::Search { query, json, index, r#type, case_sensitive, page, limit } => {
            cmd_search(&query, json, &index, &r#type, case_sensitive, page, limit, &lang)
        }
        Commands::List { json } => cmd_list(json, &lang),
        Commands::Remove { id } => cmd_remove(&id, &lang),
        Commands::Rebuild { id } => cmd_rebuild(&id, &lang),
        Commands::Preview { uid, json } => cmd_preview(&uid, json, &lang),
        Commands::Status => cmd_status(&lang),
        Commands::Version => {
            println!("psearch {}", env!("CARGO_PKG_VERSION"));
            0
        }
    };

    std::process::exit(code);
}
