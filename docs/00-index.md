# pivotsearch 技术文档索引

> 本目录是技术决策的事实记录。L1 数据源（最高优先级）。

| 文档 | 内容 | 状态 |
|---|---|---|
| [00-index.md](00-index.md) | 本索引 | ✅ |
| [01-overview.md](01-overview.md) | 项目概述、问题陈述、定位 | ✅ |
| [02-architecture.md](02-architecture.md) | 架构设计、crate 组织、依赖方向、数据流 | ✅ |
| [03-tech-selection.md](03-tech-selection.md) | 技术选型：为何选 Tantivy/pdfium/notify 等 | ✅ |
| [04-design-logic.md](04-design-logic.md) | 从 DocFetcher 借鉴的设计逻辑（净室） | ✅ |

阅读顺序：01 → 02 → 03 → 04。

## 与其他文档的关系

- **AGENTS.md**：仓库宪法（开发约定/命令/合规红线），docs/ 是其技术细节展开
- **openspec/**：规格驱动开发（Requirement + Scenario），docs/ 是其设计背景
- **.loop/**：工程方法论状态，docs/ 是其 L1 数据源
