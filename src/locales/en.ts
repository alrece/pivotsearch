// English locale resources for pivotsearch UI.

export default {
  // ── Top search bar ──
  search: "Search",
  searchPlaceholder: "Search file contents by keyword...",
  scope: "Scope",
  type: "Type",
  allScopes: "All scopes",
  caseSensitiveOn: "Case sensitive: on",
  caseSensitiveOff: "Case sensitive: off",
  indexManagement: "Index Management",

  // ── File type filter labels ──
  typeAll: "All",
  typeText: "Text",

  // ── Welcome / empty states ──
  welcomeTitle: "Welcome to pivotsearch",
  welcomeDesc:
    "Cross-platform local full-text search · Supports PDF/Word/Excel/Markdown and 9+ formats",
  addIndexToStart: "Add an index directory to get started",
  emptySearchHint: "Enter a keyword above to start searching",

  // ── Search result states ──
  searching: "Searching...",
  resultsFound: "Found {n} results",
  filteredSuffix: "(filtered .{ext})",
  indexingWarning:
    "⚠ Index is being built, search results may be incomplete",
  noResultsFound: 'No files found containing "{query}"',
  noResultsHint: "Tip: try a shorter keyword, or check the index directory",

  // ── Result item actions ──
  copyPath: "Copy path",
  openFolder: "Open containing folder",
  pathCopied: "Path copied",
  copyFailed: "Copy failed",
  openFolderFailed: "Failed to open folder",

  // ── Preview panel ──
  preview: "Preview",
  previewFailed: "Preview failed",
  fileNotFound: "File not found",
  noPreviewSelected: "Select a result to preview",
  previewLoading: "Loading preview...",

  // ── Index management dialog ──
  indexManagementTitle: "Index Management",
  indexPath: "Index Path",
  fileCount: "Files",
  actions: "Actions",
  addIndex: "Add Index",
  rebuild: "Rebuild",
  remove: "Remove",
  browseFolder: "Browse",
  selectIndexDir: "Select a directory to index",
  installCli: "Install CLI",
  close: "Close",

  // ── Index operations ──
  indexComplete: "Indexing complete",
  addFailed: "Add failed: {msg}",
  rebuildFailed: "Rebuild failed: {msg}",
  getDetailsFailed: "Failed to get details",
  installFailed: "Install failed: {msg}",

  // ── Index detail dialog ──
  indexDetailTitle: "Index Details",
  fileTypeDistribution: "File Type Distribution",
  recentFiles: "Recently Modified Files",

  // ── Status bar ──
  indexingProgress: "Indexing... {pct}% ({processed}/{total})",
  ready: "Ready",

  // ── Language switcher ──
  langZh: "中文",
  langEn: "EN",
} as const;
