## ADDED Requirements

### Requirement: Parser trait 与注册表
系统必须定义 `Parser` trait（extensions/mimes/parse 方法）和一个全局注册表 `ParserRegistry`，包含所有支持的格式解析器。解析器选择采用两级策略。

#### Scenario: 扩展名精确匹配
- **WHEN** 解析 `report.pdf`，注册表中有 PdfParser 且扩展名含 "pdf"
- **THEN** 选中 PdfParser 进行解析

#### Scenario: mime 优先于扩展名
- **WHEN** 文件名为 `data.txt` 但魔数检测为 PDF（用户改了扩展名），且配置了 mime 检测规则
- **THEN** 走 mime 路径，选中 PdfParser（而非 TextParser）

#### Scenario: 多 parser 容错尝试
- **WHEN** mime 检测命中多个候选 parser，第一个解析失败
- **THEN** 自动尝试下一个候选 parser，直到成功或全部失败

### Requirement: 10 类格式解析器
系统必须实现以下格式的解析器，每个产出 ParseResult（content + title + authors + misc_metadata + parser_name）：

- PDF（pdfium-render，含中文支持）
- DOCX（docx-rs 或 ooxmlsdk）
- XLSX/XLS/CSV（calamine）
- PPTX（ooxmlsdk）
- Markdown（pulldown-cmark）
- HTML（scraper，提取正文去 script/style）
- TXT/源代码（encoding_rs + chardetng 编码检测）
- ePub（epub crate）
- 归档穿透（zip/tar/sevenz-rust，解包后递归解析）
- 图片 OCR（feature gate，见 ocr-pipeline）

#### Scenario: PDF 中文提取
- **WHEN** 解析一个含中文的 PDF（使用 CID 字体）
- **THEN** 通过 pdfium-render 提取出正确的中文明文（不乱码）

#### Scenario: GBK 编码文本
- **WHEN** 解析一个 GBK 编码的 .txt 文件
- **THEN** chardetng 检测编码，encoding_rs 转码为 UTF-8，content 为正确中文

#### Scenario: HTML 正文提取
- **WHEN** 解析一个 HTML 文件
- **THEN** content 为 body 正文（去除 script/style/nav），title 从 `<title>` 提取，author 从 `<meta name="author">` 提取

### Requirement: 不支持格式明确处理
MS Office 老格式 .doc/.ppt 在 v1 不支持（Rust 无成熟纯解析器）。系统检测到时必须返回 `UnsupportedFormat` 错误，UI 提示用户转换为 .docx/.pptx。

#### Scenario: .doc 文件提示转换
- **WHEN** 索引遇到 `old.doc` 文件
- **THEN** 返回 `PivotsearchError::UnsupportedFormat("doc")`，UI 显示"不支持的格式 .doc，请转换为 .docx"

#### Scenario: 不支持的文件不阻塞索引
- **WHEN** 一个目录混合支持和不支持的格式
- **THEN** 支持的正常索引，不支持的单个跳过并记录警告，不阻塞整个目录的索引

### Requirement: ParseResult 数据结构
ParseResult 必须为纯数据结构（非 trait），字段：content（String，必需）、title（Option<String>）、authors（Vec<String>）、misc_metadata（Vec<String>）、parser_name（&'static str，由注册表注入而非 parser 自设）。

#### Scenario: parser_name 由注册表注入
- **WHEN** PdfParser 解析文件后构造 ParseResult
- **THEN** parser_name 不由 PdfParser 设置，而是注册表在调度时注入为 "PdfParser"

#### Scenario: content 必需
- **WHEN** 某格式解析出空内容（如纯图片 PDF 无文字层）
- **THEN** content 为空字符串（合法），仍入索引（仅文件名可搜）
