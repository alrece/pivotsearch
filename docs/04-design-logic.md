# 04 — 从 DocFetcher 借鉴的设计逻辑（净室）

> 本文档记录从 DocFetcher（Java/Lucene，GPL v3）借鉴的**设计模式**。严格遵守净室红线：只复刻设计逻辑，不复制任何 Java 代码/类名/标识符。

## 借鉴的设计精华

### 1. 解析层与写入层解耦

**DocFetcher**：`ParseService` 产 `ParseResult`（纯文本）→ `LuceneDocWriter` 组装 Lucene Document。
**pivotsearch**：`Parser` trait 产 `ParseResult`（String）→ `index` crate 组装 Tantivy Document。
**价值**：解析与索引解耦，便于单元测试和替换分词器/解析器。

### 2. mtime 驱动的增量（不用 hash）

**DocFetcher**：`FileDocument.isModified()` = `getLastModified() != file.lastModified()`。
**pivotsearch**：`is_modified = old_mtime != new_mtime`。
**价值**：不用 hash，性能极佳。配合 notify 事件触发。

### 3. unseenDocs 文件树 diff 增量算法

**DocFetcher**：进入目录前克隆 `documentMap`，遍历中 `remove`，剩余的即磁盘上已删除的。
**pivotsearch**：同样 `HashMap` 克隆 + `remove`，剩余即删除。
**价值**：比"全删重建"高效，天然处理新增/修改/删除三类。

### 4. 未改归档整体跳过

**DocFetcher**：`isUnmodifiedArchive(folder, newMtime)` 为 true 则不递归。
**pivotsearch**：folder 节点存 `last_modified`，相等则 return。
**价值**：大归档不重复解压，大体量索引快速增量。

### 5. Parser 注册表 + 两级选择

**DocFetcher**：`ParseService.parsers` 硬编码列表，`parse()` 先 mime 检测（多 parser 容错尝试）后扩展名匹配。
**pivotsearch**：`Vec<Box<dyn Parser>>` + `infer` crate(mime) + 扩展名 map；mime 优先，失败换下一个 parser。
**价值**：真实世界乱命名文件容错。

### 6. 一索引根一目录 + 独立元数据

**DocFetcher**：每索引根对应 `indexParentDir/<rootName>_<timestamp>/`，含 Lucene segments + tree-index.ser。
**pivotsearch**：每索引根一目录，含 Tantivy 索引 + tree_index.sqlite。
**改进**：用 SQLite 替代 Java 序列化（DocFetcher 的 ser 跨版本脆弱 + StackOverflow）。

### 7. uid 主键 + parser 名入索引

**DocFetcher**：`uid = "file://" + path`，`parser` 字段存解析器类名（预览时决定渲染方式）。
**pivotsearch**：`uid = format!("file://{}", canonical_path)`，`parser` 字段存解析器名。
**价值**：预览时按 parser 名决定如何重新渲染；upsert 主键。

### 8. watcher 防抖 + mtime 二次校验

**DocFetcher**：`DelayedExecutor(1000ms)` 防抖 + `sameLastModified()` 校验过滤 JNotify 访问误报。
**pivotsearch**：`notify-debouncer-full`（1s 单 flight）+ 查 tree_index mtime 校验。
**价值**：杜绝编辑器保存触发 N 次事件引发 N 次重排；过滤纯访问误报。

### 9. 解析失败文件仍留在树里

**DocFetcher**：parser 失败的文件仍入树（避免下次重复重试坏文件）。
**pivotsearch**：tree_index 记录 mtime + parser=null，下次不重复解析。
**价值**：坏文件不阻塞整体索引。

## DocFetcher 缺失、pivotsearch 补强

| 能力 | DocFetcher | pivotsearch |
|---|---|---|
| OCR | ❌ 完全无 | ✅ Tesseract（feature gate） |
| 元数据持久化 | Java 序列化（脆弱） | SQLite（可查询可恢复） |
| GUI 框架 | SWT（单线程） | Tauri（进程分离，IPC 清晰） |
| 索引并发 | 单工作线程 | 单 worker（同索引）+ 架构预留多索引并发 |
| 现代格式解析 | POI + PDFBox（Java） | 纯 Rust crate 组合 |

## 应避免照搬

- **Java 序列化存元数据**：跨版本脆，已换 SQLite
- **SWT 全 GUI 线程模型**：Tauri 进程分离更清晰
- **TrueZIP 虚拟文件系统**：Rust 无等价物，显式解包到临时目录
- **双高亮路径**（FastVector + 标准）：Tantivy 无 term-vector offset，统一用 SnippetGenerator

## 净室合规检查

每次产出后跑：
```bash
grep -ri "docfetcher\|net.sourceforge.docfetcher" crates/ src/ src-tauri/
# 无输出 = 通过；有输出 = 违反净室红线，必须改名
```
