# AGENTS.md

English | [中文](AGENTS.zh-CN.md)

This file is the collaboration baseline document for this repository, providing unified working guidance for code agents and implementation collaborators. If there is a conflict with other collaboration documents (CLAUDE.md, etc.), this file takes precedence.

## Project Overview

`pivotsearch` is a cross-platform (Windows / macOS / Linux) local full-text search desktop application, positioned as an open-source alternative to AnyTXT Searcher. It draws on the core design logic of DocFetcher (Java/Lucene) — mtime-driven incremental indexing, file-tree diff, Parser registry, one index root per directory — and does the engineering integration with a modern Rust component stack (Tantivy + Tauri), while filling in the OCR capability that DocFetcher lacks.

Core philosophy:

- **Local-first, offline-capable**: All indexing and searching is done entirely on the user's machine; no data is sent to any external service (except for OCR language-pack downloads)
- **Incremental indexing**: An incremental algorithm based on mtime + file-tree diff, combined with file-system watching, achieves "update on change"
- **Multi-format parsing**: PDF / Office (docx/xlsx/pptx) / Markdown / HTML / plain text / ePub / source code, primarily in pure Rust, with native dependencies introduced in a controlled way for PDF Chinese support and OCR
- **Chinese-friendly**: Built-in jieba tokenization, handling mixed Chinese/English text and legacy GBK/Big5 encodings

The core capabilities currently planned for this repository (8 capabilities):

- Indexing engine: Tantivy schema design, uid primary key, Document assembly (`core-index-schema`)
- Parsing layer: Parser registry + two-level selection (mime first / extension fallback) + per-format parsers (`parser-registry`)
- Incremental indexing: mtime comparison + unseenDocs file-tree diff + whole-archive skip + SQLite metadata persistence (`incremental-index`)
- File watching: notify cross-platform watching + debouncing + event filtering + mtime secondary verification for denoising (`file-watcher`)
- Indexing queue: single-worker task queue + Task state machine + multi-index concurrency (`indexing-queue`)
- Search engine: multi-index merging + query parsing + pagination + highlighting + interruption (`search-engine`)
- OCR pipeline: Tesseract integration + image/scanned-document recognition + on-demand language-pack download (`ocr-pipeline`, feature gate, optional)
- Desktop UI: Tauri 2 frontend + instant search + result highlighting + preview panel + index management (`desktop-ui`)

## AI Collaboration Entry Points

- `AGENTS.md`: Repository-level factual baseline; the single source of truth for architectural facts, development conventions, and delivery principles
- `CLAUDE.md`: Claude Code-specific quick entry point; retains only usage instructions and a command index
- `.loop/`: Loop Engineering macro-loop state (see below)

Maintenance conventions:

- Changes involving repository facts, directory structure, commands, or development rules should update `AGENTS.md` first
- Changes involving Claude usage should then update `CLAUDE.md`
- If `AGENTS.md` conflicts with other collaboration documents, `AGENTS.md` takes precedence

## Loop Engineering Collaboration Conventions

This section takes effect only when the user explicitly runs `/loop:*` commands; it governs how agents use the `Loop Engineering` skill within this repository.

### Iron Rules (MUST)

- Use `.loop/STATE.yaml` as the single source of truth; the current `phase`, `step`, `iteration`, `blocker`, and phase status must all be read in real time from this file — do not guess or reuse stale cache
- Re-read `STATE.yaml` on every execution of `/loop:run`
- Every producing stage — `spec` / `design` / `plan` / `execute` / `review` / `ship` — must run `adversarial_gate` (deterministic checks + clean-room grep + compile verification) after producing its output
- `execute` must perform plan-level, item-by-item checks, not a one-shot phase-level pass
- All state changes must be atomically written back to `.loop/STATE.yaml` (write to `.tmp` first, then rename), and appended to `.loop/timeline.jsonl`, ensuring auditability and recoverability
- All automated decisions must be appended to `.loop/decisions.jsonl`
- All user-facing output must use Simplified Chinese; code, commands, paths, config keys, function names, and proper nouns retain their original form

### Prohibitions (MUST NOT)

- Do not let artifacts that failed the adversarial check proceed downstream
- Do not skip Gate 5 (consistency check between loop state and GSD `.planning/STATE.md`)
- Do not add `--force` on your own; safety gates may only be skipped when the user explicitly passes it
- Do not break the existing structure of `STATE.yaml` (fields such as `phase_status`, `artifacts`, `history`)

### Safety Gates (Gate 1-5)

- **Gate 1**: Stop when the `blockers` of the current phase in `STATE.yaml` is non-empty
- **Gate 2**: The artifacts of the previous phase must actually exist on disk
- **Gate 3**: When advancing from Phase 4 to Phase 5, `qa-report.md` must not contain `FAIL`
- **Gate 4**: `.loop/adversarial/last-verdict.json` must have `passed=true`
- **Gate 5**: Before `execute` advances, the loop state must be verified as consistent with the GSD `STATE.md`

## Technology Stack

### Rust Backend (Core)

- Full-text engine: `tantivy` 0.24 (inverted index + query)
- Chinese tokenization: `jieba-rs` + custom Tantivy Tokenizer (does not depend on `tantivy-jieba` version synchronization)
- PDF: `pdfium-render` (statically links Google PDFium, ensures Chinese quality)
- Office: `calamine` (xlsx/xls/csv), `docx-rs` or `ooxmlsdk` (docx), `ooxmlsdk` (pptx)
- Markdown: `pulldown-cmark`
- HTML: `scraper` (body extraction), `lol_html` (streaming for large files)
- Plain text / encoding: `encoding_rs` + `chardetng` (GBK/Big5 detection)
- ePub: `epub` crate (zip + xhtml)
- OCR: `kreuzberg-tesseract` (built-in static compilation, feature gate, optional)
- mime detection: `infer` (magic numbers)
- File watching: `notify` 6.x + `notify-debouncer-full`
- File traversal: `walkdir`
- Archive traversal: `zip` / `tar` / `sevenz-rust`
- Metadata storage: `rusqlite` (SQLite, replacing Java serialization)
- Concurrency: `crossbeam-channel` / `parking_lot`
- Logging: `tracing`
- Errors: `thiserror` + `anyhow`
- Async: `tokio`

### Frontend (Tauri 2)

- Vue 3 + TypeScript + Vite
- Element Plus component library
- Pinia state management
- Instant search (debounced input) + virtual list + highlight rendering + preview panel

### Not Supported (Out of v1 Scope)

- **Legacy MS Office formats .doc / .ppt**: The Rust ecosystem has no mature pure-Rust parser; reverse-engineering the binary format is impractical. When detected, the UI prompts the user to convert to .docx / .pptx
- **.xls**: Usable via `calamine`, within scope

## Runtime Constraints (Iron Rules)

### Dependency-Direction Iron Rule

- The orchestration layer (`crates/core`) depends only on the trait definitions in `crates/contracts`, and **must never import concrete implementations** (parser/index/watcher/queue/search/ocr)
- `crates/contracts` is the **dependency sink** (it depends on no other internal crate)
- The only layer that "knows everything" and may import concrete implementations is `crates/cli` and `src-tauri/` (the composition root)
- The capability crates (parser/index/watcher/queue/search/ocr) do not depend on each other; they interact only through contracts traits

```
cli / src-tauri  →  core  →  contracts  ←  parser / index / watcher / queue / search / ocr
```

### Tantivy Key Constraints (Differences from Lucene)

- **Schema is immutable**: A Tantivy schema is fixed once at startup; field evolution requires a reindex, without Lucene's flexibility. Any schema change must explicitly evaluate reindex cost in the spec
- **Single-writer hard constraint**: Only one writer may exist per index directory at a time; the single-worker model (`indexing-queue`) is a **hard constraint** under Tantivy — do not change it to multiple workers concurrently writing the same index. Different index roots may each run an independent writer concurrently
- **No native upsert**: `update` = `delete_term(uid)` + `add_document`, and `delete_term` only takes effect for readers after commit — after an update you must reopen the reader
- **No term-vector offset**: Lucene's FastVectorHighlighter dual-path is merged into one in Tantivy; uniformly use `SnippetGenerator` to re-split text

### Clean-Room Red Line

`pivotsearch` is a **design-logic replication** of DocFetcher (GPL v3), not a code copy. Strictly observe:

- **Prohibited**: Copying any DocFetcher Java source code, class names, identifiers, or comment text
- **Permitted**: Drawing on its public design patterns — mtime-driven incremental, unseenDocs set diff, Parser registry + two-level selection, one index root per directory, uid primary key, parser name stored as an index field
- After every output, the adversarial gate must run the clean-room grep: `grep -ri "docfetcher\|DocFetcher\|net.sourceforge.docfetcher" crates/ src/ src-tauri/` — any hit means failure

### Native Dependency Distribution

- **PDFium**: Statically linked at build time (the `static-bindings` feature of `pdfium-render`); no runtime dependency on the user's machine
- **Tesseract**: `kreuzberg-tesseract` includes static compilation; language packs (`chi_sim`/`eng`, etc.) are downloaded on demand the first time OCR is enabled, and are not included in the default installation package
- **Three-platform priority**: All native dependencies must be packageable on Windows/macOS/Linux across all three platforms; if any platform is unsupported, it must be explicitly marked `platform-limit` in the spec

## Directory Structure

```
pivotsearch/
├── Cargo.toml                    # workspace root
├── crates/
│   ├── contracts/                # ★dependency sink: traits (Parser/Indexer/Searcher/Watcher) + data structures + error types
│   ├── parser/                   # parsing layer: Parser registry + per-format implementations
│   ├── index/                    # indexing layer: Tantivy wrapper + schema + Document assembly + incremental + tree_index (SQLite)
│   ├── watcher/                  # watching layer: notify + debouncing + event filtering + mtime verification
│   ├── queue/                    # task queue: single worker + Task state machine + multi-index concurrency
│   ├── search/                   # query layer: multi-index merging + query parsing + pagination + highlighting
│   ├── ocr/                      # OCR layer: Tesseract integration (feature gate, optional)
│   ├── core/                     # orchestration layer: assembles the above modules, provides the PivotsearchEngine main entry point
│   └── cli/                      # CLI binary (for development-time debugging)
├── src-tauri/                    # Tauri Rust backend
├── src/                          # Vue 3 frontend
│   ├── views/ components/ stores/
├── docs/                         # technical documentation
├── .loop/ openspec/ .planning/   # engineering methodology
├── AGENTS.md CLAUDE.md DESIGN.md README.md Makefile
```

## Common Commands

```bash
# Build / check / test
cargo check                       # compile check for the whole workspace
cargo build --release             # release build
cargo test                        # whole-workspace tests
cargo test -p pivotsearch-index   # single-crate test
cargo clippy --all-targets -- -D warnings  # lint

# Frontend
pnpm install                      # install frontend dependencies
pnpm dev                          # Vite dev server
pnpm build                        # frontend build

# Tauri
cargo tauri dev                   # desktop development mode
cargo tauri build                 # desktop packaging

# Clean-room compliance check (adversarial gate)
grep -ri "docfetcher\|net.sourceforge.docfetcher" crates/ src/ src-tauri/ && echo "FAIL: hit DocFetcher identifier" || echo "PASS: clean-room compliant"

# Loop Engineering (engages only on explicit invocation)
# /loop:init   /loop:status   /loop:run --next [--auto]   /loop:retro
```
