//! # pivotsearch CLI
//!
//! Development-time debugging CLI. Phase 1 implements the minimal index/search loop.

use std::path::PathBuf;

use pivotsearch_contracts::{ParserRegistry, SearchRequest};
use pivotsearch_index::{build_document, build_schema, compute_uid};
use pivotsearch_parser::ParserRegistryImpl;
use pivotsearch_search::{SearchSchemaFields, SimpleSearcher};
use tantivy::Index;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_help();
        return Ok(());
    }

    match args[1].as_str() {
        "index" => {
            if args.len() < 3 {
                eprintln!("用法: pivotsearch index <dir> [index_path]");
                std::process::exit(1);
            }
            let dir = &args[2];
            let index_path = args
                .get(3)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".pivotsearch-index"));
            cmd_index(dir, &index_path)?;
        }
        "search" => {
            if args.len() < 3 {
                eprintln!("用法: pivotsearch search <query> [index_path]");
                std::process::exit(1);
            }
            let query = &args[2];
            let index_path = args
                .get(3)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".pivotsearch-index"));
            cmd_search(query, &index_path)?;
        }
        "help" | "--help" | "-h" => print_help(),
        _ => {
            eprintln!("未知命令: {}", args[1]);
            print_help();
            std::process::exit(1);
        }
    }
    Ok(())
}

fn cmd_index(dir: &str, index_path: &PathBuf) -> anyhow::Result<()> {
    let root = PathBuf::from(dir);
    if !root.is_dir() {
        anyhow::bail!("不是目录: {dir}");
    }

    println!("索引目录: {dir}");
    println!("索引存储: {}", index_path.display());

    // Build schema + index
    let (schema, fields, tokenizer_manager) = build_schema();
    std::fs::create_dir_all(index_path)?;
    let index = Index::create_in_dir(index_path, schema.clone())?;
    index.tokenizers().register(
        pivotsearch_index::schema::JIEBA_TOKENIZER_NAME,
        tantivy::tokenizer::TextAnalyzer::from(
            pivotsearch_index::tokenizer::JiebaTokenizer::default(),
        ),
    );

    // Convert field handles (the index crate's SchemaFields → search's SearchSchemaFields)
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

    let mut writer = index.writer(50_000_000)?; // 50MB

    // Build the parser registry (includes PDF if PDFium is available)
    // Enable the PDF parser (requires the PDFium library; if the library is missing, PdfParser.parse returns an error without blocking)
    let registry = ParserRegistryImpl::with_builtin_parsers().with_pdf();

    // Walk the directory
    let mut total = 0usize;
    let mut skipped = 0usize;
    let mut errors = 0usize;

    for entry in walkdir::WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();

        // Skip lock files and hidden files (avoid indexing Tantivy's own index files)
        if file_name.ends_with(".lock") || file_name.starts_with('.') {
            continue;
        }

        // Parse using the registry
        match registry.parse(path) {
            Ok(parse_result) => {
                let uid = compute_uid(path);
                let doc = build_document(&fields, path, &parse_result, &uid, "default");
                // upsert: delete-then-add (Tantivy has no native upsert)
                writer.delete_term(tantivy::Term::from_field_text(fields.uid, &uid));
                writer.add_document(doc)?;
                total += 1;
            }
            Err(pivotsearch_contracts::PivotsearchError::UnsupportedFormat(ext)) => {
                skipped += 1;
                if !ext.is_empty() {
                    eprintln!("  跳过 .{ext}: {file_name}");
                }
            }
            Err(e) => {
                errors += 1;
                eprintln!("  解析失败 {file_name}: {e}");
            }
        }

        if total > 0 && total % 100 == 0 {
            println!("  已索引 {total} 个文件...");
        }
    }

    writer.commit()?;
    drop(writer);

    println!("完成：索引 {total} 个文件，跳过 {skipped}，失败 {errors}");

    // Build a searcher to verify the index is queryable
    let _searcher = SimpleSearcher::new(index, search_fields, tokenizer_manager);
    println!("索引就绪，可用 `pivotsearch search <query>` 查询。");

    Ok(())
}

fn cmd_search(query: &str, index_path: &PathBuf) -> anyhow::Result<()> {
    if !index_path.exists() {
        anyhow::bail!("索引不存在: {}，请先运行 `pivotsearch index <dir>`", index_path.display());
    }

    let (_schema, fields, tokenizer_manager) = build_schema();
    let index = Index::open_in_dir(index_path)?;
    index.tokenizers().register(
        pivotsearch_index::schema::JIEBA_TOKENIZER_NAME,
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

    let searcher = SimpleSearcher::new(index, search_fields, tokenizer_manager)?;
    let request = SearchRequest {
        query: query.to_string(),
        ..Default::default()
    };
    let response = searcher.search(&request)?;

    println!("查询「{query}」命中 {} 条结果：\n", response.total_hits);
    for (i, result) in response.results.iter().enumerate() {
        println!("{}. {}", i + 1, result.title);
        println!("   路径: {}", result.path);
        println!("   类型: {} | 大小: {} 字节", result.parser, result.size);
        println!("   片段: {}", strip_html(&result.snippet));
        println!();
    }

    if response.results.is_empty() {
        println!("（无结果。试试更宽泛的关键词，或确认索引已包含目标文件。）");
    }

    Ok(())
}

fn strip_html(s: &str) -> String {
    // Simple replacement of <b></b> highlight tags for CLI display
    s.replace("<b>", "\x1b[33m").replace("</b>", "\x1b[0m")
}

fn print_help() {
    let version = env!("CARGO_PKG_VERSION");
    println!("pivotsearch v{version}");
    println!();
    println!("跨平台本地全文搜索（CLI）");
    println!();
    println!("用法:");
    println!("  pivotsearch index  <dir> [index_path]   索引一个目录");
    println!("  pivotsearch search <query> [index_path] 搜索");
    println!("  pivotsearch help                        显示帮助");
    println!();
    println!("默认 index_path = ./.pivotsearch-index");
}
