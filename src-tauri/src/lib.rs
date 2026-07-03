//! pivotsearch Tauri 后端：#[tauri::command] 桥接核心引擎给前端。
//!
//! 这是组装根——可以 import parser/index/search 的具体实现。

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

// ── 命令的参数/返回类型（前端 TS 对齐）──

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
    phase: String, // "indexing" / "done"
}

// ── 命令实现 ──

/// 添加一个索引根（后台线程索引）。
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

    // 生成 index_id（路径的 hash）
    let index_id = format!("idx-{:x}", md5_hash(&path));
    let display_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string());

    // 在数据目录下创建索引存储
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取数据目录失败: {e}"))?;
    let index_dir = data_dir.join("indexes").join(&index_id);
    std::fs::create_dir_all(&index_dir).map_err(|e| format!("创建索引目录失败: {e}"))?;

    // 构造 schema + index + tree_index（全部移入后台线程）
    let (schema, fields, _tokenizer_manager) = build_schema();
    let tantivy_dir = index_dir.join("tantivy");
    std::fs::create_dir_all(&tantivy_dir).map_err(|e| e.to_string())?;

    // open-or-create：如果索引已存在（重启后再添加同一路径），open 而非 create
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
    // add_index_root 用 INSERT OR IGNORE，重复添加同一路径不会报错
    tree_index
        .add_index_root(&index_id, &path, display_name.as_deref(), now_millis())
        .map_err(|e| e.to_string())?;

    // 记录 index_dir 到 state（search 时重新 open）
    {
        let mut s = state.lock();
        s.index_dirs.insert(index_id.clone(), index_dir.clone());
    }

    // 后台线程执行索引（fields/index/tree_index 全部移入，不阻塞命令返回）
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
                    },
                );
                return;
            }
        };
        let app_for_progress = app_clone.clone();
        let id_for_progress = index_id_clone.clone();
        let mut progress_cb = move |processed: usize, total: usize| {
            let pct = if total > 0 { processed * 100 / total } else { 0 };
            let _ = app_for_progress.emit(
                "index-progress",
                IndexProgress {
                    index_id: id_for_progress.clone(),
                    processed,
                    total,
                    message: format!("正在索引... {}% ({}{})", pct, processed, if total > 0 { format!("/{}", total) } else { String::new() }),
                    phase: "indexing".to_string(),
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
                index_id: index_id_clone,
                processed: 0,
                total: 0,
                message: "索引完成".to_string(),
                phase: "done".to_string(),
            },
        );
    });

    Ok(index_id)
}

/// 执行搜索：从 state 读所有 index_dirs，重新 open 每个 Tantivy Index，合并查询。
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

    // 为每个索引构造 SimpleSearcher，加入 MultiIndexSearcher
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

/// 获取预览数据：从 uid 反推 path，重新解析原文件返回完整 content。
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

    // 用 parser 注册表重新解析原文件
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

/// 列出所有索引根：从 state.index_dirs 读，对每个打开 tree_index 取信息。
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
                // tree_index 未就绪（索引进行中），显示基本信息
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

/// 移除索引根：删 Tantivy 目录 + tree_index + 从 state 移除。
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
        // 删 tree_index 记录
        let tree_path = dir.join("tree_index.sqlite");
        if let Ok(ti) = TreeIndex::open(&tree_path) {
            let _ = ti.remove_index_root(&id);
        }
        // 删整个索引目录
        let _ = std::fs::remove_dir_all(&dir);
    }
    Ok(())
}

/// 重建索引：清空后重新全量索引（后台线程）。
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
                let mut progress_cb = move |processed: usize, total: usize| {
                    let pct = if total > 0 { processed * 100 / total } else { 0 };
                    let _ = app_for_progress.emit(
                        "index-progress",
                        IndexProgress {
                            index_id: id_for_progress.clone(),
                            processed,
                            total,
                            message: format!("正在重建... {}% ({}{})", pct, processed, if total > 0 { format!("/{}", total) } else { String::new() }),
                            phase: "indexing".to_string(),
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
                        index_id: id_clone,
                        processed: 0,
                        total: 0,
                        message: "重建完成".to_string(),
                        phase: "done".to_string(),
                    },
                );
            });
        }
    }
    Ok(())
}

/// 复制文本到系统剪贴板。
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

/// 在系统文件管理器中打开文件所在目录（高亮该文件）。
#[tauri::command]
async fn open_in_folder(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err("文件不存在".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        // macOS: open -R <path> 在 Finder 中高亮文件
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
        // Linux: 打开文件所在目录
        let dir = p.parent().unwrap_or(p).to_string_lossy().to_string();
        std::process::Command::new("xdg-open")
            .arg(&dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 安装 psearch CLI 到系统 PATH（创建符号链接 /usr/local/bin/psearch → app 内的 sidecar）。
#[tauri::command]
async fn install_cli(app: tauri::AppHandle) -> Result<String, String> {
    // 找到 app bundle 内的 psearch sidecar 路径
    let exe_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("获取资源目录失败: {e}"))?;
    // sidecar 在 Contents/MacOS/psearch (macOS)
    let psearch_path = exe_dir.join("../../../MacOS/psearch");

    // 实际路径（规范化的）
    let psearch_real = if psearch_path.exists() {
        psearch_path
    } else {
        // fallback：直接用当前可执行文件同目录
        std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .ok_or("无法定位可执行文件目录")?
            .join("psearch")
    };

    if !psearch_real.exists() {
        return Err("psearch CLI 未在 app bundle 中找到".to_string());
    }

    // 创建符号链接 /usr/local/bin/psearch → app 内 psearch
    #[cfg(target_os = "macos")]
    {
        let link = std::path::PathBuf::from("/usr/local/bin/psearch");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&psearch_real, &link).map_err(|e| {
            format!("创建符号链接失败: {e}\n请手动执行: sudo ln -sf {} {}", psearch_real.display(), link.display())
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
        Err("Windows 请手动将 psearch.exe 添加到 PATH".to_string())
    }
}

/// 获取索引详细信息（双击索引行查看）。
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

// ── 辅助函数 ──

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

// ── Tauri 应用入口 ──

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::new(Mutex::new(EngineState::new())))
        .setup(|app| {
            // 启动时从磁盘恢复已有索引到 state
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
