# CLAUDE.md

English | [中文](CLAUDE.zh-CN.md)

Quick-entry guide for Claude Code. Relationship to `AGENTS.md`: if they conflict, `AGENTS.md` wins.

## One-liner

`pivotsearch` is a cross-platform (Win/macOS/Linux) local full-text search desktop app — an open-source alternative to AnyTXT Searcher. It uses a modern Rust stack (Tantivy + Tauri) to replicate DocFetcher's core design and adds OCR support.

## Reading order

1. `AGENTS.md` — the repository constitution: architectural facts + dev conventions + clean-room red lines (highest authority)
2. `.loop/STATE.yaml` — current iteration state (single source of truth)
3. `openspec/changes/pivotsearch-v1-local-search/` — spec-driven development
4. `.planning/ROADMAP.md` — phase roadmap
5. `docs/` — technical docs

## Key conventions cheat-sheet

- **Dependency-direction iron rule**: the `core` orchestration layer depends only on `contracts` traits — it must never import concrete implementations. Only `cli` / `src-tauri` may import concrete implementations.
- **Tantivy constraints**: schema is immutable (changes require reindex); single-writer hard constraint (only one writer per index directory at a time).
- **Clean-room red line**: do NOT copy DocFetcher Java code/class names/identifiers — only replicate its design logic. After producing output, run `grep -ri "docfetcher" crates/ src/ src-tauri/` to verify.
- **Native dependencies**: PDFium is statically linked; Tesseract is optional + language packs downloaded on demand; `.doc` / `.ppt` are not supported in v1.
- **Loop output language**: user-facing Loop Engineering output uses Simplified Chinese; code/commands/paths/function names stay in their original form.

## Command index

```bash
cargo check && cargo test                    # compile + test
cargo clippy --all-targets -- -D warnings    # lint
cargo tauri dev                              # desktop dev mode
grep -ri "docfetcher" crates/ src/ src-tauri/ # clean-room check (no output = pass)
```

## Loop Engineering

Engages only when `/loop:*` commands are explicitly invoked. State is read from `.loop/STATE.yaml`; events are appended to `.loop/timeline.jsonl`. See the "Loop Engineering" section in `AGENTS.md`.
