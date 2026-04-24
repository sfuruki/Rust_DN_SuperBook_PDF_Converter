use crate::lib_page_number::{PageNumberOptions, TesseractPageDetector};
use crate::stage::StageError;

pub fn detect_page_number(
    image_path: &std::path::Path,
    page_index: usize,
) -> Result<Option<i32>, StageError> {
    let options = PageNumberOptions::default();

    let detected = TesseractPageDetector::detect_single(image_path, page_index, &options)
        .map_err(|e| StageError::Image {
            stage: "page_number",
            message: format!("Page number detection failed: {}", e),
        })?;

    Ok(detected.number)
}
