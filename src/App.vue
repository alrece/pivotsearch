<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { ElMessage } from "element-plus";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import {
  search as searchApi,
  listIndexes,
  addIndex,
  removeIndex,
  getPreview,
  rebuildIndex,
  onIndexProgress,
  copyToClipboard,
  openInFolder,
  installCli,
  getIndexDetails,
  type SearchResult,
  type IndexInfo,
  type IndexProgress,
  type PreviewData,
} from "./api";
import { useI18n } from "./composables/useI18n";

const { locale, t, toggleLocale } = useI18n();

// ── Panel width (draggable splitter) ──
const panelWidth = ref(50); // result-list width as a percentage
let isDragging = false;

function startDrag() {
  isDragging = true;
  document.body.style.cursor = "col-resize";
  document.body.style.userSelect = "none";
}

function onDrag(e: MouseEvent) {
  if (!isDragging) return;
  const container = document.querySelector(".result-layout") as HTMLElement;
  if (!container) return;
  const rect = container.getBoundingClientRect();
  const pct = ((e.clientX - rect.left) / rect.width) * 100;
  panelWidth.value = Math.max(20, Math.min(80, pct));
}

function stopDrag() {
  isDragging = false;
  document.body.style.cursor = "";
  document.body.style.userSelect = "";
}

function closePreview() {
  selectedIndex.value = -1;
  previewData.value = null;
}

// ── Search state ──
const query = ref("");
const results = ref<SearchResult[]>([]);
const totalHits = ref(0);
const loading = ref(false);
let searchTimer: ReturnType<typeof setTimeout> | null = null;

// ── Selection / preview state ──
const selectedIndex = ref(-1);
const previewData = ref<PreviewData | null>(null);
const previewLoading = ref(false);

// ── Index management ──
const indexes = ref<IndexInfo[]>([]);
const showIndexDialog = ref(false);
const newPath = ref("");
const progressMsg = ref("");
const indexProgress = ref<{ processed: number; total: number; pct: number } | null>(null);
const isIndexing = ref(false);

// ── Index detail dialog ──
const detailDialog = ref(false);
const indexDetail = ref<any>(null);
let unlistenProgress: (() => void) | null = null;

// ── File type filter ──
const filterType = ref(""); // empty = all
// ── Case sensitivity ──
const caseSensitive = ref(false);
const typeOptions = computed(() => [
  { label: t("typeAll"), value: "" },
  { label: "PDF", value: "pdf" },
  { label: "Word", value: "docx" },
  { label: "Excel", value: "xlsx" },
  { label: "PPT", value: "pptx" },
  { label: "Markdown", value: "md" },
  { label: "HTML", value: "html" },
  { label: t("typeText"), value: "txt" },
]);

// ── Search scope (index dropdown) ──
const searchScope = ref(""); // empty = all indexes
const scopeOptions = computed(() => [
  { label: t("allScopes"), value: "" },
  ...indexes.value.map((idx) => ({
    label: idx.display_name || idx.path,
    value: idx.id,
  })),
]);

// ── Filtered results ──
const filteredResults = computed(() => {
  if (!filterType.value) return results.value;
  return results.value.filter(
    (r) => r.path.toLowerCase().endsWith("." + filterType.value)
  );
});

// ── Instant search ──
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
    const resp = await searchApi(query.value, null, 0, caseSensitive.value);
    results.value = resp.results;
    totalHits.value = resp.total_hits;
    selectedIndex.value = -1;
    previewData.value = null;
  } catch (e) {
    console.error("search failed", e);
    results.value = [];
  } finally {
    loading.value = false;
  }
}

// ── Click a result item -> load preview ──
async function selectResult(index: number) {
  selectedIndex.value = index;
  const result = filteredResults.value[index];
  if (!result) return;

  previewLoading.value = true;
  try {
    previewData.value = await getPreview(result.uid);
  } catch (e) {
    console.error("preview failed", e);
    previewData.value = null;
  } finally {
    previewLoading.value = false;
  }
}

// ── Keyboard navigation ──
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

// ── Preview content highlighting ──
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

// ── snippet highlighting (result list) ──
function renderSnippet(snippet: string): string {
  // snippet already contains <b> tags (highlighted by Rust), convert to <mark>
  return snippet.replace(/<b>/g, '<mark>').replace(/<\/b>/g, "</mark>");
}

// ── Formatting ──
// ── File action buttons ──
async function onCopyPath(path: string) {
  try {
    await copyToClipboard(path);
    ElMessage.success(t("pathCopied"));
  } catch (e) {
    ElMessage.error(t("copyFailed"));
  }
}

async function onOpenFolder(path: string) {
  try {
    await openInFolder(path);
  } catch (e) {
    ElMessage.error(t("openFolderFailed"));
  }
}

async function onIndexDblClick(row: IndexInfo) {
  try {
    indexDetail.value = await getIndexDetails(row.id);
    detailDialog.value = true;
  } catch (e) {
    ElMessage.error(t("getDetailsFailed"));
  }
}

async function onInstallCli() {
  try {
    const msg = await installCli();
    ElMessage.success(msg);
  } catch (e) {
    ElMessage.error(t("installFailed", { msg: String(e) }));
  }
}

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

// ── Extract the filename (with extension) from a path ──
function fileName(path: string): string {
  const parts = path.replace(/\\/g, "/").split("/");
  return parts[parts.length - 1] || path;
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

// ── Index management ──
async function refreshIndexes() {
  try {
    indexes.value = await listIndexes();
  } catch (e) {
    console.error("failed to list indexes", e);
  }
}

// ── Folder picker dialog ──
async function browseFolder() {
  try {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: t("selectIndexDir"),
    });
    if (selected) {
      newPath.value = selected as string;
    }
  } catch (e) {
    console.error("folder selection failed", e);
  }
}

async function onAddIndex() {
  if (!newPath.value.trim()) return;
  try {
    isIndexing.value = true;
    await addIndex(newPath.value);
    newPath.value = "";
    // Progress is driven by the onIndexProgress callback; no setTimeout here.
  } catch (e) {
    isIndexing.value = false;
    ElMessage.error(t("addFailed", { msg: String(e) }));
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
    isIndexing.value = true;
    await rebuildIndex(id);
    // Progress is driven by the onIndexProgress callback.
  } catch (e) {
    isIndexing.value = false;
    ElMessage.error(t("rebuildFailed", { msg: String(e) }));
  }
}

// ── Empty-state flags ──
const hasSearched = computed(() => query.value.length > 0);
const noIndexes = computed(() => indexes.value.length === 0);
const noResults = computed(
  () => hasSearched.value && !loading.value && filteredResults.value.length === 0
);

// ── Lifecycle ──
onMounted(async () => {
  await refreshIndexes();
  unlistenProgress = await onIndexProgress((p: IndexProgress) => {
    progressMsg.value = p.message;
    if (p.phase === "done") {
      isIndexing.value = false;
      indexProgress.value = null;
      progressMsg.value = "";
      ElMessage.success(t("indexComplete"));
      refreshIndexes();
    } else if (p.phase === "error") {
      isIndexing.value = false;
      indexProgress.value = null;
      ElMessage.error(p.message);
    } else {
      isIndexing.value = true;
      const pct = p.total > 0 ? Math.round((p.processed / p.total) * 100) : 0;
      indexProgress.value = { processed: p.processed, total: p.total, pct };
    }
  });
  // Focus the search box
  setTimeout(() => {
    document.querySelector<HTMLInputElement>(".search-input input")?.focus();
  }, 100);
});

onMounted(async () => {
  window.addEventListener("mousemove", onDrag);
  window.addEventListener("mouseup", stopDrag);
  await refreshIndexes();
  unlistenProgress = await onIndexProgress((p: IndexProgress) => {
    progressMsg.value = p.message;
    if (p.phase === "done") {
      isIndexing.value = false;
      indexProgress.value = null;
      progressMsg.value = "";
      ElMessage.success(t("indexComplete"));
      refreshIndexes();
    } else if (p.phase === "error") {
      isIndexing.value = false;
      indexProgress.value = null;
      ElMessage.error(p.message);
    } else {
      isIndexing.value = true;
      const pct = p.total > 0 ? Math.round((p.processed / p.total) * 100) : 0;
      indexProgress.value = { processed: p.processed, total: p.total, pct };
    }
  });
  setTimeout(() => {
    document.querySelector<HTMLInputElement>(".search-input input")?.focus();
  }, 100);
});

onUnmounted(() => {
  if (unlistenProgress) unlistenProgress();
  window.removeEventListener("mousemove", onDrag);
  window.removeEventListener("mouseup", stopDrag);
});
</script>

<template>
  <div class="app" @keydown="onKeydown" tabindex="0">
    <!-- ── Top search bar ── -->
    <header class="topbar">
      <div class="logo">
        <span class="logo-text">PivotSearch</span>
      </div>
      <div class="search-input">
        <el-input
          v-model="query"
          :placeholder="t('searchPlaceholder')"
          size="large"
          @input="onSearchInput"
          clearable
        />
      </div>
      <el-select v-model="searchScope" :placeholder="t('scope')" size="large" class="scope-select">
        <el-option
          v-for="opt in scopeOptions"
          :key="opt.value"
          :label="opt.label"
          :value="opt.value"
        />
      </el-select>
      <el-select v-model="filterType" :placeholder="t('type')" size="large" class="type-select">
        <el-option
          v-for="opt in typeOptions"
          :key="opt.value"
          :label="opt.label"
          :value="opt.value"
        />
      </el-select>
      <el-tooltip :content="caseSensitive ? t('caseSensitiveOn') : t('caseSensitiveOff')" placement="bottom">
        <button
          class="case-toggle"
          :class="{ active: caseSensitive }"
          @click="caseSensitive = !caseSensitive; onSearchInput()"
        >
          Aa
        </button>
      </el-tooltip>
      <el-button size="large" type="primary" @click="doSearch" :loading="loading">
        {{ t('search') }}
      </el-button>
      <el-button size="large" @click="showIndexDialog = true">
        {{ t('indexManagement') }}
      </el-button>
      <button class="lang-toggle" :title="locale === 'en' ? '中文' : 'English'" @click="toggleLocale">
        {{ locale === 'en' ? '中' : 'EN' }}
      </button>
    </header>

    <!-- ── Main: left result list + right preview panel ── -->
    <main class="main-body">
      <!-- No-index welcome -->
      <div v-if="noIndexes && !hasSearched" class="welcome-screen">
        <div class="welcome-content">
          <div class="welcome-icon">📁</div>
          <h2>{{ t('welcomeTitle') }}</h2>
          <p>{{ t('welcomeDesc') }}</p>
          <el-button type="primary" size="large" @click="showIndexDialog = true">
            {{ t('addIndexToStart') }}
          </el-button>
        </div>
      </div>

      <!-- Search results layout -->
      <div v-else class="result-layout">
        <!-- Left: result list -->
        <div class="result-panel" :style="{ flex: '0 0 ' + panelWidth + '%' }">
          <!-- Result header -->
          <div class="result-header" v-if="hasSearched">
            <span v-if="loading">{{ t('searching') }}</span>
            <span v-else>{{ t('resultsFound', { n: totalHits }) }}</span>
            <span class="result-filter" v-if="filterType">
              {{ t('filteredSuffix', { ext: filterType }) }}
            </span>
            <span v-if="isIndexing" class="indexing-hint">
              {{ t('indexingWarning') }}
            </span>
          </div>

          <!-- Empty search hint -->
          <div v-if="!hasSearched && !noIndexes" class="empty-search">
            <p>{{ t('emptySearchHint') }}</p>
          </div>

          <!-- No results -->
          <div v-if="noResults" class="no-results">
            <p>{{ t('noResultsFound', { query }) }}</p>
            <p class="hint">{{ t('noResultsHint') }}</p>
          </div>

          <!-- Result list -->
          <div
            v-for="(r, i) in filteredResults"
            :key="r.uid"
            class="result-item"
            :class="{ selected: i === selectedIndex }"
            @click="selectResult(i)"
          >
            <div class="result-item-header">
              <span class="file-icon">{{ fileIcon(r.path) }}</span>
              <span class="file-title">{{ fileName(r.path) }}</span>
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
              <span class="meta-actions">
                <button class="meta-btn" :title="t('copyPath')" @click.stop="onCopyPath(r.path)">📋</button>
                <button class="meta-btn" :title="t('openFolder')" @click.stop="onOpenFolder(r.path)">📂</button>
              </span>
            </div>
          </div>
        </div>

        <!-- Draggable splitter -->
        <div
          v-if="selectedIndex >= 0 || previewLoading"
          class="splitter"
          @mousedown="startDrag"
        ></div>

        <!-- Right: preview panel -->
        <div class="preview-panel" v-if="selectedIndex >= 0 || previewLoading">
          <div class="preview-header">
            <span v-if="previewData" class="preview-title">
              {{ previewData.path.split("/").pop() }}
            </span>
            <span v-else>{{ t('previewLoading') }}</span>
            <button class="preview-close" :title="t('close')" @click="closePreview">✕</button>
          </div>
          <div class="preview-content" v-loading="previewLoading">
            <div v-if="previewData && previewData.exists" class="preview-text">
              <pre v-html="renderPreviewContent(previewData.content)"></pre>
            </div>
            <div v-else-if="previewData && !previewData.exists" class="preview-error">
              <p>📁 {{ t('fileNotFound') }}</p>
              <p class="hint">{{ previewData?.path }}</p>
            </div>
          </div>
        </div>
      </div>
    </main>

    <!-- ── Bottom status bar ── -->
    <footer class="statusbar">
      <!-- Index progress bar -->
      <template v-if="isIndexing && indexProgress">
        <span class="status-progress">
          {{ progressMsg || t('indexingProgress', { pct: indexProgress.pct, processed: indexProgress.processed, total: indexProgress.total }) }}
        </span>
        <el-progress
          :percentage="indexProgress?.pct ?? 0"
          :stroke-width="14"
          :show-text="false"
          style="width: 200px; margin-left: 8px;"
        />
      </template>
      <span v-else-if="progressMsg" class="status-progress">{{ progressMsg }}</span>
      <span v-else-if="indexes.length > 0" class="status-info">
        📂 {{ indexes.length }} {{ t('indexManagement') }}
        <template v-for="(idx, i) in indexes" :key="idx.id">
          <span v-if="i > 0">·</span>
          {{ idx.display_name || idx.path.split("/").pop() }}
          ({{ idx.file_count }})
        </template>
      </span>
      <span v-else class="status-info">{{ t('ready') }} · {{ t('addIndexToStart') }}</span>
    </footer>

    <!-- ── Index management dialog ── -->
    <el-dialog v-model="showIndexDialog" :title="t('indexManagementTitle')" width="600px">
      <div class="index-add">
        <el-input
          v-model="newPath"
          :placeholder="t('selectIndexDir')"
          @keyup.enter="onAddIndex"
        />
        <el-button @click="browseFolder">📁 {{ t('browseFolder') }}</el-button>
        <el-button type="primary" @click="onAddIndex" :disabled="isIndexing">{{ t('addIndex') }}</el-button>
      </div>

      <el-table :data="indexes" stripe style="width: 100%; margin-top: 16px" @row-dblclick="onIndexDblClick">
        <el-table-column prop="display_name" :label="t('indexPath')" width="150">
          <template #default="{ row }">
            {{ row.display_name || row.path.split("/").pop() }}
          </template>
        </el-table-column>
        <el-table-column prop="path" :label="t('indexPath')" />
        <el-table-column prop="file_count" :label="t('fileCount')" width="80" />
        <el-table-column :label="t('actions')" width="150">
          <template #default="{ row }">
            <el-button size="small" @click="onRebuildIndex(row.id)" :disabled="isIndexing">{{ t('rebuild') }}</el-button>
            <el-button size="small" type="danger" @click="onRemoveIndex(row.id)">{{ t('remove') }}</el-button>
          </template>
        </el-table-column>
      </el-table>

      <div class="cli-install">
        <el-button @click="onInstallCli">💻 {{ t('installCli') }} (psearch)</el-button>
      </div>
    </el-dialog>

    <!-- ── Index detail dialog ── -->
    <el-dialog v-model="detailDialog" :title="t('indexDetailTitle')" width="600px">
      <div v-if="indexDetail" class="detail-content">
        <div class="detail-section">
          <h4>{{ t('indexDetailTitle') }}</h4>
          <div class="detail-row"><span>{{ t('indexPath') }}</span><span>{{ indexDetail.name || indexDetail.path.split("/").pop() }}</span></div>
          <div class="detail-row"><span>{{ t('indexPath') }}</span><span class="mono">{{ indexDetail.path }}</span></div>
          <div class="detail-row"><span>{{ t('fileCount') }}</span><span>{{ indexDetail.file_count }}</span></div>
        </div>

        <div class="detail-section">
          <h4>{{ t('fileTypeDistribution') }}</h4>
          <div v-for="stat in indexDetail.parser_stats" :key="stat.parser" class="detail-row">
            <span>{{ stat.parser }}</span>
            <span>
              <el-progress
                :percentage="Math.round(stat.count / indexDetail.file_count * 100)"
                :stroke-width="12"
                :format="() => stat.count.toString()"
                style="width: 150px;"
              />
            </span>
          </div>
        </div>

        <div class="detail-section">
          <h4>{{ t('recentFiles') }}</h4>
          <el-table :data="indexDetail.recent_files" stripe size="small" style="width: 100%">
            <el-table-column prop="path" :label="t('indexPath')" show-overflow-tooltip>
              <template #default="{ row }">
                {{ row.path.split("/").pop() }}
              </template>
            </el-table-column>
            <el-table-column :label="t('recentFiles')" width="120">
              <template #default="{ row }">
                {{ formatDate(row.mtime) }}
              </template>
            </el-table-column>
            <el-table-column prop="parser" :label="t('type')" width="120" />
          </el-table>
        </div>
      </div>
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

/* ── Top search bar ── */
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
  font-size: 19px;
  font-weight: 700;
  color: var(--ps-primary);
  white-space: nowrap;
  letter-spacing: 0.3px;
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

.case-toggle {
  width: 36px;
  height: 36px;
  border: 1px solid var(--ps-border);
  border-radius: 6px;
  background: var(--ps-surface);
  cursor: pointer;
  font-size: 14px;
  font-weight: 700;
  color: var(--ps-text-secondary);
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s;
}

.case-toggle:hover {
  border-color: var(--ps-primary);
}

.case-toggle.active {
  background: var(--ps-primary);
  color: #fff;
  border-color: var(--ps-primary);
}

/* Language toggle button — shares the case-toggle visual language. */
.lang-toggle {
  width: 36px;
  height: 36px;
  border: 1px solid var(--ps-border);
  border-radius: 6px;
  background: var(--ps-surface);
  cursor: pointer;
  font-size: 13px;
  font-weight: 700;
  color: var(--ps-text-secondary);
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s;
}

.lang-toggle:hover {
  border-color: var(--ps-primary);
  color: var(--ps-primary);
}

/* ── Main body ── */
.main-body {
  flex: 1;
  overflow: hidden;
  display: flex;
}

/* Welcome screen */
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

/* Result layout */
.result-layout {
  flex: 1;
  display: flex;
  overflow: hidden;
}

/* Left: result list */
.result-panel {
  overflow-y: auto;
  background: var(--ps-surface);
  min-width: 200px;
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

.meta-actions {
  display: inline-flex;
  gap: 2px;
  margin-left: 4px;
}

.meta-btn {
  background: none;
  border: none;
  cursor: pointer;
  font-size: 13px;
  padding: 0 3px;
  border-radius: 3px;
  opacity: 0.5;
  transition: opacity 0.1s, background 0.1s;
}

.meta-btn:hover {
  opacity: 1;
  background: #e8e8e8;
}

/* Draggable splitter */
.splitter {
  width: 5px;
  flex-shrink: 0;
  background: var(--ps-border);
  cursor: col-resize;
  position: relative;
  transition: background 0.15s;
}

.splitter:hover,
.splitter:active {
  background: var(--ps-primary);
}

/* Right: preview panel */
.preview-panel {
  flex: 1;
  min-width: 200px;
  display: flex;
  flex-direction: column;
  background: var(--ps-surface);
  overflow: hidden;
}

.preview-header {
  padding: 8px 12px;
  background: #f5f7fa;
  border-bottom: 1px solid var(--ps-border);
  font-size: 13px;
  font-weight: 600;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.preview-title {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.preview-close {
  background: none;
  border: none;
  cursor: pointer;
  font-size: 15px;
  color: var(--ps-text-secondary);
  padding: 2px 6px;
  border-radius: 4px;
  flex-shrink: 0;
  transition: background 0.1s, color 0.1s;
}

.preview-close:hover {
  background: #e8e8e8;
  color: var(--ps-text);
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

/* ── Bottom status bar ── */
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

/* ── Index management dialog ── */
.index-add {
  display: flex;
  gap: 8px;
}

.cli-install {
  margin-top: 16px;
  padding-top: 12px;
  border-top: 1px solid var(--ps-border);
  display: flex;
  align-items: center;
  gap: 8px;
}

.cli-hint {
  font-size: 12px;
  color: var(--ps-text-secondary);
}

.indexing-hint {
  margin-left: 8px;
  color: #e6a23c;
  font-size: 12px;
}

.detail-content {
  max-height: 500px;
  overflow-y: auto;
}

.detail-section {
  margin-bottom: 20px;
}

.detail-section h4 {
  margin-bottom: 8px;
  font-size: 14px;
  color: var(--ps-text);
  border-bottom: 1px solid var(--ps-border);
  padding-bottom: 4px;
}

.detail-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 4px 0;
  font-size: 13px;
}

.detail-row span:first-child {
  color: var(--ps-text-secondary);
}

.mono {
  font-family: "SF Mono", "Cascadia Code", monospace;
  font-size: 12px;
}

/* Element Plus overrides */
.el-input__wrapper {
  border-radius: 6px;
}

.el-button--primary {
  --el-button-bg-color: var(--ps-primary);
  --el-button-border-color: var(--ps-primary);
}
</style>
