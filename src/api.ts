// Tauri IPC 桥接：前端调用 Rust #[tauri::command]
// 封装所有后端命令，提供类型安全的调用接口。

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ── 类型定义（与 Rust 端对齐）──

export interface SearchResult {
  uid: string;
  path: string;
  title: string;
  snippet: string;
  score: number;
  size: number;
  last_modified: number;
  parser: string;
  index_id: string;
}

export interface SearchResponse {
  total_hits: number;
  results: SearchResult[];
  page: number;
  page_count: number;
}

export interface SearchFilters {
  index_ids?: string[] | null;
  parsers?: string[] | null;
  min_size?: number | null;
  max_size?: number | null;
}

export interface IndexInfo {
  id: string;
  path: string;
  display_name: string | null;
  file_count: number;
}

export interface PreviewData {
  uid: string;
  path: string;
  parser: string;
  content: string;
  exists: boolean;
}

// ── 命令封装 ──

export async function addIndex(path: string): Promise<string> {
  return invoke<string>("add_index", { path });
}

export async function search(
  query: string,
  filters: SearchFilters | null,
  page: number,
  caseSensitive: boolean
): Promise<SearchResponse> {
  return invoke<SearchResponse>("search", {
    query,
    filters,
    page,
    caseSensitive,
  });
}

export async function getPreview(uid: string): Promise<PreviewData> {
  return invoke<PreviewData>("get_preview", { uid });
}

export async function listIndexes(): Promise<IndexInfo[]> {
  return invoke<IndexInfo[]>("list_indexes");
}

export async function removeIndex(id: string): Promise<void> {
  return invoke<void>("remove_index", { id });
}

export async function rebuildIndex(id: string): Promise<void> {
  return invoke<void>("rebuild_index", { id });
}

// ── 事件监听 ──

export interface IndexProgress {
  index_id: string;
  processed: number;
  total: number;
  message: string;
}

export function onIndexProgress(
  callback: (progress: IndexProgress) => void
): Promise<UnlistenFn> {
  return listen<IndexProgress>("index-progress", (event) => {
    callback(event.payload);
  });
}

// ── 文件操作 ──

export async function copyToClipboard(text: string): Promise<void> {
  return invoke<void>("copy_to_clipboard", { text });
}

export async function openInFolder(path: string): Promise<void> {
  return invoke<void>("open_in_folder", { path });
}

export async function installCli(): Promise<string> {
  return invoke<string>("install_cli");
}
