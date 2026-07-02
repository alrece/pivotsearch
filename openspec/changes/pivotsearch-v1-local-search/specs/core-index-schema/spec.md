## ADDED Requirements

### Requirement: Tantivy schema 八字段定死
系统必须在启动时一次性定义不可变的 Tantivy schema，包含 8 个字段：uid（Str，主键）、content（Text，jieba 分词，不存）、title（Text，存）、author（Text 多值，存）、type（Str，扩展名）、parser（Str，解析器名）、size（I64，字节）、last_modified（I64，毫秒时间戳）、index_id（Str，所属索引根）。

#### Scenario: schema 字段类型正确
- **WHEN** 系统初始化构建 schema
- **THEN** uid/type/parser/index_id 用 raw tokenizer（精确匹配），content/title/author 用 jieba tokenizer（分词），size/last_modified 用 I64（数值范围查询）

#### Scenario: schema 不可变约束
- **WHEN** 尝试在运行时新增或修改字段
- **THEN** 编译期拒绝（schema 由 builder 构建后不可变），文档明确说明字段演进需 reindex

### Requirement: uid 主键算法
uid 必须为 `file://{canonical_path}` 格式，其中 canonical_path 由 `std::fs::canonicalize` 规范化。uid 作为文档主键，用于 delete 和 update 操作。

#### Scenario: uid 规范化
- **WHEN** 索引文件 `/Users/foo/./docs/../readme.md`
- **THEN** uid 为 `file:///Users/foo/readme.md`（解析符号链接和相对路径）

#### Scenario: uid 唯一性
- **WHEN** 同一路径的文件被重复索引
- **THEN** uid 相同，第二次索引触发 upsert（delete_term + add），不产生重复文档

### Requirement: Document 组装策略
content 字段必须追加 title、author、文件名（带扩展名和不带扩展名两个版本，因为 jieba 不在点处切分），通过多次 add_text 实现拼接（Tantivy 多值等效 Lucene 拼接）。

#### Scenario: content 追加元数据
- **WHEN** 解析一个 docx 文件，title="季度报告"，author=["张三"]，文件名="report.docx"
- **THEN** content 字段实际索引文本为 `{正文} 季度报告 张三 report.docx report`（正文 + 元数据 + 文件名两版本，空格分隔）

#### Scenario: title 缺失退化
- **WHEN** 解析的文件无 title 元数据
- **THEN** title 字段使用去扩展名的文件名（report.docx → "report"）

### Requirement: upsert 语义（delete_term + add）
Tantivy 无原生 upsert，update 必须实现为 `delete_term(Term::from_field_text(uid_field, uid))` + `add_document(doc)`，且 delete_term 在 commit 后才对 reader 生效。

#### Scenario: update 后 reader 可见
- **WHEN** 修改文件后触发 update（delete + add），随后 commit + reopen reader
- **THEN** 新查询返回更新后的内容，旧版本不再命中

#### Scenario: 未 commit 的 delete 不可见
- **WHEN** 执行 delete_term 后但未 commit，期间查询
- **THEN** 旧文档仍可被搜到（delete 尚未生效），commit 后才真正删除
