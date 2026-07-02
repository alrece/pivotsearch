## ADDED Requirements

### Requirement: 单工作线程串行执行
系统必须用单工作线程串行执行索引任务（crossbeam-channel 发 Task 到唯一 worker）。这是 Tantivy 单 writer 强约束的必然要求——同一索引目录同时只能一个 writer，多 worker 并发写会损坏索引。

#### Scenario: 任务串行无冲突
- **WHEN** 短时间内对同一索引根产生 3 个 Update task
- **THEN** 单 worker 串行执行，无并发写冲突

#### Scenario: 不同索引根可并发（未来优化）
- **WHEN** 对两个不同索引根分别产生 Update task
- **THEN** v1 仍串行执行（共享单 worker）；架构预留每索引独立 worker 的优化空间，但同一索引内必须串行

### Requirement: Task 状态机
每个 Task 必须经历状态机：NotReady → Ready → Indexing → Finished。CancelAction 可选 Keep（保留索引）或 Discard（删除索引）。

#### Scenario: 正常生命周期
- **WHEN** 添加新索引根产生 Create task
- **THEN** NotReady → Ready（前置条件满足）→ Indexing（worker 取出执行）→ Finished

#### Scenario: 取消并保留
- **WHEN** 用户在 Indexing 中取消，cancel_action = Keep
- **THEN** 停止当前操作，保留已索引内容

#### Scenario: 取消并丢弃
- **WHEN** cancel_action = Discard
- **THEN** 停止操作并删除索引目录和 tree_index 记录

### Requirement: UPDATE 与 REBUILD 语义
Update action 执行增量更新（mtime 比对，只处理变化文件）。Rebuild action 执行全量重建（清空索引 + tree_index 后从头索引）。

#### Scenario: Update 增量
- **WHEN** 文件监听触发 Update task
- **THEN** 跑增量算法，只处理新增/修改/删除的文件

#### Scenario: Rebuild 全量重建
- **WHEN** 用户手动点"重建索引"或 schema 变更后
- **THEN** 清空 Tantivy 索引目录和 tree_index，重新解析索引根所有文件

### Requirement: 任务去重与重叠检测
系统必须对入队任务做去重和重叠检测：(1) 同 index_id 已有 Ready Update task → 新 Update 冗余丢弃；(2) 新索引根路径与 registry/queue 中已有索引 contains 互含 → 拒绝（避免重复索引）。

#### Scenario: 冗余 Update 去重
- **WHEN** 队列已有 index_id=X 的 Ready Update task，又入队 index_id=X 的 Update
- **THEN** 新 Update 判定为冗余丢弃，不重复执行

#### Scenario: 路径包含检测
- **WHEN** 已索引 `/home/foo/docs`，用户尝试添加 `/home/foo/docs/sub`（被包含）
- **THEN** 拒绝添加，提示"该路径已被现有索引 /home/foo/docs 包含"

#### Scenario: 路径反向包含
- **WHEN** 已索引 `/home/foo/docs/sub`，用户尝试添加 `/home/foo/docs`（包含已有）
- **THEN** 拒绝添加，提示"该路径包含现有索引 /home/foo/docs/sub，请先移除"

### Requirement: SUCCESS_UNCHANGED 跳过持久化
当 Task 执行返回 SuccessUnchanged（无任何文件变化）时，必须跳过 SQLite 持久化和 reader reopen，节省 IO。

#### Scenario: 无变化跳过
- **WHEN** Update task 扫描后发现所有文件 mtime 未变，返回 SuccessUnchanged
- **THEN** 不触发 tree_index 写入、不 reopen reader，直接 Finished

#### Scenario: 有变化持久化
- **WHEN** Update task 返回 SuccessChanged
- **THEN** 触发 tree_index SQLite 写入 + reader reopen，保证查询可见最新数据
