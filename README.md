# pivotsearch

> Cross-platform local full-text search desktop app · Open-source alternative to AnyTXT · Windows / macOS / Linux

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![CI](https://github.com/alrece/pivotsearch/actions/workflows/ci.yml/badge.svg)](https://github.com/alrece/pivotsearch/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/alrece/pivotsearch)](https://github.com/alrece/pivotsearch/releases)
[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-red.svg)](https://v2.tauri.app/)

English | [中文](README.zh-CN.md)

`pivotsearch` is a fully local, offline-capable full-text search tool. It indexes the content of documents on your hard drive (PDF / Word / Excel / PowerPoint / Markdown / HTML / plain text / ePub / source code), letting you search local files in seconds like Google—without sending any data to the cloud.

## Interface Preview

Classic AnyTXT-style three-pane layout:

```
┌──────────────────────────────────────────────────────────────────┐
│ 🔍 pivotsearch │ [Search box....] [Scope▾] [Type▾] [Search] [Manage Indexes] │
├────────────────────────────────────┬─────────────────────────────┤
│ Found 3 results                     │  report.md                  │
├────────────────────────────────────┤                             │
│ 📃 Quarterly report                 │  Preview panel              │
│ ...match snippet revenue growth...   │                             │
│ /path/report.md · 2KB · 2024-01-01  │  Revenue grew twenty percent │
├────────────────────────────────────┤  this quarter, exceeding     │
│ 📝 Technical plan                   │  expectations.              │
│ ...match snippet React frontend...   │  The tech department        │
│ /path/plan.docx · 5KB · ...         │  contributed the main growth.│
│                                     │  (keywords highlighted blue)│
├────────────────────────────────────┴─────────────────────────────┤
│ 📂 2 index directories · Documents(42349 files) · Notes(567 files)│
└──────────────────────────────────────────────────────────────────┘
```

## Core Features

- ⚡ **Instant retrieval** — Built on the Tantivy inverted index, with indexing throughput of **1087 files/second**
- 🔄 **Incremental background indexing** — Filesystem watching + mtime comparison, updates on change
- 📄 **9 formats** — PDF / Word(docx) / Excel(xlsx) / PPT(pptx) / Markdown / HTML / TXT / ePub / source code + archive passthrough (zip/tar)
- 🇨🇳 **Chinese-friendly** — jieba tokenization + stop-word filtering + GBK/Big5 encoding detection
- 🖥️ **Native desktop app** — Tauri 2 + Vue 3, packaged for three platforms (.app / .dmg / .exe / .deb)
- 🔍 **Instant search + preview** — Search as you type (300ms debounce) + click to preview full text + keyword highlighting
- 📁 **Directory picker** — Add indexes via the native system directory picker dialog
- 🔒 **Fully offline** — Zero data leakage
- 🔬 **OCR (optional)** — Tesseract integration, images/scanned documents are searchable (feature gate)

## Download & Installation

### Download from Releases (recommended)

Go to [Releases](https://github.com/alrece/pivotsearch/releases) to download the installer for your platform:

| Platform | Format |
|---|---|
| macOS (Apple Silicon) | `.dmg` |
| macOS (Intel) | `.dmg` |
| Windows | `.msi` / `.exe` |
| Linux | `.deb` / `.AppImage` |

> **Note for macOS users**: Since this app is not signed with an Apple Developer certificate, you may see a "cannot verify the developer" warning on first launch. To resolve this:
> - Right-click the app → select "Open" → click "Open" to confirm
> - Or run in Terminal: `sudo xattr -rd com.apple.quarantine /Applications/pivotsearch.app`

### Build from source

```bash
git clone https://github.com/alrece/pivotsearch.git
cd pivotsearch
pnpm install
pnpm tauri build          # package
# or development mode:
pnpm tauri dev            # hot-reload development
```

**Prerequisites**: Rust 1.75+, Node 20+, pnpm, (macOS) Xcode Command Line Tools

### PDF support (optional)

PDF parsing requires the PDFium library. Run the build script to download it:

```bash
./scripts/fetch-pdfium.sh    # auto-detects platform and downloads
```

### OCR support (optional)

```bash
cargo build --features ocr   # enable OCR (first build ~15s, builds Tesseract)
```

## Usage

1. Launch pivotsearch
2. Click "Manage Indexes" → "📁 Browse" to select the directory to index
3. Wait for indexing to complete (the bottom status bar shows progress)
4. Type keywords into the search box for instant results

## Supported Formats

| Format | Extension | Status |
|---|---|---|
| PDF | `.pdf` | ✅ requires PDFium |
| Word | `.docx` | ✅ |
| Excel | `.xlsx` `.xls` `.csv` | ✅ |
| PowerPoint | `.pptx` | ✅ |
| Markdown | `.md` | ✅ |
| HTML | `.html` `.htm` | ✅ |
| Plain text / source code | `.txt` `.rs` `.py` `.js` `.json` `.yaml` etc. | ✅ |
| ePub | `.epub` | ✅ |
| Archive (passthrough indexing) | `.zip` `.tar` `.tar.gz` | ✅ |
| Image (OCR) | `.jpg` `.png` `.tiff` | ⚠️ optional feature |
| Legacy Word | `.doc` | ❌ please convert to `.docx` |

## Tech Stack

**Backend (Rust)**: Tantivy 0.24 (full-text engine) · jieba-rs (Chinese tokenization) · notify (file watching) · SQLite (metadata) · pdfium-render (PDF) · kreuzberg-tesseract (OCR)

**Frontend**: Tauri 2 · Vue 3 · TypeScript · Element Plus · Pinia

See the [tech selection doc](docs/03-tech-selection.md) for details.

## Architecture

```
crates/
├── contracts/    contract layer (trait definitions, dependency sink)
├── parser/       parsing layer (9-format parsers + registry two-stage selection)
├── index/        indexing layer (Tantivy schema + incremental algorithm + SQLite)
├── watcher/      watching layer (notify + debounce + event filtering)
├── queue/        queue layer (single worker thread + Task state machine)
├── search/       query layer (single-index + multi-index merge + highlighting)
├── ocr/          OCR (feature gate, optional)
├── core/         orchestration layer (depends only on contracts)
└── cli/          CLI tool (development & debugging)
src-tauri/        Tauri desktop backend (command bridge)
src/              Vue 3 frontend
```

## CLI Mode

```bash
# index a directory
cargo run --bin pivotsearch -- index /path/to/docs

# search
cargo run --bin pivotsearch -- search "keywords"
```

## Development

```bash
cargo check && cargo test     # backend build + tests (44 tests)
pnpm build                    # frontend build
make cleanroom                # clean-room compliance check
pnpm tauri dev                # desktop hot-reload development
```

The project is managed using the Loop Engineering methodology (`.loop/` + `openspec/` + `.planning/`), fully auditable throughout. See [AGENTS.md](AGENTS.md) for details.

## Project Status

**v0.5.0** — i18n support (Chinese/English UI toggle + `--lang` CLI flag) + English-first documentation.

**v0.4.0** — macOS squircle-style app icon + brand-green 3D "PS" visual redesign.

**v0.3.0** — Installers for three platforms + psearch CLI + progress bar + index details + case-sensitive search.

See [CHANGELOG.md](CHANGELOG.md) for details.

## Acknowledgements

This project draws on the core design logic of classic desktop search tools (such as DocFetcher) — mtime incrementality, file-tree diffing, Parser registry — re-implemented with a modern Rust component stack, and augmented with OCR capability.

## License

[Apache License 2.0](LICENSE)
