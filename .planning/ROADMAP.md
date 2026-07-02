# pivotsearch — Roadmap

> 按 Phase 划分，每个 Phase 含若干 Plan，逐 Plan 执行 + 对抗门。
> 任务编号 T0-T14 对应 `openspec/.../tasks.md`。

## Phase 划分

### Phase 0: 工程脚手架
**目标：** 全套方法论框架 + Cargo workspace 骨架可编译

- T0 方法论框架（.loop/ openspec/ .planning/）+ 顶层文件（AGENTS/CLAUDE/README/LICENSE/DESIGN）
- T0 Cargo workspace（9 crate 骨架）+ Tauri 项目骨架

**验收：** `cargo check` 通过 + 净室 grep 无 DocFetcher 残留 + `.loop/STATE.yaml` 初始化 + 8 个 spec 完成

### Phase 1: 核心索引闭环（Lane A 后端核心）
**目标：** CLI 能索引目录并查询，最小闭环跑通

- T1 契约层（contracts crate：Parser/Indexer/Searcher/Watcher trait + 数据结构 + 错误）
- T1 Tantivy schema（index crate：8 字段 + uid + Document 组装）
- T2 解析层 MVP（parser crate：5 高频格式 txt/md/html/pdf/docx/xlsx + 注册表两级选择）
- T3 查询闭环（search crate：单索引查询 + 高亮 + 分页；core crate 组装；cli binary）

**验收：** `pivotsearch index <dir>` 索引 txt/md/docx/xlsx/pdf，`pivotsearch search <query>` 返回带高亮结果

### Phase 2: 增量与监听（Lane A 增强）
**目标：** 文件改动自动增量更新

- T4 增量索引（index crate：mtime 比对 + unseenDocs diff + 归档跳过 + SQLite tree_index）
- T5 文件监听（watcher crate：notify + 防抖 + 过滤 + mtime 校验）
- T6 任务队列（queue crate：单工作线程 + Task 状态机 + UPDATE/REBUILD + 去重）

**验收：** CLI 启动监听目录，编辑文件后自动增量更新，二次查询命中新内容

### Phase 3: 解析补全 + 多索引（Lane A 完善）
**目标：** 全格式覆盖 + 多目录跨索引搜索

- T7 全格式解析（pptx/epub/归档穿透；.doc/.ppt 不支持提示）
- T8 多索引合并搜索（每索引独立 Searcher + index_id 过滤 + size range + type 过滤）

**验收：** 多目录索引 + 跨索引查询 + 筛选器生效 + pptx/epub/zip 内文件可搜

### Phase 4: OCR + 桌面 UI（Lane B）
**目标：** 三端桌面应用端到端可用

- T9 OCR 管道（ocr crate feature gate：Tesseract + 图片/扫描件 + 语言包按需下载）
- T10 Tauri 桥接 + 核心 UI（#[tauri::command] + 后台线程 + 进度 emit + 搜索视图）
- T11 UI 完善（索引管理 + 预览 + 筛选器 + 设置）

**验收：** `cargo tauri dev` 端到端：添加索引→后台索引→即时搜索→预览；OCR 可选启用

### Phase 5: 打磨与发布（Lane C）
**目标：** 三端安装包可分发

- T12 三端打包 CI（GitHub Actions matrix + PDFium 静态链接 + 性能压测）
- T13 文档与中文调优（用户/开发文档 + jieba 词典/停用词调优）
- T14 净室合规 + 对抗门自动化（全量 grep + 对抗脚本 + 覆盖率 ≥95%）

**验收：** 三端安装包可用 + 文档完整 + 覆盖率 ≥95%

## 依赖关系

```
Phase 0 (脚手架) ──→ Phase 1 (核心闭环) ──→ Phase 2 (增量监听) ──→ Phase 3 (补全多索引)
                                                                    │
                                                                    ↓
                                              Phase 4 (OCR+UI) ←─────┘
                                                    │
                                                    ↓
                                              Phase 5 (打包发布)
```

Phase 0 → 1 → 2 → 3 严格串行（后端核心逐步构建）。Phase 4 依赖 Phase 3（全格式 + 多索引就绪）。Phase 5 依赖 Phase 4（UI 可用才能打包）。

## 工作量预估（诚实）

数月级工程。各 Phase 预估：
- Phase 0：1-2 天（当前进行中）
- Phase 1：1-2 周
- Phase 2：1-2 周
- Phase 3：1 周
- Phase 4：2-3 周
- Phase 5：1 周

按 Loop Engineering 增量迭代，每 Phase retro 产 next_seed，不一次性铺开。
