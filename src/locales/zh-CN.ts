// 简体中文 locale resources for pivotsearch UI.

export default {
  // ── 顶部搜索栏 ──
  search: "搜索",
  searchPlaceholder: "输入关键词搜索文件内容...",
  scope: "范围",
  type: "类型",
  allScopes: "全部范围",
  caseSensitiveOn: "大小写敏感：已开启",
  caseSensitiveOff: "大小写敏感：已关闭",
  indexManagement: "索引管理",

  // ── 文件类型筛选标签 ──
  typeAll: "全部",
  typeText: "文本",

  // ── 欢迎 / 空状态 ──
  welcomeTitle: "欢迎使用 pivotsearch",
  welcomeDesc:
    "跨平台本地全文搜索 · 支持 PDF/Word/Excel/Markdown 等 9 种格式",
  addIndexToStart: "添加索引目录开始使用",
  emptySearchHint: "在上方输入关键词开始搜索",

  // ── 搜索结果状态 ──
  searching: "搜索中...",
  resultsFound: "找到 {n} 个结果",
  filteredSuffix: "（已筛选 .{ext}）",
  indexingWarning: "⚠ 索引正在构建中，搜索结果可能不完整",
  noResultsFound: "未找到包含「{query}」的文件",
  noResultsHint: "建议：尝试更短的关键词，或检查索引目录",

  // ── 结果项操作 ──
  copyPath: "复制路径",
  openFolder: "打开所在目录",
  pathCopied: "路径已复制",
  copyFailed: "复制失败",
  openFolderFailed: "打开目录失败",

  // ── 预览面板 ──
  preview: "预览",
  previewFailed: "预览失败",
  fileNotFound: "文件不存在",
  noPreviewSelected: "选择一个结果以预览",
  previewLoading: "正在加载预览...",

  // ── 索引管理对话框 ──
  indexManagementTitle: "索引管理",
  indexPath: "索引路径",
  fileCount: "文件数",
  actions: "操作",
  addIndex: "添加索引",
  rebuild: "重建",
  remove: "移除",
  browseFolder: "浏览",
  selectIndexDir: "选择要索引的目录",
  installCli: "安装 CLI",
  close: "关闭",

  // ── 索引操作 ──
  indexComplete: "索引完成",
  addFailed: "添加失败: {msg}",
  rebuildFailed: "重建失败: {msg}",
  getDetailsFailed: "获取详情失败",
  installFailed: "安装失败: {msg}",

  // ── 索引详情对话框 ──
  indexDetailTitle: "索引详情",
  fileTypeDistribution: "文件类型分布",
  recentFiles: "最近修改的文件",

  // ── 状态栏 ──
  indexingProgress: "正在索引... {pct}% ({processed}/{total})",
  ready: "就绪",

  // ── 语言切换 ──
  langZh: "中文",
  langEn: "EN",
} as const;
