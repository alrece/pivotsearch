//! # pivotsearch-ocr
//!
//! OCR layer: Tesseract integration (optional, behind a feature gate).
//!
//! Not compiled by default. Enable with `cargo build --features ocr`, which pulls in
//! kreuzberg-tesseract (Tesseract + Leptonica C++ statically compiled in).
//!
//! Language packs (.traineddata) are required at runtime; the caller provides their path.

use std::path::Path;

// ── Without the OCR feature: placeholder implementation ──

#[cfg(not(feature = "ocr"))]
pub fn ocr_available() -> bool {
    false
}

#[cfg(not(feature = "ocr"))]
pub fn ocr_image(_path: &Path, _lang: &str, _datapath: &Path) -> Result<String, String> {
    Err("OCR 未启用，请用 --features ocr 编译".to_string())
}

// ── With the OCR feature enabled: kreuzberg-tesseract implementation ──

#[cfg(feature = "ocr")]
pub fn ocr_available() -> bool {
    true
}

#[cfg(feature = "ocr")]
pub fn ocr_image(path: &Path, lang: &str, datapath: &Path) -> Result<String, String> {
    use kreuzberg_tesseract::{TesseractAPI, TessPageSegMode};

    // 1. Initialize the Tesseract API
    let api = TesseractAPI::new().map_err(|e| format!("Tesseract init: {e:?}"))?;
    api.init(datapath, lang)
        .map_err(|e| format!("Tesseract init lang {lang}: {e:?}"))?;
    api.set_page_seg_mode(TessPageSegMode::PSM_AUTO)
        .map_err(|e| format!("set_page_seg_mode: {e:?}"))?;

    // 2. Read the image file into pixel data (decoded with the image crate)
    let img = image::open(path).map_err(|e| format!("图片解码: {e}"))?;
    let rgb = img.to_rgb8();
    let (width, height) = rgb.dimensions();
    let bytes_per_pixel = 3; // RGB
    let bytes_per_line = width as i32 * bytes_per_pixel;

    // 3. Set the image and run recognition
    api.set_image(
        rgb.as_raw(),
        width as i32,
        height as i32,
        bytes_per_pixel,
        bytes_per_line,
    )
    .map_err(|e| format!("set_image: {e:?}"))?;
    api.recognize().map_err(|e| format!("recognize: {e:?}"))?;

    // 4. Retrieve the recognized text
    api.get_utf8_text()
        .map_err(|e| format!("get_utf8_text: {e:?}"))
}


// Re-export so external code can check availability
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

#[cfg(all(test, feature = "ocr"))]
mod ocr_e2e_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn ocr_recognizes_english_text() {
        let test_img = PathBuf::from("/tmp/test_ocr.png");
        let datapath = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../native/tessdata");
        if !test_img.exists() || !datapath.join("eng.traineddata").exists() {
            eprintln!("跳过：测试图片或语言包不存在");
            return;
        }
        let result = ocr_image(&test_img, "eng", &datapath);
        match &result {
            Ok(text) => {
                let t = text.to_lowercase();
                assert!(t.contains("hello") || t.contains("revenue"),
                    "OCR 应识别出 hello/revenue，实际: {:?}", text);
                println!("✅ OCR 识别成功: {:?}", text.trim());
            }
            Err(e) => panic!("OCR 失败: {}", e),
        }
    }
}
