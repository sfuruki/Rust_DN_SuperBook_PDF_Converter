//! OCR ステージ
//!
//! YomiToku による日本語 AI-OCR を実行し、テキストを work_dir/ocr.txt に保存する。
//! 処理後、ctx.text_path を設定する。

mod blocks;

use crate::gpu_queue::GpuJobQueue;
use crate::stage::{PageContext, Stage, StageResult};
use async_trait::async_trait;
use std::sync::Arc;

/// YomiToku OCR ステージ
pub struct OcrStage {
    /// OCR を有効にするか
    pub enabled: bool,
    /// OCR language setting
    pub language: String,
    /// 確信度閾値（0.0-1.0）
    pub confidence: f64,
    /// 出力形式（"json", "text", "markdown"）
    pub format: String,
    /// GPU 動的キュー
    gpu_queue: Arc<GpuJobQueue>,
}

impl OcrStage {
    pub fn new(
        enabled: bool,
        language: impl Into<String>,
        confidence: f64,
        format: impl Into<String>,
        gpu_queue: Arc<GpuJobQueue>,
    ) -> Self {
        Self {
            enabled,
            language: language.into(),
            confidence,
            format: format.into(),
            gpu_queue,
        }
    }
}

#[async_trait]
impl Stage for OcrStage {
    fn name(&self) -> &'static str {
        "ocr"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    async fn run(&self, ctx: &mut PageContext) -> StageResult {
        ctx.set_processing(self.name());

        let _permit = self.gpu_queue.acquire().await?;

        let text_output_path = ctx
            .text_path
            .clone()
            .unwrap_or_else(|| ctx.work_dir.join("ocr.txt"));

        let result_path = blocks::call_yomitoku_api(
            &ctx.image_path,
            &text_output_path,
            &self.language,
            self.confidence,
            &self.format,
        )
        .await?;

        ctx.text_path = Some(result_path);
        Ok(())
    }
}
