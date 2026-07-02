## ADDED Requirements

### Requirement: Tauri 命令桥接
系统必须通过 Tauri 2 的 `#[tauri::command]` 暴露 Rust 后端能力给前端：add_index（添加索引根）、search（搜索）、get_preview（获取预览）、list_indexes（列出索引）、remove_index（移除索引）、rebuild_index（重建索引）、get_status（获取索引/监听状态）。

#### Scenario: 添加索引根
- **WHEN** 前端调用 `invoke('add_index', { path: '/home/foo/docs' })`
- **THEN** 后端创建索引根记录，触发后台线程索引，返回 index_id

#### Scenario: 搜索查询
- **WHEN** 前端调用 `invoke('search', { query: '报告', filters: {...}, page: 0 })`
- **THEN** 后端返回 SearchResponse（total_hits + results + page_count）

### Requirement: 后台线程与进度推送
索引操作必须在后台线程执行，不阻塞 Tauri 命令返回。进度通过 `app.emit("index-progress", payload)` 推送，前端 `listen("index-progress", ...)` 更新 UI。

#### Scenario: 索引进度实时显示
- **WHEN** 索引一个含 1000 文件的大目录
- **THEN** 后台线程每处理 N 个文件 emit 一次进度（如 "已处理 320/1000"），前端进度条实时更新

#### Scenario: 索引完成通知
- **WHEN** 索引完成或失败
- **THEN** emit "index-complete" 事件，前端显示完成/错误状态

#### Scenario: 命令立即返回
- **WHEN** 调用 add_index 触发后台索引
- **THEN** 命令立即返回 index_id（不等索引完成），用户无需等待即可做其他操作

### Requirement: 即时搜索
搜索框必须实现即时搜索：用户输入 200ms debounce 后自动触发查询（无需按 Enter），结果区显示加载骨架，结果返回后渲染。

#### Scenario: 即时搜索触发
- **WHEN** 用户在搜索框输入 "报告"（200ms 内未再输入）
- **THEN** 自动触发查询，显示结果

#### Scenario: 连续输入去抖
- **WHEN** 用户快速输入 "季度报告"（每个字符间隔 <200ms）
- **THEN** 只在停止输入 200ms 后触发一次查询（不每个字符都查）

#### Scenario: 搜索中状态
- **WHEN** 查询进行中
- **THEN** 显示加载骨架，旧结果淡化但不清空（避免闪烁）

### Requirement: 结果列表与高亮
结果列表必须显示每项的：文件名、命中片段（高亮关键词）、路径、大小、修改时间。支持虚拟滚动以处理万级结果。

#### Scenario: 结果项展示
- **WHEN** 查询返回结果
- **THEN** 每项显示：📄 file.md / ...命中片段（关键词黄底高亮）/ /path/to/file 2KB 2024-01-01

#### Scenario: 虚拟滚动
- **WHEN** 查询返回 5000 条结果
- **THEN** 虚拟滚动只渲染可视区域约 20-30 项，滚动流畅无卡顿

### Requirement: 预览面板
点击结果项必须展开预览面板，重新解析原文件渲染（与索引内容解耦，保证最新），高亮命中位置。

#### Scenario: 预览文档
- **WHEN** 点击一个 md 结果项
- **THEN** 右侧预览面板打开，重新解析原文件，显示渲染后内容，命中关键词高亮，自动滚动到第一个命中位置

#### Scenario: 预览 PDF
- **WHEN** 点击一个 PDF 结果项
- **THEN** 预览面板用 pdfium 渲染 PDF 页面，命中位置高亮

#### Scenario: 文件不存在降级
- **WHEN** 预览时原文件已被删除或移动（可移动介质场景）
- **THEN** 显示"文件不可访问"，但索引中的元信息（文件名/路径/snippet）仍展示

### Requirement: 索引管理面板
系统必须提供索引管理面板：列出所有索引根（路径 + 文件数 + 索引大小 + 状态），支持添加、删除、重建、启停监听。

#### Scenario: 列出索引
- **WHEN** 打开索引管理
- **THEN** 显示所有索引根卡片：/home/foo/docs（1234 文件，56MB，监听中）

#### Scenario: 添加索引根
- **WHEN** 点"添加"，选择目录 /home/bar/notes
- **THEN** 创建索引根，后台开始索引，状态显示"索引中（已处理 0/...）"

#### Scenario: 删除索引根
- **WHEN** 点某索引根的"删除"并确认
- **THEN** 删除 Tantivy 索引目录 + tree_index 记录，列表移除

#### Scenario: 重建索引
- **WHEN** 点"重建"
- **THEN** 清空该索引根的索引，从头全量重建

### Requirement: 筛选器
搜索结果必须支持筛选器：类型（多选 PDF/DOCX/MD...）、大小范围、索引根（多选）。

#### Scenario: 类型筛选
- **WHEN** 查询后勾选筛选器"仅 PDF"
- **THEN** 结果只显示 type=pdf 的命中

#### Scenario: 多索引筛选
- **WHEN** 勾选"工作目录"和"参考资料"两个索引根
- **THEN** 只搜索这两个索引根

### Requirement: 设置
系统必须提供设置页：OCR 开关（feature gate 启用时）、文本/HTML 扩展名配置、排除规则、监听间隔、语言包管理。

#### Scenario: OCR 开关
- **WHEN** 设置页切换 OCR 开关（首次开启）
- **THEN** 提示下载语言包，下载完成后 ImageOcrParser 生效

#### Scenario: 扩展名配置
- **WHEN** 用户在"文本扩展名"添加 .log
- **THEN** .log 文件被 TextParser 处理（默认可能不在列表）
