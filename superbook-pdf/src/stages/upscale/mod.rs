//! Upscale ステージ
//!
//! RealESRGAN による AI 超解像処理を実行する。
//! 処理後、ctx.image_path を上書きする。

mod blocks;

use crate::gpu_queue::GpuJobQueue;
use crate::stage::{PageContext, Stage, StageResult};
use async_trait::async_trait;
use std::sync::Arc;

/// AI 超解像ステージ
pub struct UpscaleStage {
    /// 拡大倍率（2 or 4）
    pub scale: u32,
    /// モデル名
    pub model_name: String,
    /// 有効フラグ
    pub enabled: bool,
    /// タイル処理サイズ（px）。0 = タイル無し
    pub tile: u32,
    /// FP32 モード
    pub fp32: bool,
    /// GPU 動的キュー
    gpu_queue: Arc<GpuJobQueue>,
}

impl UpscaleStage {
    pub fn new(
        scale: u32,
        model_name: impl Into<String>,
        enabled: bool,
        tile: u32,
        fp32: bool,
        gpu_queue: Arc<GpuJobQueue>,
    ) -> Self {
        Self {
            scale,
            model_name: model_name.into(),
            enabled,
            tile,
            fp32,
            gpu_queue,
        }
    }
}

#[async_trait]
impl Stage for UpscaleStage {
    fn name(&self) -> &'static str {
        "upscale"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    async fn run(&self, ctx: &mut PageContext) -> StageResult {
        ctx.set_processing(self.name());

        let _permit = self.gpu_queue.acquire().await?;

        blocks::call_realesrgan_api(
            &ctx.image_path,
            &ctx.image_path,
            self.scale,
            &self.model_name,
            self.tile,
            self.fp32,
        )
        .await
    }
}
