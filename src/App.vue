<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import {
  search as searchApi,
  listIndexes,
  addIndex,
  removeIndex,
  getPreview,
  rebuildIndex,
  onIndexProgress,
  type SearchResult,
  type IndexInfo,
  type IndexProgress,
  type PreviewData,
} from "./api";

// ═══ 搜索状态 ═══
const query = ref("");
const results = ref<SearchResult[]>([]);
const totalHits = ref(0);
const loading = ref(false);
let searchTimer: ReturnType<typeof setTimeout> | null = null;

// ═══ 选中/预览状态 ═══
const selectedIndex = ref(-1);
const previewData = ref<PreviewData | null>(null);
const previewLoading = ref(false);

// ═══ 索引管理 ═══
const indexes = ref<IndexInfo[]>([]);
const showIndexDialog = ref(false);
const newPath = ref("");
const progressMsg = ref("");
let unlistenProgress: (() => void) | null = null;

// ═══ 文件类型筛选 ═══
const filterType = ref(""); // 空=全部
const typeOptions = [
  { label: "全部", value: "" },
  { label: "PDF", value: "pdf" },
  { label: "Word", value: "docx" },
  { label: "Excel", value: "xlsx" },
  { label: "PPT", value: "pptx" },
  { label: "Markdown", value: "md" },
  { label: "HTML", value: "html" },
  { label: "文本", value: "txt" },
];

// ═══ 搜索范围（索引下拉）═══
const searchScope = ref(""); // 空=全部索引
const scopeOptions = computed(() => [
  { label: "全部范围", value: "" },
  ...indexes.value.map((idx) => ({
    label: idx.display_name || idx.path,
    value: idx.id,
  })),
]);

// ═══ 过滤后的结果 ═══
const filteredResults = computed(() => {
  if (!filterType.value) return results.value;
  return results.value.filter(
    (r) => r.path.toLowerCase().endsWith("." + filterType.value)
  );
});

// ═══ 即时搜索 ═══
function onSearchInput() {
  if (searchTimer) clearTimeout(searchTimer);
  if (!query.value.trim()) {
    results.value = [];
    totalHits.value = 0;
    selectedIndex.value = -1;
    previewData.value = null;
    return;
  }
  loading.value = true;
  searchTimer = setTimeout(doSearch, 300);
}

async function doSearch() {
  try {
    const resp = await searchApi(query.value, null, 0);
    results.value = resp.results;
    totalHits.value = resp.total_hits;
    selectedIndex.value = -1;
    previewData.value = null;
  } catch (e) {
    console.error("搜索失败", e);
    results.value = [];
  } finally {
    loading.value = false;
  }
}

// ═══ 点击结果项 → 加载预览 ═══
async function selectResult(index: number) {
  selectedIndex.value = index;
  const result = filteredResults.value[index];
  if (!result) return;

  previewLoading.value = true;
  try {
    previewData.value = await getPreview(result.uid);
  } catch (e) {
    console.error("预览失败", e);
    previewData.value = null;
  } finally {
    previewLoading.value = false;
  }
}

// ═══ 键盘导航 ═══
function onKeydown(e: KeyboardEvent) {
  if (e.key === "ArrowDown") {
    e.preventDefault();
    if (selectedIndex.value < filteredResults.value.length - 1) {
      selectResult(selectedIndex.value + 1);
    }
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    if (selectedIndex.value > 0) {
      selectResult(selectedIndex.value - 1);
    }
  } else if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
    if (searchTimer) clearTimeout(searchTimer);
    doSearch();
  }
}

// ═══ 预览内容高亮 ═══
function renderPreviewContent(content: string): string {
  if (!query.value || !content) return escapeHtml(content);
  const terms = query.value.split(/\s+/).filter(Boolean);
  let result = escapeHtml(content);
  for (const term of terms) {
    const escaped = term.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const regex = new RegExp(`(${escaped})`, "gi");
    result = result.replace(regex, '<span class="hl">$1</span>');
  }
  return result;
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

// ═══ snippet 高亮（结果列表）═══
function renderSnippet(snippet: string): string {
  // snippet 已经含 <b> 标签（Rust 端高亮），直接转 <mark>
  return snippet.replace(/<b>/g, '<mark>').replace(/<\/b>/g, "</mark>");
}

// ═══ 格式化 ═══
function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function formatDate(ts: number): string {
  if (!ts) return "";
  const d = new Date(ts);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

function fileIcon(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase();
  const icons: Record<string, string> = {
    pdf: "📄", doc: "📝", docx: "📝", xls: "📊", xlsx: "📊",
    ppt: "📑", pptx: "📑", md: "📃", html: "🌐", htm: "🌐",
    txt: "📄", epub: "📖", zip: "🗜️", tar: "🗜️",
  };
  return icons[ext || ""] || "📄";
}

// ═══ 索引管理 ═══
async function refreshIndexes() {
  try {
    indexes.value = await listIndexes();
  } catch (e) {
    console.error("获取索引失败", e);
  }
}

// ═══ 目录选择对话框 ═══
async function browseFolder() {
  try {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: "选择要索引的目录",
    });
    if (selected) {
      newPath.value = selected as string;
    }
  } catch (e) {
    console.error("目录选择失败", e);
  }
}

async function onAddIndex() {
  if (!newPath.value.trim()) return;
  try {
    progressMsg.value = "正在添加并索引...";
    await addIndex(newPath.value);
    newPath.value = "";
    setTimeout(async () => {
      await refreshIndexes();
      progressMsg.value = "";
    }, 3000);
  } catch (e) {
    progressMsg.value = `添加失败: ${e}`;
  }
}

async function onRemoveIndex(id: string) {
  try {
    await removeIndex(id);
    await refreshIndexes();
  } catch (e) {
    console.error(e);
  }
}

async function onRebuildIndex(id: string) {
  try {
    progressMsg.value = "正在重建索引...";
    await rebuildIndex(id);
    setTimeout(() => (progressMsg.value = ""), 3000);
  } catch (e) {
    progressMsg.value = `重建失败: ${e}`;
  }
}

// ═══ 空状态判断 ═══
const hasSearched = computed(() => query.value.length > 0);
const noIndexes = computed(() => indexes.value.length === 0);
const noResults = computed(
  () => hasSearched.value && !loading.value && filteredResults.value.length === 0
);

// ═══ 生命周期 ═══
onMounted(async () => {
  await refreshIndexes();
  unlistenProgress = await onIndexProgress((p: IndexProgress) => {
    progressMsg.value = p.message;
  });
  // 聚焦搜索框
  setTimeout(() => {
    document.querySelector<HTMLInputElement>(".search-input input")?.focus();
  }, 100);
});

onUnmounted(() => {
  if (unlistenProgress) unlistenProgress();
});
</script>

<template>
  <div class="app" @keydown="onKeydown" tabindex="0">
    <!-- ═══ 顶部搜索栏 ═══ -->
    <header class="topbar">
      <div class="logo">
        <span class="logo-icon">🔍</span>
        <span class="logo-text">pivotsearch</span>
      </div>
      <div class="search-input">
        <el-input
          v-model="query"
          placeholder="输入关键词搜索文件内容..."
          size="large"
          @input="onSearchInput"
          clearable
        />
      </div>
      <el-select v-model="searchScope" placeholder="范围" size="large" class="scope-select">
        <el-option
          v-for="opt in scopeOptions"
          :key="opt.value"
          :label="opt.label"
          :value="opt.value"
        />
      </el-select>
      <el-select v-model="filterType" placeholder="类型" size="large" class="type-select">
        <el-option
          v-for="opt in typeOptions"
          :key="opt.value"
          :label="opt.label"
          :value="opt.value"
        />
      </el-select>
      <el-button size="large" type="primary" @click="doSearch" :loading="loading">
        搜索
      </el-button>
      <el-button size="large" @click="showIndexDialog = true">
        索引管理
      </el-button>
    </header>

    <!-- ═══ 主体：左结果列表 + 右预览面板 ═══ -->
    <main class="main-body">
      <!-- 无索引引导 -->
      <div v-if="noIndexes && !hasSearched" class="welcome-screen">
        <div class="welcome-content">
          <div class="welcome-icon">📁</div>
          <h2>欢迎使用 pivotsearch</h2>
          <p>跨平台本地全文搜索 · 支持 PDF/Word/Excel/Markdown 等 9 种格式</p>
          <el-button type="primary" size="large" @click="showIndexDialog = true">
            添加索引目录开始使用
          </el-button>
        </div>
      </div>

      <!-- 搜索结果布局 -->
      <div v-else class="result-layout">
        <!-- 左：结果列表 -->
        <div class="result-panel">
          <!-- 结果头部 -->
          <div class="result-header" v-if="hasSearched">
            <span v-if="loading">搜索中...</span>
            <span v-else>找到 {{ totalHits }} 个结果</span>
            <span class="result-filter" v-if="filterType">
              （已筛选 .{{ filterType }}）
            </span>
          </div>

          <!-- 空搜索提示 -->
          <div v-if="!hasSearched && !noIndexes" class="empty-search">
            <p>🔍 在上方输入关键词开始搜索</p>
          </div>

          <!-- 无结果 -->
          <div v-if="noResults" class="no-results">
            <p>未找到包含「{{ query }}」的文件</p>
            <p class="hint">建议：尝试更短的关键词，或检查索引目录</p>
          </div>

          <!-- 结果列表 -->
          <div
            v-for="(r, i) in filteredResults"
            :key="r.uid"
            class="result-item"
            :class="{ selected: i === selectedIndex }"
            @click="selectResult(i)"
          >
            <div class="result-item-header">
              <span class="file-icon">{{ fileIcon(r.path) }}</span>
              <span class="file-title">{{ r.title }}</span>
            </div>
            <div
              class="file-snippet"
              v-html="renderSnippet(r.snippet)"
            ></div>
            <div class="file-meta">
              <span class="meta-path">{{ r.path }}</span>
              <span class="meta-sep">·</span>
              <span>{{ formatSize(r.size) }}</span>
              <span class="meta-sep">·</span>
              <span>{{ formatDate(r.last_modified) }}</span>
              <span class="meta-sep">·</span>
              <span class="meta-parser">{{ r.parser }}</span>
            </div>
          </div>
        </div>

        <!-- 右：预览面板 -->
        <div class="preview-panel" v-if="selectedIndex >= 0 || previewLoading">
          <div class="preview-header">
            <span v-if="previewData">
              {{ previewData.path.split("/").pop() }}
            </span>
            <span v-else>加载中...</span>
          </div>
          <div class="preview-content" v-loading="previewLoading">
            <div v-if="previewData && previewData.exists" class="preview-text">
              <pre v-html="renderPreviewContent(previewData.content)"></pre>
            </div>
            <div v-else-if="previewData && !previewData.exists" class="preview-error">
              <p>📁 文件不可访问</p>
              <p class="hint">{{ previewData?.path }}</p>
              <p class="hint">文件可能已被移动或删除</p>
            </div>
          </div>
        </div>
      </div>
    </main>

    <!-- ═══ 底部状态栏 ═══ -->
    <footer class="statusbar">
      <span v-if="progressMsg" class="status-progress">{{ progressMsg }}</span>
      <span v-else-if="indexes.length > 0" class="status-info">
        📂 {{ indexes.length }} 个索引目录
        <template v-for="(idx, i) in indexes" :key="idx.id">
          <span v-if="i > 0">·</span>
          {{ idx.display_name || idx.path.split("/").pop() }}
          ({{ idx.file_count }} 文件)
        </template>
      </span>
      <span v-else class="status-info">就绪 · 添加索引目录开始使用</span>
    </footer>

    <!-- ═══ 索引管理对话框 ═══ -->
    <el-dialog v-model="showIndexDialog" title="索引管理" width="600px">
      <div class="index-add">
        <el-input
          v-model="newPath"
          placeholder="点击右侧按钮选择目录，或手动输入路径"
          @keyup.enter="onAddIndex"
        />
        <el-button @click="browseFolder">📁 浏览</el-button>
        <el-button type="primary" @click="onAddIndex">添加</el-button>
      </div>

      <el-table :data="indexes" stripe style="width: 100%; margin-top: 16px">
        <el-table-column prop="display_name" label="名称" width="150">
          <template #default="{ row }">
            {{ row.display_name || row.path.split("/").pop() }}
          </template>
        </el-table-column>
        <el-table-column prop="path" label="路径" />
        <el-table-column prop="file_count" label="文件数" width="80" />
        <el-table-column label="操作" width="150">
          <template #default="{ row }">
            <el-button size="small" @click="onRebuildIndex(row.id)">重建</el-button>
            <el-button size="small" type="danger" @click="onRemoveIndex(row.id)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-dialog>
  </div>
</template>

<style>
:root {
  --ps-primary: #1972f5;
  --ps-primary-light: #e8f1ff;
  --ps-bg: #f5f6f8;
  --ps-surface: #ffffff;
  --ps-border: #e4e7ed;
  --ps-text: #303133;
  --ps-text-secondary: #909399;
  --ps-highlight: #fff3bf;
  --ps-highlight-blue: #1972f5;
  --ps-selected: #ecf5ff;
}

* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body, #app {
  height: 100%;
  overflow: hidden;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC",
    "Noto Sans SC", "Microsoft YaHei", sans-serif;
  font-size: 13px;
  color: var(--ps-text);
  background: var(--ps-bg);
}

.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  outline: none;
}

/* ═══ 顶部搜索栏 ═══ */
.topbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
  background: var(--ps-surface);
  border-bottom: 1px solid var(--ps-border);
  flex-shrink: 0;
}

.logo {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-right: 8px;
  flex-shrink: 0;
}

.logo-icon {
  font-size: 20px;
}

.logo-text {
  font-size: 15px;
  font-weight: 600;
  color: var(--ps-primary);
  white-space: nowrap;
}

.search-input {
  flex: 1;
  min-width: 200px;
}

.scope-select {
  width: 130px;
  flex-shrink: 0;
}

.type-select {
  width: 100px;
  flex-shrink: 0;
}

/* ═══ 主体 ═══ */
.main-body {
  flex: 1;
  overflow: hidden;
  display: flex;
}

/* 欢迎屏 */
.welcome-screen {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--ps-bg);
}

.welcome-content {
  text-align: center;
  max-width: 400px;
}

.welcome-icon {
  font-size: 64px;
  margin-bottom: 16px;
}

.welcome-content h2 {
  font-size: 22px;
  margin-bottom: 8px;
  color: var(--ps-text);
}

.welcome-content p {
  color: var(--ps-text-secondary);
  margin-bottom: 24px;
  line-height: 1.6;
}

/* 结果布局 */
.result-layout {
  flex: 1;
  display: flex;
  overflow: hidden;
}

/* 左：结果列表 */
.result-panel {
  flex: 1;
  overflow-y: auto;
  background: var(--ps-surface);
  border-right: 1px solid var(--ps-border);
  min-width: 300px;
}

.result-header {
  padding: 8px 16px;
  background: var(--ps-primary-light);
  color: var(--ps-primary);
  font-size: 12px;
  border-bottom: 1px solid var(--ps-border);
  position: sticky;
  top: 0;
  z-index: 1;
}

.result-filter {
  margin-left: 4px;
  color: var(--ps-text-secondary);
}

.empty-search, .no-results {
  padding: 60px 20px;
  text-align: center;
  color: var(--ps-text-secondary);
}

.no-results p {
  margin-bottom: 8px;
}

.hint {
  font-size: 12px;
  color: var(--ps-text-secondary);
}

.result-item {
  padding: 10px 16px;
  border-bottom: 1px solid var(--ps-border);
  cursor: pointer;
  transition: background 0.1s;
}

.result-item:hover {
  background: #f5f7fa;
}

.result-item.selected {
  background: var(--ps-selected);
  border-left: 3px solid var(--ps-primary);
  padding-left: 13px;
}

.result-item-header {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 4px;
}

.file-icon {
  font-size: 14px;
}

.file-title {
  font-weight: 600;
  font-size: 13px;
  color: var(--ps-text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.file-snippet {
  font-size: 12px;
  color: var(--ps-text-secondary);
  line-height: 1.5;
  margin-bottom: 4px;
  max-height: 60px;
  overflow: hidden;
}

.file-snippet mark {
  background: var(--ps-highlight);
  color: var(--ps-text);
  border-radius: 2px;
  padding: 0 1px;
}

.file-meta {
  font-size: 11px;
  color: var(--ps-text-secondary);
  display: flex;
  align-items: center;
  gap: 4px;
  flex-wrap: wrap;
}

.meta-path {
  font-family: "SF Mono", "Cascadia Code", "Consolas", monospace;
  max-width: 400px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.meta-sep {
  opacity: 0.4;
}

.meta-parser {
  background: #f0f2f5;
  padding: 0 4px;
  border-radius: 2px;
}

/* 右：预览面板 */
.preview-panel {
  width: 45%;
  min-width: 300px;
  display: flex;
  flex-direction: column;
  background: var(--ps-surface);
}

.preview-header {
  padding: 8px 16px;
  background: #f5f7fa;
  border-bottom: 1px solid var(--ps-border);
  font-size: 13px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex-shrink: 0;
}

.preview-content {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

.preview-text pre {
  font-family: -apple-system, "Segoe UI", "PingFang SC", sans-serif;
  font-size: 13px;
  line-height: 1.8;
  white-space: pre-wrap;
  word-wrap: break-word;
  color: var(--ps-text);
}

.preview-text .hl {
  background: #d4e8ff;
  color: var(--ps-highlight-blue);
  font-weight: 600;
  padding: 0 2px;
  border-radius: 2px;
}

.preview-error {
  text-align: center;
  padding: 40px 20px;
  color: var(--ps-text-secondary);
}

.preview-error p {
  margin-bottom: 8px;
}

/* ═══ 底部状态栏 ═══ */
.statusbar {
  padding: 6px 16px;
  background: var(--ps-surface);
  border-top: 1px solid var(--ps-border);
  font-size: 12px;
  color: var(--ps-text-secondary);
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.status-progress {
  color: var(--ps-primary);
}

/* ═══ 索引管理对话框 ═══ */
.index-add {
  display: flex;
  gap: 8px;
}

/* Element Plus 覆盖 */
.el-input__wrapper {
  border-radius: 6px;
}

.el-button--primary {
  --el-button-bg-color: var(--ps-primary);
  --el-button-border-color: var(--ps-primary);
}
</style>
