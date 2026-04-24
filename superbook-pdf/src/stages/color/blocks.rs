use crate::stage::{StageError, StageResult};

pub fn apply_color_correction(image_path: &std::path::Path) -> StageResult {
    use crate::lib_color_stats::{BleedSuppression, ColorAnalyzer};

    let bleed_config = BleedSuppression::default();
    let img = image::open(image_path).map_err(|e| StageError::Image {
        stage: "color",
        message: format!("Failed to open image for color correction: {}", e),
    })?;
    let mut rgb_img = img.to_rgb8();
    ColorAnalyzer::apply_bleed_suppression(&mut rgb_img, &bleed_config);
    let corrected_img = image::DynamicImage::ImageRgb8(rgb_img);
    crate::util::save_webp_lossless(&corrected_img, image_path).map_err(|e| StageError::Image {
        stage: "color",
        message: format!("Failed to save color-corrected image: {}", e),
    })?;

    Ok(())
}
