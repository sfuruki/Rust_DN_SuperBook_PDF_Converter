use crate::stage::{StageError, StageResult};

/// 最終出力用にリサイズする
pub fn finalize_image(image_path: &std::path::Path, output_height: u32) -> StageResult {
    if output_height == 0 {
        return Ok(());
    }

    let img = image::open(image_path).map_err(|e| StageError::Image {
        stage: "save",
        message: format!("Failed to open image: {}", e),
    })?;

    let (w, h) = (img.width(), img.height());
    if h == output_height {
        return Ok(());
    }

    // Preserve AI upscale effect: do not downsample enlarged pages here.
    if h > output_height {
        return Ok(());
    }

    let scale = output_height as f64 / h as f64;
    let new_width = (w as f64 * scale).round() as u32;
    let resized = img.resize_exact(
        new_width,
        output_height,
        image::imageops::FilterType::Lanczos3,
    );

    crate::util::save_webp_lossless(&resized, image_path).map_err(|e| StageError::Image {
        stage: "save",
        message: format!("Failed to save resized image: {}", e),
    })?;

    Ok(())
}
