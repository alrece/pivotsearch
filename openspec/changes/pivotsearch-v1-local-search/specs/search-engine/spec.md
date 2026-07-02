## ADDED Requirements

### Requirement: 多索引合并搜索
系统必须支持跨多个索引根的合并搜索：每个索引根一个 Tantivy Searcher，各跑 top-N 查询，手动合并取全局 top-N。当 SearchRequest.index_ids 为 None 时搜索全部索引。

#### Scenario: 搜全部索引
- **WHEN** 用户查询"报告"，index_ids=None（共 3 个索引根）
- **THEN** 3 个索引各自查询 top-50，合并后取全局 top-50 返回

#### Scenario: 限定索引搜索
- **WHEN** 用户查询"报告"，index_ids=["工作目录", "参考资料"]
- **THEN** 只搜索这两个索引根，合并 top-N

#### Scenario: 单索引损坏不致命
- **WHEN** 3 个索引中第 2 个目录损坏（Index::open 失败）
- **THEN** 跳过损坏索引，搜索其余 2 个，记录警告，不崩溃

### Requirement: 查询解析
系统必须用 tantivy QueryParser 解析用户查询，默认 AND 操作符，支持 term/phrase/boolean/wildcard/prefix 查询，允许 leading wildcard。

#### Scenario: AND 默认
- **WHEN** 用户查询 "季度 报告"（两个词空格分隔）
- **THEN** 默认按 AND 处理，只返回同时含"季度"和"报告"的文档

#### Scenario: 短语查询
- **WHEN** 用户查询 "季度报告"（jieba 分词为短语）
- **THEN** 按短语查询，返回连续出现"季度报告"的文档（相关性更高）

#### Scenario: 通配符查询
- **WHEN** 用户查询 "report*"
- **THEN** 匹配 report/reports/reporting 等前缀词

### Requirement: 中文分词
系统必须用 jieba-rs 作为中文 tokenizer，实现自定义 Tantivy Tokenizer（不依赖 tantivy-jieba 版本同步）。content/title/author 字段用 jieba tokenizer。

#### Scenario: 中文分词
- **WHEN** 索引含 "我爱自然语言处理" 的文档
- **THEN** jieba 分词为 我/爱/自然/语言/处理 等词，查询"自然语言"可命中

#### Scenario: 中英混排
- **WHEN** 文档含 "使用 React 开发前端"
- **THEN** jieba 分词处理好中文部分，英文 React 作为整体 token，混合查询"React 前端"可命中

#### Scenario: 自定义词典
- **WHEN** 用户添加自定义词典含 "pivotsearch" 作为专有名词
- **THEN** "pivotsearch 是好工具" 分词为 pivotsearch/是/好/工具，不再被拆散

### Requirement: 分页
系统必须支持分页，PAGE_SIZE=50。每页通过 `collector::Top(k = (page+1) * PAGE_SIZE)` 取后再切片实现。

#### Scenario: 第一页
- **WHEN** 查询返回 120 条命中，请求 page=0
- **THEN** 返回前 50 条，page_count=3

#### Scenario: 翻页
- **WHEN** 请求 page=2（第三页）
- **THEN** Top(150) 取后切片第 101-120 条（最后一页只有 20 条）

### Requirement: SnippetGenerator 高亮
系统必须用 Tantivy SnippetGenerator 生成命中片段并标记高亮范围，统一处理短语和普通查询（替代 Lucene 的 FastVector/标准双高亮路径）。

#### Scenario: 生成命中片段
- **WHEN** 文档 content 含 "本季度营收增长20%"，查询 "营收"
- **THEN** snippet 为 "本季度**营收**增长20%"（营收被标记高亮范围）

#### Scenario: 无命中字段空 snippet
- **WHEN** 某文档命中但命中位置无法生成片段
- **THEN** snippet 为空或文件名，前端优雅显示

### Requirement: 过滤器
系统必须支持过滤器：类型过滤（parser 字段 TermsQuery）、大小范围（size 字段 RangeQuery）、索引过滤（index_id 字段 TermQuery）。

#### Scenario: 类型过滤
- **WHEN** 查询 "报告" 且 parsers=["PdfParser"]
- **THEN** 只返回 PDF 类型的命中

#### Scenario: 大小范围
- **WHEN** 查询 "报告" 且 min_size=1024, max_size=1048576（1KB-1MB）
- **THEN** 只返回大小在该范围的命中

### Requirement: 搜索中断
系统必须支持搜索中断（stopSearch）：通过 AtomicBool 标志，搜索循环中检查，置位则提前返回已收集的结果。

#### Scenario: 用户取消搜索
- **WHEN** 用户在搜索过程中点"停止"
- **THEN** AtomicBool 置位，搜索循环检测后立即返回已收集的部分结果
