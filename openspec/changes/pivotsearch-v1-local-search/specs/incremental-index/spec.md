## ADDED Requirements

### Requirement: mtime 驱动的增量判定
系统必须以文件 mtime（修改时间）作为增量判定的主依据，不用内容 hash。文件变化判定：`is_modified = old_mtime != new_mtime`。

#### Scenario: 未改文件跳过
- **WHEN** 二次索引时文件 mtime 未变
- **THEN** 跳过解析与索引写入，不产生 IO 开销

#### Scenario: 修改文件触发重索引
- **WHEN** 文件内容被编辑保存，mtime 变化
- **THEN** 触发 upsert（delete_term uid + add 新文档）

### Requirement: 文件树 diff 增量算法
系统必须用 unseenDocs/unseenSubFolders 集合 diff 算法处理增量：进入目录前克隆已索引文件/子目录集合，遍历中每见到一个就 remove，遍历完后剩余的即为磁盘上已删除的，执行删除。

#### Scenario: 新增文件
- **WHEN** 目录新增了 `new.md`（tree_index 中无记录）
- **THEN** 解析 new.md 并 add_document

#### Scenario: 修改文件
- **WHEN** `existing.md` 的 mtime 变化（在 tree_index 中有记录但 mtime 不同）
- **THEN** delete_term(uid) + add_document

#### Scenario: 删除文件
- **WHEN** `gone.md` 从磁盘删除（遍历中未见到，留在 unseen_docs）
- **THEN** delete_term(uid) 并从 tree_index 移除

#### Scenario: 删除子目录递归
- **WHEN** 整个子目录被删除
- **THEN** 递归遍历该子树，对所有文档执行 delete_term

### Requirement: 未改归档整体跳过
系统必须对归档文件（zip/tar/7z）实现整体跳过优化：当归档文件 mtime 未变时，不递归解包解析内部文件，直接返回 SuccessUnchanged。

#### Scenario: 归档未改跳过
- **WHEN** 二次索引时 `archive.zip` mtime 未变
- **THEN** 不解压，直接跳过（含内部 N 个文件全部跳过），大幅节省 IO

#### Scenario: 归档改了重新解析
- **WHEN** `archive.zip` 被替换，mtime 变化
- **THEN** 解包到临时目录，递归解析内部所有文件

### Requirement: tree_index SQLite 持久化
系统必须用 SQLite（rusqlite）持久化 tree_index（已索引文件树状态），替代 Java 序列化。表结构：index_roots（id/path/display_name/created_at）+ indexed_files（uid/path/mtime/parser/index_id）。

#### Scenario: 持久化与恢复
- **WHEN** 索引完成后关闭程序，重新启动
- **THEN** tree_index 从 SQLite 恢复，下次增量判定基于持久化的 mtime

#### Scenario: 解析失败文件保留记录
- **WHEN** 某文件解析失败（损坏的 PDF）
- **THEN** 仍记录在 tree_index（mtime + parser=null），下次不重复尝试解析（避免反复重试坏文件）

### Requirement: 增量结果三态
增量更新必须返回三态枚举：SuccessChanged（有变化，需 save）、SuccessUnchanged（无变化，跳过 save 省 IO）、Failure（错误，记录但不崩溃）。

#### Scenario: SuccessUnchanged 跳过 save
- **WHEN** 整个索引根 mtime 未变，无任何文件变化
- **THEN** 返回 SuccessUnchanged，不触发 SQLite 持久化和 reader reopen

#### Scenario: SuccessChanged 触发持久化
- **WHEN** 有文件新增/修改/删除
- **THEN** 返回 SuccessChanged，触发 tree_index SQLite 写入 + reader reopen
