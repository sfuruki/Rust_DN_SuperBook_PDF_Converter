//! Normalize ステージ
//!
//! OCR 前の内部解像度正規化を独立して実行する。

mod blocks;

use crate::stage::{PageContext, Stage, StageError, StageResult};
use async_trait::async_trait;

/// 解像度正規化ステージ
pub struct NormalizeStage {
    /// 有効フラグ
    pub enabled: bool,
}

impl NormalizeStage {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait]
impl Stage for NormalizeStage {
    fn name(&self) -> &'static str {
        "normalize"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    async fn run(&self, ctx: &mut PageContext) -> StageResult {
        ctx.set_processing(self.name());

        let image_path = ctx.image_path.clone();

        tokio::task::spawn_blocking(move || blocks::apply_normalize(&image_path))
            .await
            .map_err(|e| StageError::Image {
                stage: "normalize",
                message: format!("Task join error: {}", e),
            })??;

        Ok(())
    }
}
