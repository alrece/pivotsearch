<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, type Component } from "vue";
import { ElMessage } from "element-plus";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import {
  Search,
  FolderOpen,
  FolderPlus,
  Copy,
  X,
  FileText,
  FileSpreadsheet,
  Presentation,
  BookOpen,
  Globe,
  Archive,
  FileCode,
  File,
  Terminal,
} from "lucide-vue-next";
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

const panelWidth = ref(50);
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

const query = ref("");
const results = ref<SearchResult[]>([]);
const totalHits = ref(0);
const loading = ref(false);
let searchTimer: ReturnType<typeof setTimeout> | null = null;

const selectedIndex = ref(-1);
const previewData = ref<PreviewData | null>(null);
const previewLoading = ref(false);

const indexes = ref<IndexInfo[]>([]);
const showIndexDialog = ref(false);
const newPath = ref("");
const progressMsg = ref("");
const indexProgress = ref<{ processed: number; total: number; pct: number } | null>(null);
const isIndexing = ref(false);

const detailDialog = ref(false);
const indexDetail = ref<any>(null);
let unlistenProgress: (() => void) | null = null;

const filterType = ref("");
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

const searchScope = ref("");
const scopeOptions = computed(() => [
  { label: t("allScopes"), value: "" },
  ...indexes.value.map((idx) => ({
    label: idx.display_name || idx.path,
    value: idx.id,
  })),
]);

const filteredResults = computed(() => {
  if (!filterType.value) return results.value;
  return results.value.filter(
    (r) => r.path.toLowerCase().endsWith("." + filterType.value)
  );
});

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

function renderSnippet(snippet: string): string {
  return snippet.replace(/<b>/g, "<mark>").replace(/<\/b>/g, "</mark>");
}

async function onCopyPath(path: string) {
  try {
    await copyToClipboard(path);
    ElMessage.success(t("pathCopied"));
  } catch {
    ElMessage.error(t("copyFailed"));
  }
}

async function onOpenFolder(path: string) {
  try {
    await openInFolder(path);
  } catch {
    ElMessage.error(t("openFolderFailed"));
  }
}

async function onIndexDblClick(row: IndexInfo) {
  try {
    indexDetail.value = await getIndexDetails(row.id);
    detailDialog.value = true;
  } catch {
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

function fileName(path: string): string {
  const parts = path.replace(/\\/g, "/").split("/");
  return parts[parts.length - 1] || path;
}

function fileIcon(path: string): Component {
  const ext = path.split(".").pop()?.toLowerCase() || "";
  const icons: Record<string, Component> = {
    pdf: FileText,
    doc: FileText,
    docx: FileText,
    xls: FileSpreadsheet,
    xlsx: FileSpreadsheet,
    ppt: Presentation,
    pptx: Presentation,
    md: FileCode,
    html: Globe,
    htm: Globe,
    txt: FileText,
    epub: BookOpen,
    zip: Archive,
    tar: Archive,
  };
  return icons[ext] || File;
}

async function refreshIndexes() {
  try {
    indexes.value = await listIndexes();
  } catch (e) {
    console.error("failed to list indexes", e);
  }
}

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
  } catch (e) {
    isIndexing.value = false;
    ElMessage.error(t("rebuildFailed", { msg: String(e) }));
  }
}

const hasSearched = computed(() => query.value.length > 0);
const noIndexes = computed(() => indexes.value.length === 0);
const noResults = computed(
  () => hasSearched.value && !loading.value && filteredResults.value.length === 0
);

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
    <header class="topbar">
      <div class="logo">
        <span class="logo-mark" aria-hidden="true" />
        <span class="logo-text">PivotSearch</span>
      </div>
      <div class="search-input">
        <el-input
          v-model="query"
          :placeholder="t('searchPlaceholder')"
          size="large"
          clearable
          @input="onSearchInput"
        >
          <template #prefix>
            <Search :size="16" class="search-prefix-icon" />
          </template>
        </el-input>
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
          type="button"
          class="case-toggle"
          :class="{ active: caseSensitive }"
          @click="caseSensitive = !caseSensitive; onSearchInput()"
        >
          Aa
        </button>
      </el-tooltip>
      <el-button size="large" type="primary" :loading="loading" @click="doSearch">
        {{ t('search') }}
      </el-button>
      <el-button size="large" @click="showIndexDialog = true">
        {{ t('indexManagement') }}
      </el-button>
      <button
        type="button"
        class="lang-toggle"
        :title="locale === 'en' ? '中文' : 'English'"
        @click="toggleLocale"
      >
        {{ locale === 'en' ? '中' : 'EN' }}
      </button>
    </header>

    <main class="main-body">
      <div v-if="noIndexes && !hasSearched" class="welcome-screen">
        <div class="welcome-content">
          <div class="welcome-icon">
            <FolderPlus :size="48" :stroke-width="1.5" />
          </div>
          <h2>{{ t('welcomeTitle') }}</h2>
          <p>{{ t('welcomeDesc') }}</p>
          <el-button type="primary" size="large" @click="showIndexDialog = true">
            {{ t('addIndexToStart') }}
          </el-button>
        </div>
      </div>

      <div v-else class="result-layout">
        <div class="result-panel" :style="{ flex: '0 0 ' + panelWidth + '%' }">
          <div v-if="hasSearched" class="result-header">
            <span v-if="loading">{{ t('searching') }}</span>
            <span v-else>{{ t('resultsFound', { n: totalHits }) }}</span>
            <span v-if="filterType" class="result-filter">
              {{ t('filteredSuffix', { ext: filterType }) }}
            </span>
            <span v-if="isIndexing" class="indexing-hint">
              {{ t('indexingWarning') }}
            </span>
          </div>

          <div v-if="!hasSearched && !noIndexes" class="empty-search">
            <Search :size="28" class="empty-icon" />
            <p>{{ t('emptySearchHint') }}</p>
          </div>

          <div v-if="noResults" class="no-results">
            <Search :size="28" class="empty-icon" />
            <p>{{ t('noResultsFound', { query }) }}</p>
            <p class="hint">{{ t('noResultsHint') }}</p>
          </div>

          <div v-if="filteredResults.length" class="result-list">
            <article
              v-for="(r, i) in filteredResults"
              :key="r.uid"
              class="result-item"
              :class="{ selected: i === selectedIndex }"
              :style="{ '--delay': `${Math.min(i, 12) * 40}ms` }"
              @click="selectResult(i)"
            >
              <div class="result-item-header">
                <component :is="fileIcon(r.path)" :size="14" class="file-icon" />
                <span class="file-title">{{ fileName(r.path) }}</span>
              </div>
              <div class="file-snippet" v-html="renderSnippet(r.snippet)" />
              <div class="file-meta">
                <span class="meta-path">{{ r.path }}</span>
                <span class="meta-sep">·</span>
                <span>{{ formatSize(r.size) }}</span>
                <span class="meta-sep">·</span>
                <span>{{ formatDate(r.last_modified) }}</span>
                <span class="meta-sep">·</span>
                <span class="meta-parser">{{ r.parser }}</span>
                <span class="meta-actions">
                  <button
                    type="button"
                    class="meta-btn"
                    :title="t('copyPath')"
                    @click.stop="onCopyPath(r.path)"
                  >
                    <Copy :size="13" />
                  </button>
                  <button
                    type="button"
                    class="meta-btn"
                    :title="t('openFolder')"
                    @click.stop="onOpenFolder(r.path)"
                  >
                    <FolderOpen :size="13" />
                  </button>
                </span>
              </div>
            </article>
          </div>
        </div>

        <div
          v-if="selectedIndex >= 0 || previewLoading"
          class="splitter"
          @mousedown="startDrag"
        />

        <div v-if="selectedIndex >= 0 || previewLoading" class="preview-panel">
          <div class="preview-header">
            <span v-if="previewData" class="preview-title">
              {{ previewData.path.split("/").pop() }}
            </span>
            <span v-else>{{ t('previewLoading') }}</span>
            <button type="button" class="preview-close" :title="t('close')" @click="closePreview">
              <X :size="16" />
            </button>
          </div>
          <div v-loading="previewLoading" class="preview-content">
            <div v-if="previewData && previewData.exists" class="preview-text">
              <pre v-html="renderPreviewContent(previewData.content)" />
            </div>
            <div v-else-if="previewData && !previewData.exists" class="preview-error">
              <FolderOpen :size="32" class="empty-icon" />
              <p>{{ t('fileNotFound') }}</p>
              <p class="hint">{{ previewData?.path }}</p>
            </div>
          </div>
        </div>
      </div>
    </main>

    <footer class="statusbar">
      <template v-if="isIndexing && indexProgress">
        <span class="status-progress">
          {{ progressMsg || t('indexingProgress', { pct: indexProgress.pct, processed: indexProgress.processed, total: indexProgress.total }) }}
        </span>
        <el-progress
          :percentage="indexProgress?.pct ?? 0"
          :stroke-width="8"
          :show-text="false"
          style="width: 200px; margin-left: 8px;"
        />
      </template>
      <span v-else-if="progressMsg" class="status-progress">{{ progressMsg }}</span>
      <span v-else-if="indexes.length > 0" class="status-info">
        <FolderOpen :size="13" class="status-icon" />
        {{ indexes.length }} {{ t('indexManagement') }}
        <template v-for="(idx, i) in indexes" :key="idx.id">
          <span v-if="i > 0">·</span>
          {{ idx.display_name || idx.path.split("/").pop() }}
          ({{ idx.file_count }})
        </template>
      </span>
      <span v-else class="status-info">{{ t('ready') }} · {{ t('addIndexToStart') }}</span>
    </footer>

    <el-dialog v-model="showIndexDialog" :title="t('indexManagementTitle')" width="640px">
      <div class="index-add">
        <el-input
          v-model="newPath"
          :placeholder="t('selectIndexDir')"
          @keyup.enter="onAddIndex"
        />
        <el-button @click="browseFolder">
          <FolderOpen :size="14" style="margin-right: 4px" />
          {{ t('browseFolder') }}
        </el-button>
        <el-button type="primary" :disabled="isIndexing" @click="onAddIndex">
          {{ t('addIndex') }}
        </el-button>
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
            <el-button size="small" :disabled="isIndexing" @click="onRebuildIndex(row.id)">
              {{ t('rebuild') }}
            </el-button>
            <el-button size="small" type="danger" @click="onRemoveIndex(row.id)">
              {{ t('remove') }}
            </el-button>
          </template>
        </el-table-column>
      </el-table>

      <div class="cli-install">
        <el-button @click="onInstallCli">
          <Terminal :size="14" style="margin-right: 4px" />
          {{ t('installCli') }} (psearch)
        </el-button>
      </div>
    </el-dialog>

    <el-dialog v-model="detailDialog" :title="t('indexDetailTitle')" width="640px">
      <div v-if="indexDetail" class="detail-content">
        <div class="detail-section">
          <h4>{{ t('indexDetailTitle') }}</h4>
          <div class="detail-row">
            <span>{{ t('indexPath') }}</span>
            <span>{{ indexDetail.name || indexDetail.path.split("/").pop() }}</span>
          </div>
          <div class="detail-row">
            <span>{{ t('indexPath') }}</span>
            <span class="mono">{{ indexDetail.path }}</span>
          </div>
          <div class="detail-row">
            <span>{{ t('fileCount') }}</span>
            <span>{{ indexDetail.file_count }}</span>
          </div>
        </div>

        <div class="detail-section">
          <h4>{{ t('fileTypeDistribution') }}</h4>
          <div v-for="stat in indexDetail.parser_stats" :key="stat.parser" class="detail-row">
            <span>{{ stat.parser }}</span>
            <span>
              <el-progress
                :percentage="Math.round(stat.count / indexDetail.file_count * 100)"
                :stroke-width="10"
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

<style scoped>
.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  outline: none;
  background: var(--color-bg-subtle);
}

.topbar {
  display: flex;
  align-items: center;
  gap: 8px;
  height: var(--nav-height);
  padding: 0 16px;
  background: var(--color-surface);
  border-bottom: 1px solid var(--color-border-subtle);
  flex-shrink: 0;
  box-shadow: var(--shadow-1);
}

.logo {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-right: 8px;
  flex-shrink: 0;
}

.logo-mark {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--color-primary-6);
  box-shadow: 0 0 0 3px var(--color-primary-2);
}

.logo-text {
  font-size: 16px;
  font-weight: 700;
  color: var(--color-primary-6);
  white-space: nowrap;
  letter-spacing: 0.2px;
}

.search-input {
  flex: 1;
  min-width: 200px;
}

.search-prefix-icon {
  color: var(--color-fg-muted);
  display: block;
}

.scope-select {
  width: 130px;
  flex-shrink: 0;
}

.type-select {
  width: 100px;
  flex-shrink: 0;
}

.case-toggle,
.lang-toggle {
  width: 40px;
  height: 40px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
  background: var(--color-bg-subtle);
  cursor: pointer;
  font-size: 13px;
  font-weight: 700;
  color: var(--color-fg-muted);
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background var(--duration-fast) var(--ease-out-quart),
              border-color var(--duration-fast),
              color var(--duration-fast);
}

.case-toggle:hover,
.lang-toggle:hover {
  border-color: var(--color-primary-3);
  color: var(--color-primary-7);
  background: var(--color-primary-1);
}

.case-toggle.active {
  background: var(--color-primary-6);
  color: #fff;
  border-color: var(--color-primary-7);
  box-shadow: 0 0 0 1px var(--color-primary-7), 0 1px 0 rgba(0, 0, 0, 0.06);
}

.main-body {
  flex: 1;
  overflow: hidden;
  display: flex;
}

.welcome-screen {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--color-bg-subtle);
}

.welcome-content {
  text-align: center;
  max-width: 400px;
  padding: 32px;
  background: var(--color-card);
  border: 1px solid var(--color-border-subtle);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-2);
}

.welcome-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  margin: 0 auto 16px;
  width: 72px;
  height: 72px;
  border-radius: var(--radius-xl);
  background: var(--color-primary-1);
  color: var(--color-primary-6);
}

.welcome-content h2 {
  font-size: 20px;
  margin-bottom: 8px;
  color: var(--color-fg);
  font-weight: 600;
}

.welcome-content p {
  color: var(--color-fg-muted);
  margin-bottom: 24px;
  line-height: var(--leading-normal);
  font-size: 14px;
}

.result-layout {
  flex: 1;
  display: flex;
  overflow: hidden;
}

.result-panel {
  overflow-y: auto;
  background: var(--color-bg-subtle);
  min-width: 200px;
  display: flex;
  flex-direction: column;
}

.result-header {
  padding: 10px 16px;
  background: var(--color-primary-1);
  color: var(--color-primary-7);
  font-size: 12px;
  font-weight: 500;
  border-bottom: 1px solid var(--color-primary-2);
  position: sticky;
  top: 0;
  z-index: 1;
}

.result-filter {
  margin-left: 4px;
  color: var(--color-fg-muted);
}

.empty-search,
.no-results {
  padding: 60px 20px;
  text-align: center;
  color: var(--color-fg-muted);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
}

.empty-icon {
  color: var(--color-fg-muted);
  opacity: 0.7;
}

.no-results p {
  margin-bottom: 0;
  color: var(--color-fg);
  font-size: 14px;
}

.hint {
  font-size: 12px;
  color: var(--color-fg-muted);
}

.result-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px 14px 16px;
}

.result-item {
  padding: 12px 14px;
  background: var(--color-card);
  border: 1px solid var(--color-border-subtle);
  border-radius: 10px;
  cursor: pointer;
  animation: hit-in 240ms var(--ease-out-quart) backwards;
  animation-delay: var(--delay);
  transition: border-color var(--duration-fast) var(--ease-out-quart),
              box-shadow var(--duration-fast),
              background var(--duration-fast);
}

.result-item:hover {
  border-color: var(--color-primary-3);
  box-shadow: var(--shadow-1);
}

.result-item.selected {
  background: var(--color-primary-1);
  border-color: var(--color-primary-6);
  box-shadow: 0 0 0 1px var(--color-primary-6), var(--shadow-1);
}

@keyframes hit-in {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.result-item-header {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 6px;
}

.file-icon {
  color: var(--color-primary-7);
  flex-shrink: 0;
}

.file-title {
  font-weight: 600;
  font-size: 14px;
  color: var(--color-fg);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.file-snippet {
  font-size: 13px;
  color: var(--color-fg-subtle);
  line-height: 1.6;
  margin-bottom: 8px;
  max-height: 60px;
  overflow: hidden;
}

.file-snippet :deep(mark) {
  background: var(--color-highlight-bg);
  color: var(--color-highlight-fg);
  border-radius: 2px;
  padding: 0 2px;
  font-weight: 500;
}

.file-meta {
  font-size: 11px;
  color: var(--color-fg-muted);
  display: flex;
  align-items: center;
  gap: 4px;
  flex-wrap: wrap;
}

.meta-path {
  font-family: var(--font-mono);
  max-width: 400px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.meta-sep {
  opacity: 0.4;
}

.meta-parser {
  background: var(--color-bg-muted);
  padding: 1px 6px;
  border-radius: var(--radius-xs);
  border: 1px solid var(--color-border-subtle);
  color: var(--color-fg-subtle);
}

.meta-actions {
  display: inline-flex;
  gap: 2px;
  margin-left: auto;
}

.meta-btn {
  background: none;
  border: none;
  cursor: pointer;
  padding: 4px;
  border-radius: var(--radius-xs);
  color: var(--color-fg-muted);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  opacity: 0.65;
  transition: opacity var(--duration-fast), background var(--duration-fast), color var(--duration-fast);
}

.meta-btn:hover {
  opacity: 1;
  background: var(--state-hover);
  color: var(--color-primary-7);
}

.splitter {
  width: 5px;
  flex-shrink: 0;
  background: var(--color-border-subtle);
  cursor: col-resize;
  transition: background var(--duration-fast);
}

.splitter:hover,
.splitter:active {
  background: var(--color-primary-6);
}

.preview-panel {
  flex: 1;
  min-width: 200px;
  display: flex;
  flex-direction: column;
  background: var(--color-surface);
  overflow: hidden;
  border-left: 1px solid var(--color-border-subtle);
}

.preview-header {
  padding: 10px 14px;
  background: var(--color-bg-subtle);
  border-bottom: 1px solid var(--color-border-subtle);
  font-size: 13px;
  font-weight: 600;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  color: var(--color-fg);
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
  color: var(--color-fg-muted);
  padding: 4px;
  border-radius: var(--radius-xs);
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  transition: background var(--duration-fast), color var(--duration-fast);
}

.preview-close:hover {
  background: var(--state-hover);
  color: var(--color-fg);
}

.preview-content {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

.preview-text pre {
  font-family: var(--font-sans);
  font-size: 14px;
  line-height: 1.8;
  white-space: pre-wrap;
  word-wrap: break-word;
  color: var(--color-fg);
  margin: 0;
}

.preview-text :deep(.hl) {
  background: var(--color-highlight-bg);
  color: var(--color-highlight-fg);
  font-weight: 600;
  padding: 0 2px;
  border-radius: 2px;
}

.preview-error {
  text-align: center;
  padding: 40px 20px;
  color: var(--color-fg-muted);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
}

.preview-error p {
  margin: 0;
}

.statusbar {
  padding: 0 16px;
  height: 32px;
  background: var(--color-surface);
  border-top: 1px solid var(--color-border-subtle);
  font-size: 12px;
  color: var(--color-fg-muted);
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.status-progress {
  color: var(--color-primary-7);
}

.status-info {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  overflow: hidden;
  text-overflow: ellipsis;
}

.status-icon {
  flex-shrink: 0;
  color: var(--color-primary-6);
}

.index-add {
  display: flex;
  gap: 8px;
}

.cli-install {
  margin-top: 16px;
  padding-top: 12px;
  border-top: 1px solid var(--color-border-subtle);
  display: flex;
  align-items: center;
  gap: 8px;
}

.indexing-hint {
  margin-left: 8px;
  color: var(--color-warning-6);
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
  margin: 0 0 8px;
  font-size: 14px;
  color: var(--color-fg);
  border-bottom: 1px solid var(--color-border-subtle);
  padding-bottom: 6px;
  font-weight: 600;
}

.detail-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 0;
  font-size: 13px;
  gap: 12px;
}

.detail-row span:first-child {
  color: var(--color-fg-muted);
  flex-shrink: 0;
}

.mono {
  font-family: var(--font-mono);
  font-size: 12px;
  word-break: break-all;
  text-align: right;
}
</style>
