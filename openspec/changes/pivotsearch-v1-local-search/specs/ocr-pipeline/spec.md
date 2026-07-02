## ADDED Requirements

### Requirement: feature gate 可选启用
OCR 必须通过 cargo feature gate（`ocr`）控制，默认不编译。`cargo build --features ocr` 启用。默认包不含 Tesseract 库和语言包，保证基础包体积小。

#### Scenario: 默认不启用 OCR
- **WHEN** `cargo build`（无 features）
- **THEN** 不编译 kreuzberg-tesseract，OCR 相关代码通过 `#[cfg(feature = "ocr")]` 隔离，包体积不受影响

#### Scenario: 启用 OCR
- **WHEN** `cargo build --features ocr`
- **THEN** 编译 kreuzberg-tesseract（内置静态编译），ImageOcrParser 注册到 parser_registry

### Requirement: 图片 OCR 识别
启用 OCR 后，系统必须能识别图片中的文字：jpg/jpeg/png/tiff/bmp 格式，喂给 Tesseract，产出文本作为 content。

#### Scenario: 截图识别
- **WHEN** 索引一个含中文文字的 screenshot.png（启用 OCR）
- **THEN** Tesseract 识别出文字，content 为识别结果，可被搜索

#### Scenario: OCR 失败容错
- **WHEN** 图片无文字（如纯风景照）或清晰度极低，OCR 产出空或乱码
- **THEN** content 为空字符串（合法），文件仍入索引（文件名可搜），不阻塞

### Requirement: 扫描件 PDF OCR
扫描件 PDF（无文字层，仅图片）必须通过 pdfium 渲染每页为图片后 OCR。系统需先检测 PDF 是否含文字层（pdfium 提取文本为空或极少），决定是否走 OCR 管道。

#### Scenario: 扫描件检测
- **WHEN** 解析一个扫描的 PDF（pdfium 提取文本为空）
- **THEN** 自动走 OCR 管道：逐页渲染为图片 → Tesseract 识别 → 拼接为 content

#### Scenario: 含文字层 PDF 不重复 OCR
- **WHEN** 解析一个原生 PDF（pdfium 提取出丰富文本）
- **THEN** 直接用 pdfium 文本，不走 OCR（避免浪费）

### Requirement: 语言包按需下载
Tesseract 语言包（`.traineddata`，中文 chi_sim 约 30-50MB）必须按需下载，不进默认安装包。首次启用 OCR 时从官方源下载到用户数据目录。

#### Scenario: 首次启用下载
- **WHEN** 用户首次开启 OCR 功能（设置页切换开关）
- **THEN** 提示"需要下载中文语言包（约 40MB）"，确认后下载 chi_sim.traineddata 到用户数据目录

#### Scenario: 语言包已存在不重复下载
- **WHEN** 再次启用 OCR，语言包已在
- **THEN** 跳过下载，直接使用

#### Scenario: 多语言选择
- **WHEN** 用户勾选中文+英文识别
- **THEN** 下载 chi_sim + eng 两个语言包，OCR 时用两语言提升准确率

### Requirement: ImageOcrParser 注册
OCR 启用后，ImageOcrParser 必须注册到 parser_registry，扩展名 jpg/jpeg/png/tiff/bmp，走与其他 parser 相同的两级选择。parser_name 字段记录为 "ImageOcrParser"，预览时据此决定渲染方式。

#### Scenario: 图片文件路由
- **WHEN** 索引 photo.jpg（启用 OCR）
- **THEN** 扩展名匹配 ImageOcrParser，走 OCR 管道

#### Scenario: 预览区分 OCR 文档
- **WHEN** 预览一个 parser="ImageOcrParser" 的结果
- **THEN** 预览面板显示原图 + OCR 文本（区分于纯文本文档的渲染）
