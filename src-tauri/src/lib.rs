//! pivotsearch Tauri backend: bridges the core engine to the frontend via #[tauri::command].
//!
//! This is the assembly root — it can import concrete parser/index/search implementations.

mod state;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};

use pivotsearch_contracts::{IndexAction, ParserRegistry, SearchRequest};
use pivotsearch_index::{build_schema, incremental::*, tree_index::*};
use pivotsearch_parser::ParserRegistryImpl;
use pivotsearch_search::{MultiIndexSearcher, SearchSchemaFields, SimpleSearcher};
use tantivy::Index;

pub use state::EngineState;

// ── Command parameter / return types (aligned with the frontend TS) ──

#[derive(Serialize)]
pub struct IndexInfo {
    id: String,
    path: String,
    display_name: Option<String>,
    file_count: u64,
}

#[derive(Deserialize)]
pub struct SearchFilters {
    index_ids: Option<Vec<String>>,
    parsers: Option<Vec<String>>,
    min_size: Option<i64>,
    max_size: Option<i64>,
}

#[derive(Serialize, Clone)]
pub struct IndexProgress {
    index_id: String,
    processed: usize,
    total: usize,
    message: String,
    phase: String,  // "indexing" / "done" / "error"
    name: String,   // index display name (e.g. "Documents")
}

// ── Command implementations ──

/// Add an index root (indexed in a background thread).
#[tauri::command]
async fn add_index(
    path: String,
    app: tauri::AppHandle,
    state: State<'_, Arc<Mutex<EngineState>>>,
) -> Result<String, String> {
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err(format!("不是目录: {path}"));
    }

    // Generate the index_id (hash of the path)
    let index_id = format!("idx-{:x}", md5_hash(&path));
    let display_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string());

    // Create index storage under the data directory
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取数据目录失败: {e}"))?;
    let index_dir = data_dir.join("indexes").join(&index_id);
    std::fs::create_dir_all(&index_dir).map_err(|e| format!("创建索引目录失败: {e}"))?;

    // Build schema + index + tree_index (all moved into the background thread)
    let (schema, fields, _tokenizer_manager) = build_schema();
    let tantivy_dir = index_dir.join("tantivy");
    std::fs::create_dir_all(&tantivy_dir).map_err(|e| e.to_string())?;

    // open-or-create: if the index already exists (same path re-added after a restart),
    // open it instead of creating it.
    let tantivy_meta = tantivy_dir.join("meta.json");
    let index = if tantivy_meta.exists() {
        Index::open_in_dir(&tantivy_dir).map_err(|e| e.to_string())?
    } else {
        Index::create_in_dir(&tantivy_dir, schema).map_err(|e| e.to_string())?
    };
    index.tokenizers().register(
        pivotsearch_index::JIEBA_TOKENIZER_NAME,
        tantivy::tokenizer::TextAnalyzer::from(pivotsearch_index::tokenizer::JiebaTokenizer::default()),
    );

    let tree_index_path = index_dir.join("tree_index.sqlite");
    let tree_index = TreeIndex::open(&tree_index_path).map_err(|e| e.to_string())?;
    // add_index_root uses INSERT OR IGNORE, so re-adding the same path does not error.
    tree_index
        .add_index_root(&index_id, &path, display_name.as_deref(), now_millis())
        .map_err(|e| e.to_string())?;

    // Record the index_dir in state (reopened at search time)
    {
        let mut s = state.lock();
        s.index_dirs.insert(index_id.clone(), index_dir.clone());
    }

    // Run indexing in a background thread (fields/index/tree_index all moved in,
    // so the command return is not blocked)
    let app_clone = app.clone();
    let index_id_clone = index_id.clone();
    let config = IncrementalConfig {
        index_id: index_id.clone(),
        ..Default::default()
    };
    std::thread::spawn(move || {
        let parser_registry = ParserRegistryImpl::with_builtin_parsers();
        let mut writer = match index.writer(50_000_000) {
            Ok(w) => w,
            Err(e) => {
                let _ = app_clone.emit(
                    "index-progress",
                    IndexProgress {
                        index_id: index_id_clone.clone(),
                        processed: 0,
                        total: 0,
                        message: format!("索引失败: {e}"),
                        phase: "error".to_string(),
                        name: display_name.clone().unwrap_or_else(|| path.clone()),
                    },
                );
                return;
            }
        };
        let app_for_progress = app_clone.clone();
        let id_for_progress = index_id_clone.clone();
        let name_for_progress = display_name.clone().unwrap_or_else(|| path.clone());
        let mut progress_cb = move |processed: usize, total: usize| {
            let pct = if total > 0 { processed * 100 / total } else { 0 };
            let _ = app_for_progress.emit(
                "index-progress",
                IndexProgress {
                    index_id: id_for_progress.clone(),
                    processed,
                    total,
                    message: format!("[{}] 正在索引... {}% ({}{})", name_for_progress, pct, processed, if total > 0 { format!("/{}", total) } else { String::new() }),
                    phase: "indexing".to_string(),
                    name: name_for_progress.clone(),
                },
            );
        };
        let _ = update_incremental_with_progress(
            &root,
            IndexAction::Update,
            &config,
            &fields,
            &mut writer,
            &tree_index,
            &parser_registry,
            Some(&mut progress_cb),
        );
        let _ = writer.commit();

        let _ = app_clone.emit(
            "index-progress",
            IndexProgress {
                index_id: index_id_clone.clone(),
                processed: 0,
                total: 0,
                message: format!("[{}] 索引完成", display_name.as_deref().unwrap_or(&path)),
                phase: "done".to_string(),
                name: display_name.clone().unwrap_or_else(|| path.clone()),
            },
        );
    });

    Ok(index_id)
}

/// Run a search: read all index_dirs from state, reopen each Tantivy Index,
/// and merge the query results.
#[tauri::command]
async fn search(
    query: String,
    filters: Option<SearchFilters>,
    page: usize,
    case_sensitive: Option<bool>,
    state: State<'_, Arc<Mutex<EngineState>>>,
) -> Result<pivotsearch_contracts::SearchResponse, String> {
    let index_dirs = {
        let s = state.lock();
        s.index_dirs.clone()
    };

    if index_dirs.is_empty() {
        return Ok(pivotsearch_contracts::SearchResponse {
            total_hits: 0,
            results: vec![],
            page,
            page_count: 1,
        });
    }

    // Build a SimpleSearcher for each index and add it to the MultiIndexSearcher
    let mut multi = MultiIndexSearcher::new();
    let (_schema, fields, tokenizer_manager) = build_schema();

    for (index_id, index_dir) in &index_dirs {
        let tantivy_dir = index_dir.join("tantivy");
        let index = match Index::open_in_dir(&tantivy_dir) {
            Ok(i) => i,
            Err(e) => {
                tracing::warn!("打开索引 {} 失败，跳过: {}", index_id, e);
                continue;
            }
        };
        index.tokenizers().register(
            pivotsearch_index::JIEBA_TOKENIZER_NAME,
            tantivy::tokenizer::TextAnalyzer::from(
                pivotsearch_index::tokenizer::JiebaTokenizer::default(),
            ),
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
        match SimpleSearcher::new(index, search_fields, tokenizer_manager.clone()) {
            Ok(s) => multi.add(index_id.clone(), s),
            Err(e) => tracing::warn!("构造 searcher {} 失败: {}", index_id, e),
        }
    }

    let (index_ids, parsers, min_size, max_size) = match filters {
        Some(f) => (f.index_ids, f.parsers, f.min_size, f.max_size),
        None => (None, None, None, None),
    };
    let request = SearchRequest {
        query,
        index_ids,
        parsers,
        min_size,
        max_size,
        page,
        case_sensitive: case_sensitive.unwrap_or(false),
    };
    multi.search(&request).map_err(|e| e.to_string())
}

/// Fetch preview data: derive the path from the uid, re-parse the original file,
/// and return the full content.
#[tauri::command]
async fn get_preview(uid: String) -> Result<serde_json::Value, String> {
    let path_str = uid.strip_prefix("file://").unwrap_or(&uid);
    let path = PathBuf::from(path_str);

    if !path.exists() {
        return Ok(serde_json::json!({
            "uid": uid,
            "path": path_str,
            "parser": "",
            "content": "",
            "exists": false,
        }));
    }

    // Re-parse the original file using the parser registry
    let registry = ParserRegistryImpl::with_builtin_parsers().with_pdf();
    match registry.parse(&path) {
        Ok(result) => Ok(serde_json::json!({
            "uid": uid,
            "path": path_str,
            "parser": result.parser_name,
            "content": result.content,
            "exists": true,
        })),
        Err(e) => Ok(serde_json::json!({
            "uid": uid,
            "path": path_str,
            "parser": "",
            "content": format!("（无法解析：{e}）"),
            "exists": true,
        })),
    }
}

/// List all index roots: read state.index_dirs, then open the tree_index for each
/// to gather their info.
#[tauri::command]
async fn list_indexes(
    state: State<'_, Arc<Mutex<EngineState>>>,
) -> Result<Vec<IndexInfo>, String> {
    let index_dirs = {
        let s = state.lock();
        s.index_dirs.clone()
    };

    let mut infos = Vec::new();
    for (index_id, index_dir) in &index_dirs {
        let tree_path = index_dir.join("tree_index.sqlite");
        match TreeIndex::open(&tree_path) {
            Ok(ti) => {
                let roots = ti.list_index_roots().unwrap_or_default();
                for root in roots {
                    let file_count = ti.count_files(&root.id).unwrap_or(0);
                    infos.push(IndexInfo {
                        id: root.id,
                        path: root.path,
                        display_name: root.display_name,
                        file_count,
                    });
                }
            }
            Err(_) => {
                // tree_index not ready (indexing in progress); show basic info
                infos.push(IndexInfo {
                    id: index_id.clone(),
                    path: index_dir.display().to_string(),
                    display_name: None,
                    file_count: 0,
                });
            }
        }
    }
    Ok(infos)
}

/// Remove an index root: delete the Tantivy directory + tree_index, and remove it
/// from state.
#[tauri::command]
async fn remove_index(
    id: String,
    state: State<'_, Arc<Mutex<EngineState>>>,
) -> Result<(), String> {
    let index_dir = {
        let mut s = state.lock();
        s.index_dirs.remove(&id)
    };
    if let Some(dir) = index_dir {
        // Remove the tree_index record
        let tree_path = dir.join("tree_index.sqlite");
        if let Ok(ti) = TreeIndex::open(&tree_path) {
            let _ = ti.remove_index_root(&id);
        }
        // Delete the entire index directory
        let _ = std::fs::remove_dir_all(&dir);
    }
    Ok(())
}

/// Rebuild an index: clear it and re-run a full index pass (in a background thread).
#[tauri::command]
async fn rebuild_index(
    id: String,
    app: tauri::AppHandle,
    state: State<'_, Arc<Mutex<EngineState>>>,
) -> Result<(), String> {
    let index_dir = {
        let s = state.lock();
        s.index_dirs.get(&id).cloned()
    };

    if let Some(dir) = index_dir {
        let tree_path = dir.join("tree_index.sqlite");
        let ti = TreeIndex::open(&tree_path).map_err(|e| e.to_string())?;
        let roots = ti.list_index_roots().map_err(|e| e.to_string())?;
        if let Some(root_info) = roots.iter().find(|r| r.id == id).cloned() {
            let root = PathBuf::from(&root_info.path);
            let app_clone = app.clone();
            let id_clone = id.clone();
            let name_clone = root_info.display_name.clone().unwrap_or_else(|| root_info.path.clone());
            std::thread::spawn(move || {
                let (_schema, fields, _) = build_schema();
                let tantivy_dir = dir.join("tantivy");
                let _ = std::fs::remove_dir_all(&tantivy_dir);
                std::fs::create_dir_all(&tantivy_dir).ok();
                let index = Index::create_in_dir(&tantivy_dir, _schema).unwrap();
                index.tokenizers().register(
                    pivotsearch_index::JIEBA_TOKENIZER_NAME,
                    tantivy::tokenizer::TextAnalyzer::from(
                        pivotsearch_index::tokenizer::JiebaTokenizer::default(),
                    ),
                );
                let parser_registry = ParserRegistryImpl::with_builtin_parsers();
                let mut writer = index.writer(50_000_000).unwrap();
                let config = IncrementalConfig {
                    index_id: id_clone.clone(),
                    ..Default::default()
                };
                let app_for_progress = app_clone.clone();
                let id_for_progress = id_clone.clone();
                let name_for_rebuild = name_clone.clone();
                let mut progress_cb = move |processed: usize, total: usize| {
                    let pct = if total > 0 { processed * 100 / total } else { 0 };
                    let _ = app_for_progress.emit(
                        "index-progress",
                        IndexProgress {
                            index_id: id_for_progress.clone(),
                            processed,
                            total,
                            message: format!("[{}] 正在重建... {}% ({}{})", name_for_rebuild, pct, processed, if total > 0 { format!("/{}", total) } else { String::new() }),
                            phase: "indexing".to_string(),
                            name: name_for_rebuild.clone(),
                        },
                    );
                };
                let _ = update_incremental_with_progress(
                    &root,
                    IndexAction::Rebuild,
                    &config,
                    &fields,
                    &mut writer,
                    &ti,
                    &parser_registry,
                    Some(&mut progress_cb),
                );
                let _ = writer.commit();
                let _ = app_clone.emit(
                    "index-progress",
                    IndexProgress {
                        index_id: id_clone.clone(),
                        processed: 0,
                        total: 0,
                        message: format!("[{}] 重建完成", name_clone),
                        phase: "done".to_string(),
                        name: name_clone,
                    },
                );
            });
        }
    }
    Ok(())
}

/// Copy text to the system clipboard.
#[tauri::command]
async fn copy_to_clipboard(
    text: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard()
        .write_text(&text)
        .map_err(|e| e.to_string())
}

/// Open the file's containing folder in the system file manager (highlighting the file).
#[tauri::command]
async fn open_in_folder(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err("文件不存在".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        // macOS: `open -R <path>` highlights the file in Finder
        std::process::Command::new("open")
            .args(["-R", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        // Windows: explorer /select,<path>
        std::process::Command::new("explorer")
            .args(["/select,", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        // Linux: open the file's containing directory
        let dir = p.parent().unwrap_or(p).to_string_lossy().to_string();
        std::process::Command::new("xdg-open")
            .arg(&dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Install the psearch CLI onto the system PATH.
#[tauri::command]
async fn install_cli(app: tauri::AppHandle) -> Result<String, String> {
    // Generic approach: locate the sidecar via current_exe (the sidecar ships
    // in the same directory as the main program)
    let exe_path = std::env::current_exe().map_err(|e| format!("无法定位可执行文件: {e}"))?;
    let exe_dir = exe_path.parent().ok_or("无法获取可执行文件目录")?;

    // Try several possible sidecar filenames
    let candidates: Vec<std::path::PathBuf> = vec![
        exe_dir.join("psearch"),
        exe_dir.join("psearch.exe"),
        // psearch inside an macOS .app bundle at Contents/MacOS/psearch
        exe_dir.join("../../../MacOS/psearch"),
    ];

    let psearch_real = candidates.iter()
        .find(|p| p.exists())
        .map(|p| p.canonicalize().unwrap_or_else(|_| p.clone()))
        .ok_or_else(|| format!(
            "psearch CLI 未找到。已检查: {}\n请确认 app 是从完整安装包安装的（非绿色版）。",
            candidates.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
        ))?;

    #[cfg(target_os = "macos")]
    {
        let link = std::path::PathBuf::from("/usr/local/bin/psearch");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&psearch_real, &link).map_err(|e| {
            format!("创建符号链接失败: {e}\n请手动执行: sudo ln -sf \"{}\" \"{}\"", psearch_real.display(), link.display())
        })?;
        Ok(format!("✅ psearch 已安装到 {}\n终端可直接运行: psearch search \"关键词\" --json", link.display()))
    }

    #[cfg(target_os = "linux")]
    {
        let link = std::path::PathBuf::from("/usr/local/bin/psearch");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&psearch_real, &link).map_err(|e| e.to_string())?;
        Ok(format!("✅ psearch 已安装到 {}", link.display()))
    }

    #[cfg(target_os = "windows")]
    {
        // Windows: copy psearch.exe to the user directory and add it to PATH
        let home = dirs::home_dir().ok_or("无法获取用户目录")?;
        let bin_dir = home.join(".psearch");
        std::fs::create_dir_all(&bin_dir).map_err(|e| e.to_string())?;
        let target = bin_dir.join("psearch.exe");
        std::fs::copy(&psearch_real, &target).map_err(|e| format!("复制文件失败: {e}"))?;

        // Add the directory to the user PATH via setx (setx has a 1024-character limit)
        let current_path = std::env::var("PATH").unwrap_or_default();
        let bin_dir_str = bin_dir.to_string_lossy().to_string();
        if !current_path.contains(&bin_dir_str) {
            std::process::Command::new("setx")
                .args(["PATH", &format!("{};{}", current_path, bin_dir_str)])
                .output()
                .map_err(|e| format!("加入 PATH 失败: {e}"))?;
        }
        Ok(format!("✅ psearch.exe 已复制到 {}\n请重新打开终端后运行: psearch search \"关键词\" --json", target.display()))
    }
}

/// Fetch index details (shown when double-clicking an index row).
#[tauri::command]
async fn index_details(
    id: String,
    state: State<'_, Arc<Mutex<EngineState>>>,
) -> Result<serde_json::Value, String> {
    let index_dir = {
        let s = state.lock();
        s.index_dirs.get(&id).cloned()
    };

    let index_dir = match index_dir {
        Some(d) => d,
        None => return Err("索引不存在".to_string()),
    };

    let tree_path = index_dir.join("tree_index.sqlite");
    let ti = TreeIndex::open(&tree_path).map_err(|e| e.to_string())?;

    let roots = ti.list_index_roots().map_err(|e| e.to_string())?;
    let root_info = roots
        .iter()
        .find(|r| r.id == id)
        .ok_or("索引根信息未找到")?
        .clone();

    let file_count = ti.count_files(&id).unwrap_or(0);
    let parser_stats = ti.stats_by_parser(&id).unwrap_or_default();
    let recent = ti.recent_files(&id, 20).unwrap_or_default();

    let parser_stats_json: Vec<serde_json::Value> = parser_stats
        .iter()
        .map(|(name, count)| {
            serde_json::json!({
                "parser": name,
                "count": count,
            })
        })
        .collect();

    let recent_json: Vec<serde_json::Value> = recent
        .iter()
        .map(|f| {
            serde_json::json!({
                "path": f.path,
                "mtime": f.mtime,
                "parser": f.parser,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "id": root_info.id,
        "path": root_info.path,
        "name": root_info.display_name,
        "created_at": root_info.created_at,
        "file_count": file_count,
        "parser_stats": parser_stats_json,
        "recent_files": recent_json,
    }))
}

// ── Helper functions ──

fn md5_hash(s: &str) -> u64 {
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

// ── Tauri application entry point ──

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::new(Mutex::new(EngineState::new())))
        .setup(|app| {
            // At startup, restore existing indexes from disk into state
            let state = app.state::<Arc<Mutex<EngineState>>>();
            if let Ok(data_dir) = app.path().app_data_dir() {
                let indexes_dir = data_dir.join("indexes");
                if indexes_dir.exists() {
                    let mut s = state.lock();
                    for entry in std::fs::read_dir(&indexes_dir).into_iter().flatten() {
                        if let Ok(e) = entry {
                            let dir_name = e.file_name().to_string_lossy().to_string();
                            if dir_name.starts_with("idx-") {
                                s.index_dirs.insert(dir_name, e.path());
                            }
                        }
                    }
                    tracing::info!("恢复 {} 个已有索引", s.index_dirs.len());
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            add_index,
            search,
            get_preview,
            list_indexes,
            remove_index,
            rebuild_index,
            copy_to_clipboard,
            open_in_folder,
            install_cli,
            index_details,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
