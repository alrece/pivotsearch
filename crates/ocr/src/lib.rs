//! # pivotsearch-ocr
//!
//! OCR 层：Tesseract 集成（feature gate 可选）。
//!
//! 默认不编译（无 OCR feature 时此 crate 为空壳）。
//! `cargo build --features ocr` 启用，引入 kreuzberg-tesseract。
//!
//! Phase 4 T9：当前为骨架。完整实现见 feature gate 内。

// 无 OCR feature 时，提供占位类型和说明
#[cfg(not(feature = "ocr"))]
pub fn ocr_available() -> bool {
    false
}

#[cfg(not(feature = "ocr"))]
pub fn ocr_image(_path: &std::path::Path) -> Result<String, &'static str> {
    Err("OCR 未启用，请用 --features ocr 编译")
}

// OCR feature 启用时的实现（Phase 4 T9 完整版）
#[cfg(feature = "ocr")]
pub fn ocr_available() -> bool {
    true
}

#[cfg(feature = "ocr")]
pub fn ocr_image(path: &std::path::Path) -> Result<String, Box<dyn std::error::Error>> {
    // TODO Phase 4 T9 完整实现：kreuzberg-tesseract 集成
    // 当前为骨架，实际引入 kreuzberg-tesseract 后实现
    let _ = path;
    Err("OCR feature 已启用，但 kreuzberg-tesseract 集成待实现".into())
}
