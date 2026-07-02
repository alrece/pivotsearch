//! # pivotsearch-ocr
//!
//! OCR 层：Tesseract 集成（feature gate 可选）。
//!
//! 默认不编译。`cargo build --features ocr` 启用，引入 kreuzberg-tesseract
//! （内置 Tesseract + Leptonica C++ 静态编译）。
//!
//! 运行时需要语言包（.traineddata），由调用方提供路径。

use std::path::Path;

// ── 无 OCR feature 时：占位实现 ──

#[cfg(not(feature = "ocr"))]
pub fn ocr_available() -> bool {
    false
}

#[cfg(not(feature = "ocr"))]
pub fn ocr_image(_path: &Path, _lang: &str, _datapath: &Path) -> Result<String, String> {
    Err("OCR 未启用，请用 --features ocr 编译".to_string())
}

// ── OCR feature 启用时：kreuzberg-tesseract 实现 ──

#[cfg(feature = "ocr")]
pub fn ocr_available() -> bool {
    true
}

#[cfg(feature = "ocr")]
pub fn ocr_image(path: &Path, lang: &str, datapath: &Path) -> Result<String, String> {
    use kreuzberg_tesseract::{TesseractAPI, TessPageSegMode};

    // 1. 初始化 Tesseract API
    let api = TesseractAPI::new().map_err(|e| format!("Tesseract init: {e:?}"))?;
    api.init(datapath, lang)
        .map_err(|e| format!("Tesseract init lang {lang}: {e:?}"))?;
    api.set_page_seg_mode(TessPageSegMode::PSM_AUTO)
        .map_err(|e| format!("set_page_seg_mode: {e:?}"))?;

    // 2. 读取图片文件为像素数据（用 image crate 解码）
    let img = image::open(path).map_err(|e| format!("图片解码: {e}"))?;
    let rgb = img.to_rgb8();
    let (width, height) = rgb.dimensions();
    let bytes_per_pixel = 3; // RGB
    let bytes_per_line = width as i32 * bytes_per_pixel;

    // 3. 设置图片并识别
    api.set_image(
        rgb.as_raw(),
        width as i32,
        height as i32,
        bytes_per_pixel,
        bytes_per_line,
    )
    .map_err(|e| format!("set_image: {e:?}"))?;
    api.recognize().map_err(|e| format!("recognize: {e:?}"))?;

    // 4. 获取识别文本
    api.get_utf8_text()
        .map_err(|e| format!("get_utf8_text: {e:?}"))
}


// 重新导出供外部判断
#[cfg(feature = "ocr")]
pub use kreuzberg_tesseract::TesseractAPI;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ocr_availability_matches_feature() {
        #[cfg(feature = "ocr")]
        assert!(ocr_available());
        #[cfg(not(feature = "ocr"))]
        assert!(!ocr_available());
    }
}
