# Changelog

English | [中文](CHANGELOG.zh-CN.md)

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.5.0] - 2026-07-03

### Added
- **Bilingual UI (i18n)**: Chinese/English language toggle in the top toolbar. Default follows system locale (`navigator.language`); manual choice persists to localStorage. Switch is instant (reactive).
- **`psearch --lang <en|zh>` CLI flag**: controls human-readable output language. Default `en`. JSON output keys remain fixed English regardless of `--lang`, preserving AI Agent parsing stability.
- **English-first documentation**: README, CHANGELOG, AGENTS.md, CLAUDE.md now English-primary, with parallel Chinese versions (`*.zh-CN.md`) and cross-link navigation.
- **Lightweight i18n composable** (~60 lines, zero deps) instead of vue-i18n — chosen for the ~40-string scale.

### Changed
- All Chinese code comments in `crates/`, `src/`, `src-tauri/src/` translated to English (~422 lines). No logic changes.

## [0.4.0] - 2026-07-03

### Changed
- **App icon redesign**: brand green 3D "PS" icon using the macOS squircle (continuous-curvature rounded corners, radius 22.4%) spec
  - 50 icon files regenerated (macOS `.icns` / Windows `.ico` / Linux PNG / iOS 18 / Android 15)
  - 1px ultra-narrow white border; the PS letters nearly fill the rounded canvas
  - Anti-aliasing on the rounded edges (Gaussian smoothing)
  - Unified icon style across all three platforms

### Fixed
- Fixed icon not displaying the squircle corners correctly in dev mode (corner-radius parameter corrected from 13% to the standard 22.4%)

## [0.3.1] - 2026-07-03

### Fixed
- macOS ad-hoc signing fix (`codesign --force --deep --sign -` in CI)

## [0.3.0] - 2026-07-03

### Added
- **psearch CLI tool**: for AI Agent / CloudPivot invocation, with JSON output
  - `psearch search "query" --json` (Agent core call interface)
  - `psearch index/list/remove/rebuild/preview/status`
  - Data directory shared with the desktop app
  - Deployed alongside the app (Tauri sidecar) + macOS symlink registration
- **Indexing progress bar**: shows percentage + file count at the bottom during new/rebuild (e.g. `[Documents] Indexing... 45%`)
- **Index details view**: double-click an index row to open a details popup (file-type distribution + recently modified files list)
- **Case-sensitive search**: Aa toggle button in the search bar
- **Copy path / Open directory**: quick-action buttons on each search result item
- **Draggable divider**: freely adjust the width of the result list and preview panel
- **Directory picker**: system-native directory selection dialog for adding indexes
- **macOS ad-hoc signing**: resolves Safari Gatekeeper interception when downloading the DMG

### Changed
- UI follows the AnyTXT three-column layout (top search bar + left result list + right preview panel + bottom status bar)
- Brand name unified as PivotSearch
- Search result titles now display the filename with its extension
- CI/Release workflow for all three platforms (including sidecar compilation + artifact upload)

### Fixed
- Index list lost after restart (state restored from disk)
- Error when adding the same path twice (open-or-create)
- Empty snippet highlighting (snippet_text field + manual highlighting)
- Tauri version mismatch (NPM/Rust aligned to 2.11.x)

## [0.2.0] - 2026-07-02

### Added
- Installer CI for all three platforms (macOS .dmg / Linux .deb+.AppImage / Windows .msi+.exe)
- GitHub Release auto-creation (with installation instructions)

## [0.1.0] - 2026-07-02

### Added
- Cross-platform desktop app (Tauri 2 + Vue 3 + Rust), supporting macOS / Windows / Linux
- Full-text parsing for 9 file formats: PDF / Word (docx) / Excel (xlsx/xls/csv) / PPT (pptx) / Markdown / HTML / plain text + source code / ePub / archive traversal (zip/tar)
- Tantivy inverted-index engine + jieba Chinese word segmentation (with stop-word filtering)
- Incremental indexing: mtime comparison + unseenDocs file-tree diff + SQLite metadata persistence
- File-system watcher: notify + 1s debounce + event filtering + mtime secondary verification
- Single worker-thread task queue: Task state machine + UPDATE/REBUILD + deduplication
- Multi-index merged search: cross-directory queries + file-type/size/index-root filtering
- Snippet highlighting: snippet_text field + manual query-term highlighting
- Instant search UI (300ms debounce) + result list + preview panel (re-parses the original file)
- Index management panel: system-native directory picker + add/delete/rebuild index
- GBK/Big5 legacy encoding detection (chardetng + encoding_rs)
- OCR pipeline (feature gate): kreuzberg-tesseract + image decoding + TesseractAPI
- PDFium dynamic-linking support (bblanchon/pdfium-binaries)
- CI for all three platforms (GitHub Actions matrix) + Release workflow (4 targets)
- CLI tool (`pivotsearch index <dir>` / `pivotsearch search <query>`)
- Loop Engineering methodology (.loop/openspec/.planning fully auditable)
- 44 unit tests (including real OCR recognition verification)

### Performance
- Indexing throughput: 1087 files/second (baseline test)
- Index size: ~164KB / 500 files

### Known Limitations
- Legacy .doc / .ppt formats not supported (recommend converting to .docx / .pptx)
- PDFium requires running `scripts/fetch-pdfium.sh` to download
- OCR requires `--features ocr` compilation + language packs
- Windows/Linux packaging must run in the corresponding platform's CI environment

## [0.1.0-alpha] - 2026-07-02

### Added
- Engineering scaffold: 9-crate workspace + methodology framework + 8 capability specs
- Core indexing loop: Tantivy schema + Parser registry + SimpleSearcher + CLI
- Incremental indexing and watcher: SQLite tree_index + notify + task queue
- Full-format completion: epub/pptx/archive traversal + multi-index merging
- Tauri desktop UI skeleton: Vue 3 frontend + #[tauri::command] bridge
- CI + documentation: three-platform matrix + Chinese stop-word tuning

[Unreleased]: https://github.com/alrece/pivotsearch/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/alrece/pivotsearch/releases/tag/v0.5.0
[0.4.0]: https://github.com/alrece/pivotsearch/releases/tag/v0.4.0
[0.3.1]: https://github.com/alrece/pivotsearch/releases/tag/v0.3.1
[0.3.0]: https://github.com/alrece/pivotsearch/releases/tag/v0.3.0
[0.2.0]: https://github.com/alrece/pivotsearch/releases/tag/v0.2.0
[0.1.0]: https://github.com/alrece/pivotsearch/releases/tag/v0.1.0
[0.1.0-alpha]: https://github.com/alrece/pivotsearch/releases/tag/v0.1.0-alpha
