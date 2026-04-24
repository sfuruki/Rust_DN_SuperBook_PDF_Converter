use crate::stage::StageError;

/// YomiToku HTTP API を非同期で呼び出す
pub async fn call_yomitoku_api(
    input_path: &std::path::Path,
    output_path: &std::path::Path,
    language: &str,
    confidence: f64,
    format: &str,
) -> Result<std::path::PathBuf, StageError> {
    let yomitoku_url =
        std::env::var("YOMITOKU_API_URL").unwrap_or_else(|_| "http://localhost:8001".to_string());

    let payload = serde_json::json!({
        "input_path": input_path.to_str().unwrap_or(""),
        "language": language,
        "confidence": confidence,
        "format": format,
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/ocr", yomitoku_url))
        .json(&payload)
        .send()
        .await
        .map_err(|e| StageError::AiService {
            stage: "ocr",
            message: format!("YomiToku API call failed: {}", e),
        })?;

    if !resp.status().is_success() {
        return Err(StageError::AiService {
            stage: "ocr",
            message: format!("YomiToku returned HTTP {}", resp.status()),
        });
    }

    // OCR結果をテキストファイルに保存
    let body = resp.text().await.map_err(|e| StageError::AiService {
        stage: "ocr",
        message: format!("Failed to read YomiToku response: {}", e),
    })?;

    tokio::fs::write(output_path, body.as_bytes()).await.map_err(|e| StageError::Io {
        stage: "ocr",
        source: e,
    })?;

    Ok(output_path.to_path_buf())
}
