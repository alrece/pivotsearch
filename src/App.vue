<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import {
  search as searchApi,
  listIndexes,
  addIndex,
  onIndexProgress,
  type SearchResult,
  type IndexInfo,
  type IndexProgress,
} from "./api";

// ── 搜索状态 ──
const query = ref("");
const results = ref<SearchResult[]>([]);
const totalHits = ref(0);
const loading = ref(false);
const currentPage = ref(0);
let searchTimer: ReturnType<typeof setTimeout> | null = null;

// ── 索引管理状态 ──
const indexes = ref<IndexInfo[]>([]);
const showIndexPanel = ref(false);
const newPath = ref("");
const progressMsg = ref("");
let unlistenProgress: (() => void) | null = null;

// 即时搜索（200ms debounce）
function onSearchInput() {
  if (searchTimer) clearTimeout(searchTimer);
  if (!query.value.trim()) {
    results.value = [];
    totalHits.value = 0;
    return;
  }
  loading.value = true;
  searchTimer = setTimeout(doSearch, 200);
}

async function doSearch() {
  try {
    const resp = await searchApi(query.value, null, currentPage.value);
    results.value = resp.results;
    totalHits.value = resp.total_hits;
  } catch (e) {
    console.error("搜索失败", e);
  } finally {
    loading.value = false;
  }
}

function onKeydown(e: KeyboardEvent) {
  // Ctrl/Cmd+Enter 精确搜索
  if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
    if (searchTimer) clearTimeout(searchTimer);
    doSearch();
  }
}

// 高亮渲染：把 snippet 的 <b> 标签转成 <mark>
function renderSnippet(snippet: string): string {
  return snippet.replace(/<b>/g, '<mark>').replace(/<\/b>/g, "</mark>");
}

// 格式化大小
function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

// ── 索引管理 ──
async function refreshIndexes() {
  try {
    indexes.value = await listIndexes();
  } catch (e) {
    console.error("获取索引列表失败", e);
  }
}

async function onAddIndex() {
  if (!newPath.value.trim()) return;
  try {
    progressMsg.value = "正在添加索引...";
    await addIndex(newPath.value);
    newPath.value = "";
    progressMsg.value = "索引添加成功";
    setTimeout(() => (progressMsg.value = ""), 2000);
    await refreshIndexes();
  } catch (e) {
    progressMsg.value = `添加失败: ${e}`;
  }
}

onMounted(async () => {
  await refreshIndexes();
  unlistenProgress = await onIndexProgress((p: IndexProgress) => {
    progressMsg.value = p.message;
  });
});

onUnmounted(() => {
  if (unlistenProgress) unlistenProgress();
});
</script>

<template>
  <div class="app">
    <!-- 顶部工具栏 -->
    <header class="toolbar">
      <div class="search-box">
        <el-input
          v-model="query"
          placeholder="搜索文件内容...（即时搜索）"
          size="large"
          :prefix-icon="'🔍'"
          @input="onSearchInput"
          @keydown="onKeydown"
          clearable
        />
      </div>
      <el-button @click="showIndexPanel = !showIndexPanel">索引管理</el-button>
    </header>

    <!-- 索引管理面板（折叠） -->
    <div v-if="showIndexPanel" class="index-panel">
      <h3>索引根列表</h3>
      <el-table :data="indexes" stripe style="width: 100%">
        <el-table-column prop="display_name" label="名称" width="150" />
        <el-table-column prop="path" label="路径" />
        <el-table-column prop="file_count" label="文件数" width="80" />
      </el-table>
      <div class="add-index">
        <el-input v-model="newPath" placeholder="输入目录路径" />
        <el-button type="primary" @click="onAddIndex">添加索引</el-button>
      </div>
      <p v-if="progressMsg" class="progress-msg">{{ progressMsg }}</p>
    </div>

    <!-- 主体：结果列表 + 预览 -->
    <main class="main">
      <div class="result-list" v-loading="loading">
        <p v-if="query && !loading && results.length === 0" class="empty">
          无结果。试试更宽泛的关键词。
        </p>
        <div
          v-for="(r, i) in results"
          :key="r.uid"
          class="result-item"
        >
          <div class="result-title">{{ i + 1 }}. {{ r.title }}</div>
          <div class="result-snippet" v-html="renderSnippet(r.snippet)"></div>
          <div class="result-meta">
            <span class="meta-path">{{ r.path }}</span>
            <span class="meta-sep">·</span>
            <span>{{ r.parser }}</span>
            <span class="meta-sep">·</span>
            <span>{{ formatSize(r.size) }}</span>
          </div>
        </div>
      </div>
    </main>

    <!-- 状态栏 -->
    <footer v-if="totalHits > 0" class="status-bar">
      命中 {{ totalHits }} 条结果
    </footer>
  </div>
</template>

<style>
:root {
  --ps-primary: #3b5bdb;
  --ps-bg: #fafafa;
  --ps-surface: #ffffff;
  --ps-text: #1a1a1a;
  --ps-text-secondary: #6b6b6b;
  --ps-border: #e5e5e5;
  --ps-highlight: #fff3bf;
}

* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  font-family: -apple-system, "Segoe UI", "Noto Sans SC", sans-serif;
  color: var(--ps-text);
  background: var(--ps-bg);
}

.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
}

.toolbar {
  display: flex;
  gap: 8px;
  padding: 12px 16px;
  background: var(--ps-surface);
  border-bottom: 1px solid var(--ps-border);
}

.search-box {
  flex: 1;
}

.index-panel {
  padding: 16px;
  background: var(--ps-surface);
  border-bottom: 1px solid var(--ps-border);
}

.add-index {
  display: flex;
  gap: 8px;
  margin-top: 12px;
}

.progress-msg {
  margin-top: 8px;
  color: var(--ps-text-secondary);
  font-size: 13px;
}

.main {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

.result-list {
  max-width: 900px;
  margin: 0 auto;
}

.result-item {
  padding: 12px 16px;
  margin-bottom: 8px;
  background: var(--ps-surface);
  border: 1px solid var(--ps-border);
  border-radius: 6px;
  cursor: pointer;
  transition: box-shadow 0.15s;
}

.result-item:hover {
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
}

.result-title {
  font-weight: 600;
  margin-bottom: 4px;
}

.result-snippet {
  font-size: 14px;
  color: var(--ps-text-secondary);
  margin-bottom: 4px;
  line-height: 1.5;
}

.result-snippet mark {
  background: var(--ps-highlight);
  padding: 0 2px;
  border-radius: 2px;
  color: var(--ps-text);
}

.result-meta {
  font-size: 12px;
  color: var(--ps-text-secondary);
}

.meta-path {
  font-family: "SF Mono", "Cascadia Code", monospace;
}

.meta-sep {
  margin: 0 6px;
  opacity: 0.5;
}

.empty {
  text-align: center;
  color: var(--ps-text-secondary);
  padding: 40px;
}

.status-bar {
  padding: 8px 16px;
  background: var(--ps-surface);
  border-top: 1px solid var(--ps-border);
  font-size: 13px;
  color: var(--ps-text-secondary);
}
</style>
