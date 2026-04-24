use crate::stage::StageResult;

pub fn apply_normalize(image_path: &std::path::Path) -> StageResult {
    use crate::lib_normalize::{ImageNormalizer, NormalizeOptions};

    let opts = NormalizeOptions::default();
    match ImageNormalizer::normalize(image_path, image_path, &opts) {
        Ok(normalized) => {
            let _ = normalized;
        }
        Err(e) => {
            eprintln!("[normalize] Normalize failed (non-fatal): {}", e);
        }
    }

    Ok(())
}
