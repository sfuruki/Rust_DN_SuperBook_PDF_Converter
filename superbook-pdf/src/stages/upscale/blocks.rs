use crate::stage::{StageError, StageResult};

/// RealESRGAN HTTP API を非同期で呼び出す
pub async fn call_realesrgan_api(
    input_path: &std::path::Path,
    output_path: &std::path::Path,
    scale: u32,
    model_name: &str,
    tile: u32,
    fp32: bool,
) -> StageResult {
    let realesrgan_url =
        std::env::var("REALESRGAN_API_URL").unwrap_or_else(|_| "http://localhost:8000".to_string());

    let payload = serde_json::json!({
        "input_path": input_path.to_str().unwrap_or(""),
        "output_path": output_path.to_str().unwrap_or(""),
        "scale": scale,
        "model_name": model_name,
        "tile": tile,
        "fp32": fp32,
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/upscale", realesrgan_url))
        .json(&payload)
        .send()
        .await
        .map_err(|e| StageError::AiService {
            stage: "upscale",
            message: format!("RealESRGAN API call failed: {}", e),
        })?;

    if !resp.status().is_success() {
        return Err(StageError::AiService {
            stage: "upscale",
            message: format!("RealESRGAN returned HTTP {}", resp.status()),
        });
    }

    Ok(())
}
