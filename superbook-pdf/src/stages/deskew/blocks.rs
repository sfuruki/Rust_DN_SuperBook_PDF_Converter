use crate::stage::{StageError, StageResult};

pub fn apply_deskew(
    input_path: &std::path::Path,
    output_path: &std::path::Path,
    strength: f64,
) -> StageResult {
    use crate::lib_deskew::{DeskewOptions, ImageProcDeskewer};

    let max_angle = 15.0 * strength;
    let opts = DeskewOptions::builder().max_angle(max_angle).build();
    match ImageProcDeskewer::deskew(input_path, output_path, &opts) {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("[deskew] Deskew failed (non-fatal): {}", e);
            if input_path != output_path {
                std::fs::copy(input_path, output_path).map_err(|e| StageError::Io {
                    stage: "deskew",
                    source: e,
                })?;
            }
            Ok(())
        }
    }
}
