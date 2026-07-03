# Design: i18n + Documentation English-first (v0.5.0)

> Office-hours design doc. Branch: `main`. Date: 2026-07-03.
> Mode: Builder (open source). Plan source: `.loop/refined-prompt.md` (standard version).

## Problem

pivotsearch v0.4.0 ships a polished desktop app but has two gaps an open-source
audience hits immediately:

1. **No language toggle.** The UI is Chinese-only. Non-Chinese users cannot use it.
2. **Chinese-first docs.** README/CHANGELOG/AGENTS/CLAUDE are Chinese, plus ~422
   lines of Chinese code comments. GitHub convention is English-first.

## Agreed Premises (Phase 3)

| # | Premise | Decision |
|---|---------|----------|
| P1 | i18n implementation | **Lightweight composable** (~60 lines), NOT vue-i18n. Zero deps, zero bundle cost. Only 43 strings. |
| P2 | Default language detection | `navigator.language` (WebView-native). No Tauri OS-locale FFI. |
| P3 | Comment translation | AI-translated, human reviews key modules (contracts/index/parser). |
| P4 | CHANGELOG history | English version is **fully English** (incl. history). Chinese parallel keeps full Chinese history. |
| P5 | AGENTS.md | English-first (consistency with "all docs"), Chinese parallel provided. |
| P6 | Release cadence | Commit → tag v0.5.0 → push → Release CI. No separate review gate. |

## Recommended Approach

**Commit strategy:** 5 fine-grained commits (chosen over 2-commit and 1-commit).

```
1. feat(i18n): front-end localization + language toggle         [Workstream A1]
2. feat(cli): psearch --lang flag for human-readable output    [Workstream A2]
3. docs: English-first user docs + parallel Chinese versions   [Workstream B1]
4. docs: translate Chinese code comments to English            [Workstream B2]
5. chore: bump 0.4.0 → 0.5.0 + CHANGELOG + release             [Ship]
```

### Workstream A — i18n (feature, no data-layer touch)

**A1. Front-end localization**
- New: `src/composables/useI18n.ts` — reactive `locale` ref, `t(key)` function,
  `setLocale(lang)`. Persists choice to `localStorage['pivotsearch.locale']`.
- New: `src/locales/en.ts`, `src/locales/zh-CN.ts` — flat key→string maps.
- Default: first launch reads `navigator.language` (`zh-*` → zh-CN, else en).
  Subsequent launches prefer stored localStorage value.
- UI: language toggle (中/EN) in top toolbar. Switch is instant (reactive),
  no restart. `App.vue` wraps content with the composable.
- Iron rule: locale keys never enter index/search/data code paths.

**A2. psearch CLI `--lang` flag**
- Register `--lang <en|zh>` via clap in `crates/psearch/src/main.rs`.
- Default: English. `--lang zh` affects ONLY human-readable text
  (progress messages, status text, error explanations).
- **Iron rule:** JSON output keys/structure are FIXED English regardless of
  `--lang`. This protects AI Agent JSON-parsing stability. The flag never
  mutates the JSON payload — only `eprintln!` / human-facing strings.

### Workstream B — Documentation English-first (no logic changes)

**B1. User docs**
- `README.md`, `CHANGELOG.md`, `AGENTS.md`, `CLAUDE.md` → English-primary.
- Parallel Chinese: `README.zh-CN.md`, `CHANGELOG.zh-CN.md`,
  `AGENTS.zh-CN.md`, `CLAUDE.zh-CN.md`.
- Each English doc gets a header link: `English | [中文](X.zh-CN.md)`;
  Chinese docs get the reverse link.
- CHANGELOG: English version is fully English (history translated too);
  Chinese parallel preserves the full Chinese history verbatim.

**B2. Code comments**
- All Chinese comments in `crates/`, `src/`, `src-tauri/src/` → English.
  ~391 Rust lines + ~31 Vue/TS lines.
- Comments and display strings only. Zero logic changes.
- Clean-room red line unchanged: `grep -ri "docfetcher"` must still be 0 hits.

## Quality Gates (all must PASS before tagging)

1. `cargo check --workspace` — 0 errors
2. `cargo clippy --all-targets -- -D warnings` — 0 warnings (fix the existing
   `unused variable: app` in `src-tauri/src/lib.rs:487` along the way)
3. `pnpm build` — success
4. Manual: launch app, toggle 中/EN both render correctly & instantly;
   `psearch status` (English) and `psearch status --lang zh` (Chinese) both
   correct; `psearch search "x" --json` produces identical JSON structure
   under both `--lang` values.
5. Clean-room grep: 0 hits for `docfetcher`.
6. Dependency-direction check: 0 hits for `pivotsearch-(parser|index|...)` in core.

## Release

- Version `0.4.0 → 0.5.0` (`src-tauri/tauri.conf.json` + CHANGELOG v0.5.0 entry).
- Tag `v0.5.0`, push, monitor Release CI (3 platforms).
- Expected artifacts (5): `aarch64.dmg`, `amd64.AppImage`, `amd64.deb`,
  `x64-setup.exe`, `x64_en-US.msi`.

## Scope Exclusions

- No data-layer changes (index dirs, SQLite schema, CLI JSON protocol unchanged).
- No re-translation of the existing v0.4.0 icon work.
- Tauri native dialogs (file picker / confirm) stay as-is in this version
  (deferred; they show OS-language strings regardless).
