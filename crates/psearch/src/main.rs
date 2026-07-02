//! # psearch CLI
//!
//! pivotsearch 命令行工具，供 AI Agent 调用。
//!
//! 用法：
//!   psearch index <dir> [--name NAME] [--rebuild]
//!   psearch search <query> [--json] [--index ID] [--type EXT] [--case-sensitive] [--page N] [--limit N]
//!   psearch list [--json]
//!   psearch remove <id>
//!   psearch rebuild <id>
//!   psearch preview <uid> [--json]
//!   psearch status
//!   psearch version
//!
//! 数据目录与桌面 app 共享：~/Library/Application Support/com.pivotsearch.app/indexes/

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

// ═══════════════════════════════════════════════════════════════
// CLI 定义
// ═══════════════════════════════════════════════════════════════

#[derive(Parser)]
#[command(name = "psearch", version, about = "pivotsearch CLI — 本地全文搜索，供 Agent 调用")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 添加或更新索引目录
    Index {
        /// 要索引的目录路径
        dir: String,
        /// 索引显示名称
        #[arg(long)]
        name: Option<String>,
        /// 全量重建（而非增量更新）
        #[arg(long)]
        rebuild: bool,
    },
    /// 搜索文件内容
    Search {
        /// 搜索关键词
        query: String,
        /// JSON 输出（Agent 友好）
        #[arg(long)]
        json: bool,
        /// 限定索引根 ID（可多次指定）
        #[arg(long = "index", num_args = 0..)]
        index: Option<Vec<String>>,
        /// 文件类型过滤（如 pdf/docx/md）
        #[arg(long)]
        r#type: Option<String>,
        /// 大小写敏感
        #[arg(long)]
        case_sensitive: bool,
        /// 页码（0-based，默认 0）
        #[arg(long, default_value = "0")]
        page: usize,
        /// 最大结果数（默认 50）
        #[arg(long, default_value = "50")]
        limit: usize,
    },
    /// 列出所有索引根
    List {
        /// JSON 输出
        #[arg(long)]
        json: bool,
    },
    /// 删除索引根
    Remove {
        /// 索引根 ID
        id: String,
    },
    /// 全量重建索引
    Rebuild {
        /// 索引根 ID
        id: String,
    },
    /// 预览文件全文
    Preview {
        /// 文件 UID（file://path）
        uid: String,
        /// JSON 输出
        #[arg(long)]
        json: bool,
    },
    /// 显示状态信息
    Status,
    /// 显示版本
    Version,
}

// ═══════════════════════════════════════════════════════════════
// JSON 输出信封
// ═══════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════
// 数据目录（与桌面 app 共享）
// ═══════════════════════════════════════════════════════════════

const APP_IDENTIFIER: &str = "com.pivotsearch.app";

fn data_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join(APP_IDENTIFIER)
}

fn indexes_dir() -> PathBuf {
    data_dir().join("indexes")
}

/// 扫描 indexes/ 目录，恢复所有已有索引 {index_id → index_dir}
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

/// 从 tree_index.sqlite 读索引根信息
fn read_index_info(index_dir: &Path) -> Option<(String, String, Option<String>, u64)> {
    let tree_path = index_dir.join("tree_index.sqlite");
    let ti = TreeIndex::open(&tree_path).ok()?;
    let roots = ti.list_index_roots().ok()?;
    let root = roots.first()?;
    let count = ti.count_files(&root.id).unwrap_or(0);
    Some((root.id.clone(), root.path.clone(), root.display_name.clone(), count))
}

// ═══════════════════════════════════════════════════════════════
// 辅助：构造 searcher
// ═══════════════════════════════════════════════════════════════

fn build_multi_searcher(index_dirs: &HashMap<String, PathBuf>) -> MultiIndexSearcher {
    let (_schema, fields, tm) = build_schema();
    let mut multi = MultiIndexSearcher::new();

    for (index_id, index_dir) in index_dirs {
        let tantivy_dir = index_dir.join("tantivy");
        let index = match Index::open_in_dir(&tantivy_dir) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("[warn] 打开索引 {} 失败: {}", index_id, e);
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
            Err(e) => eprintln!("[warn] 构造 searcher {} 失败: {}", index_id, e),
        }
    }
    multi
}

// ═══════════════════════════════════════════════════════════════
// 命令实现
// ═══════════════════════════════════════════════════════════════

fn cmd_index(dir: &str, name: &Option<String>, rebuild: bool) -> i32 {
    let root = PathBuf::from(dir);
    if !root.is_dir() {
        eprintln!("错误：不是目录: {dir}");
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
        eprintln!("创建索引目录失败: {e}");
        std::process::exit(1);
    });

    let (schema, fields, _) = build_schema();
    let tantivy_dir = index_dir.join("tantivy");
    std::fs::create_dir_all(&tantivy_dir).unwrap_or(());

    let tantivy_meta = tantivy_dir.join("meta.json");
    let index = if tantivy_meta.exists() {
        Index::open_in_dir(&tantivy_dir).unwrap_or_else(|e| {
            eprintln!("打开索引失败: {e}");
            std::process::exit(1);
        })
    } else {
        Index::create_in_dir(&tantivy_dir, schema).unwrap_or_else(|e| {
            eprintln!("创建索引失败: {e}");
            std::process::exit(1);
        })
    };
    index.tokenizers().register(
        JIEBA_TOKENIZER_NAME,
        tantivy::tokenizer::TextAnalyzer::from(JiebaTokenizer::default()),
    );

    let tree_path = index_dir.join("tree_index.sqlite");
    let tree_index = TreeIndex::open(&tree_path).unwrap_or_else(|e| {
        eprintln!("打开 tree_index 失败: {e}");
        std::process::exit(1);
    });
    tree_index
        .add_index_root(&index_id, dir, display_name.as_deref(), now_millis())
        .unwrap_or_else(|e| {
            eprintln!("记录索引根失败: {e}");
            std::process::exit(1);
        });

    let registry = ParserRegistryImpl::with_builtin_parsers().with_pdf();
    let action = if rebuild { IndexAction::Rebuild } else { IndexAction::Update };
    let config = IncrementalConfig {
        index_id: index_id.clone(),
        ..Default::default()
    };

    let mut writer = index.writer(50_000_000).unwrap_or_else(|e| {
        eprintln!("创建 writer 失败: {e}");
        std::process::exit(1);
    });

    let result = update_incremental(&root, action, &config, &fields, &mut writer, &tree_index, &registry);
    let _ = writer.commit();

    match result {
        Ok(pivotsearch_contracts::UpdateResult::SuccessChanged) => {
            let count = tree_index.count_files(&index_id).unwrap_or(0);
            eprintln!("✅ 索引完成: {dir} ({count} 文件)");
            0
        }
        Ok(pivotsearch_contracts::UpdateResult::SuccessUnchanged) => {
            let count = tree_index.count_files(&index_id).unwrap_or(0);
            eprintln!("✅ 无变化: {dir} ({count} 文件)");
            0
        }
        Ok(pivotsearch_contracts::UpdateResult::Failure) => {
            eprintln!("⚠️  索引部分失败");
            1
        }
        Err(e) => {
            eprintln!("❌ 索引失败: {e}");
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
) -> i32 {
    let index_dirs = scan_indexes();
    if index_dirs.is_empty() {
        if json {
            print_json_err("NO_INDEX", "没有索引，请先运行 psearch index <dir>");
        } else {
            eprintln!("没有索引。请先运行: psearch index <dir>");
        }
        return 3;
    }

    let multi = build_multi_searcher(&index_dirs);

    let parsers = type_filter.as_ref().map(|ext| {
        let ext_with_dot = format!(".{}", ext.to_lowercase());
        // type 过滤是通过扩展名后缀匹配的，这里转成 parser 名列表不精确
        // 更好的方式是 SimpleSearcher 支持 type 过滤，但当前通过客户端过滤
        vec![ext_with_dot]
    });

    let request = SearchRequest {
        query: query.to_string(),
        index_ids: index_filter.clone(),
        parsers: None, // parser 过滤不精确，用 type 后端过滤更准确
        min_size: None,
        max_size: None,
        page,
        case_sensitive,
    };

    match multi.search(&request) {
        Ok(resp) => {
            // type 过滤（客户端，按路径扩展名）
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
                let data = serde_json::json!({
                    "total_hits": total,
                    "results": filtered_results,
                    "page": resp.page,
                    "page_count": resp.page_count,
                });
                print_json_ok(data);
            } else {
                println!("搜索「{query}」命中 {total} 条结果：");
                println!();
                for (i, r) in filtered_results.iter().enumerate() {
                    let fname = r.path.rsplit('/').next().unwrap_or(&r.path);
                    println!("{}. {}", i + 1, fname);
                    println!("   路径: {}", r.path);
                    println!("   类型: {} | 大小: {} 字节", r.parser, r.size);
                    let clean = r.snippet.replace("<b>", "").replace("</b>", "");
                    if !clean.is_empty() {
                        println!("   片段: {}", clean);
                    }
                    println!();
                }
                if filtered_results.is_empty() {
                    println!("（无结果）");
                }
            }
            let _ = parsers; // 避免 unused warning
            0
        }
        Err(e) => {
            if json {
                print_json_err("SEARCH_ERROR", &e.to_string());
            } else {
                eprintln!("搜索失败: {e}");
            }
            1
        }
    }
}

fn cmd_list(json: bool) -> i32 {
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
                text_lines.push(format!("  {} [{}] ({} 文件) — {}", id, name.unwrap_or_default(), count, path));
            }
        }
    }

    if json {
        print_json_ok(serde_json::json!({ "indexes": infos }));
    } else if text_lines.is_empty() {
        println!("没有索引。运行 psearch index <dir> 添加。");
    } else {
        for line in &text_lines {
            println!("{line}");
        }
    }
    0
}

fn cmd_remove(id: &str) -> i32 {
    let index_dir = indexes_dir().join(id);
    if !index_dir.exists() {
        eprintln!("索引不存在: {id}");
        return 3;
    }
    let _ = std::fs::remove_dir_all(&index_dir);
    eprintln!("✅ 已删除: {id}");
    0
}

fn cmd_rebuild(id: &str) -> i32 {
    let index_dirs = scan_indexes();
    let index_dir = match index_dirs.get(id) {
        Some(d) => d.clone(),
        None => {
            eprintln!("索引不存在: {id}");
            return 3;
        }
    };

    let tree_path = index_dir.join("tree_index.sqlite");
    let ti = match TreeIndex::open(&tree_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("打开 tree_index 失败: {e}");
            return 1;
        }
    };
    let roots = ti.list_index_roots().unwrap_or_default();
    let root_info = match roots.iter().find(|r| r.id == id) {
        Some(r) => r.clone(),
        None => {
            eprintln!("索引根信息未找到: {id}");
            return 3;
        }
    };

    let (schema, fields, _) = build_schema();
    let tantivy_dir = index_dir.join("tantivy");
    let _ = std::fs::remove_dir_all(&tantivy_dir);
    std::fs::create_dir_all(&tantivy_dir).unwrap_or(());
    let index = Index::create_in_dir(&tantivy_dir, schema).unwrap_or_else(|e| {
        eprintln!("创建索引失败: {e}");
        std::process::exit(1);
    });
    index.tokenizers().register(
        JIEBA_TOKENIZER_NAME,
        tantivy::tokenizer::TextAnalyzer::from(JiebaTokenizer::default()),
    );

    let registry = ParserRegistryImpl::with_builtin_parsers().with_pdf();
    let mut writer = index.writer(50_000_000).unwrap_or_else(|e| {
        eprintln!("创建 writer 失败: {e}");
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
            eprintln!("✅ 重建完成: {}", root_info.path);
            0
        }
        Err(e) => {
            eprintln!("❌ 重建失败: {e}");
            1
        }
    }
}

fn cmd_preview(uid: &str, json: bool) -> i32 {
    let path_str = uid.strip_prefix("file://").unwrap_or(uid);
    let path = PathBuf::from(path_str);

    if !path.exists() {
        if json {
            print_json_ok(serde_json::json!({
                "uid": uid, "path": path_str, "exists": false, "content": ""
            }));
        } else {
            println!("文件不存在: {path_str}");
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
                println!("文件: {}", path_str);
                println!("解析器: {}", result.parser_name);
                println!("---");
                println!("{}", result.content);
            }
            0
        }
        Err(e) => {
            if json {
                print_json_err("PARSE_ERROR", &e.to_string());
            } else {
                eprintln!("解析失败: {e}");
            }
            1
        }
    }
}

fn cmd_status() -> i32 {
    let d = data_dir();
    let idx_dir = indexes_dir();
    let index_dirs = scan_indexes();

    println!("psearch {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("数据目录: {}", d.display());
    println!("索引目录: {}", idx_dir.display());
    println!("索引数量: {}", index_dirs.len());
    println!();

    if index_dirs.is_empty() {
        println!("（无索引，运行 psearch index <dir> 添加）");
    } else {
        for (_id, index_dir) in &index_dirs {
            if let Some((id, path, name, count)) = read_index_info(index_dir) {
                println!("  {} ({}): {} [{} 文件]", id, name.unwrap_or_default(), path, count);
            }
        }
    }
    0
}

// ═══════════════════════════════════════════════════════════════
// 辅助函数
// ═══════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════
// 入口
// ═══════════════════════════════════════════════════════════════

fn main() {
    let cli = Cli::parse();

    let code = match cli.command {
        Commands::Index { dir, name, rebuild } => cmd_index(&dir, &name, rebuild),
        Commands::Search { query, json, index, r#type, case_sensitive, page, limit } => {
            cmd_search(&query, json, &index, &r#type, case_sensitive, page, limit)
        }
        Commands::List { json } => cmd_list(json),
        Commands::Remove { id } => cmd_remove(&id),
        Commands::Rebuild { id } => cmd_rebuild(&id),
        Commands::Preview { uid, json } => cmd_preview(&uid, json),
        Commands::Status => cmd_status(),
        Commands::Version => {
            println!("psearch {}", env!("CARGO_PKG_VERSION"));
            0
        }
    };

    std::process::exit(code);
}
