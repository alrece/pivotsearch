//! # pivotsearch-ocr
//!
//! OCR 层：Tesseract 集成（feature gate 可选）。
//!
//! Phase 0 占位：具体实现见 Phase 4 (T9)。
//! 默认不编译，`cargo build --features ocr` 启用。

// Phase 4 将实现（feature gate "ocr"）：
// - tesseract.rs      kreuzberg-tesseract 集成 + 图片识别 + 扫描件 PDF（pdfium 渲染→OCR）
// - language_pack.rs  语言包按需下载（chi_sim/eng 等 .traineddata）

// 无 OCR feature 时此 crate 为空，仅 re-export contracts 占位。
pub use pivotsearch_contracts::ParseResult;
