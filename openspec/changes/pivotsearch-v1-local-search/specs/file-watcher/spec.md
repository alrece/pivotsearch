## ADDED Requirements

### Requirement: 跨平台文件监听
系统必须用 notify crate 实现跨平台文件监听（Linux inotify / macOS FSEvents / Windows ReadDirectoryChangesW），对每个索引根目录注册一个 watch。

#### Scenario: 检测到文件创建
- **WHEN** 在被监听的索引根目录下创建新文件 `note.md`
- **THEN** 收到 create 事件，触发该索引根的 Update task

#### Scenario: 检测到文件修改
- **WHEN** 编辑保存已存在的 `note.md`
- **THEN** 收到 modify 事件（经防抖和校验后），触发 Update task

#### Scenario: 检测到文件删除
- **WHEN** 删除 `note.md`
- **THEN** 收到 remove 事件，触发 Update task（增量算法判定删除并 delete_term）

### Requirement: 事件防抖（单 flight 合并）
系统必须用 notify-debouncer-full 实现 1 秒防抖窗口。编辑器保存常触发多次事件（写+改 mtime+其他），防抖窗口内的事件合并为单个 Update task。

#### Scenario: 多次保存合并
- **WHEN** 编辑器在 500ms 内触发 5 次 modify 事件（典型保存行为）
- **THEN** 防抖窗口合并后只产生 1 个 Update task

#### Scenario: 不同文件不互相阻塞
- **WHEN** 同一索引根下 fileA 和 fileB 几乎同时修改
- **THEN** 各自的修改都被检测到，最终 1 个 Update task 处理两者（增量算法扫描整个根）

### Requirement: 事件过滤
系统必须过滤掉不应触发更新的文件系统事件：(1) 索引目录自身的事件（防自反馈死循环），(2) MS Word 临时文件 `~$*.docx?`，(3) 用户配置的 exclude 规则，(4) 不可解析的文件（扩展名不在 parser 注册表）。

#### Scenario: 忽略索引目录自身事件
- **WHEN** Tantivy 索引写入触发索引目录下的文件变化
- **THEN** 事件被过滤（路径在索引目录内），不触发 Update，避免无限循环

#### Scenario: 忽略 Word 临时文件
- **WHEN** Word 打开文档时创建 `~$report.docx`
- **THEN** 事件被过滤，不触发更新

#### Scenario: 不可解析文件忽略
- **WHEN** 下载一个 `.exe` 文件到索引根（exe 不在 parser 注册表）
- **THEN** 事件被过滤（除非开启 index_filenames 仅索引文件名）

### Requirement: mtime 二次校验去噪
notify 在某些场景（尤其 macOS FSEvents）会对纯文件访问误报 modify 事件。系统必须对 modify 事件做 mtime 二次校验：查 tree_index 中该文件的 mtime，若相同则丢弃事件。

#### Scenario: 纯访问不触发更新
- **WHEN** 仅读取文件（cat/type）未修改，notify 误报 modify
- **THEN** 查 tree_index 发现 mtime 未变，丢弃事件，不触发 Update

#### Scenario: 真实修改通过校验
- **WHEN** 编辑保存文件，mtime 变化
- **THEN** 查 tree_index 发现 mtime 不同，保留事件，触发 Update

### Requirement: macOS FSEvents 容错
macOS FSEvents 不提供精确逐文件事件，可能合并或丢失。系统必须将 FSEvents 当作"该目录树有变化，需重扫"的提示，配合 mtime 校验兜底，而非依赖精确事件流。

#### Scenario: FSEvents 合并事件
- **WHEN** macOS 上短时间多个文件变化被 FSEvents 合并为单个目录级事件
- **THEN** 触发该索引根的完整重扫（增量算法扫描，mtime 校验决定哪些真正重索引）

#### Scenario: 非用户文件权限限制
- **WHEN** 监听不属于当前用户的文件（FSEvents 安全模型限制）
- **THEN** 事件可能丢失，定期全量扫描兜底（可配置扫描间隔）
