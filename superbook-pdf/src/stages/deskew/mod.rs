//! Deskew ステージ
//!
//! 画像の傾き補正を独立して実行する。

mod blocks;

use crate::stage::{PageContext, Stage, StageError, StageResult};
use async_trait::async_trait;

/// 傾き補正ステージ
pub struct DeskewStage {
    /// 有効フラグ
    pub enabled: bool,
    /// 傾き補正の強度（0.0-1.0）
    pub strength: f64,
}

impl DeskewStage {
    pub fn new(enabled: bool, strength: f64) -> Self {
        Self { enabled, strength }
    }
}

#[async_trait]
impl Stage for DeskewStage {
    fn name(&self) -> &'static str {
        "deskew"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    async fn run(&self, ctx: &mut PageContext) -> StageResult {
        ctx.set_processing(self.name());

        let image_path = ctx.image_path.clone();
        let output_path = image_path.clone();
        let strength = self.strength;

        tokio::task::spawn_blocking(move || blocks::apply_deskew(&image_path, &output_path, strength))
            .await
            .map_err(|e| StageError::Image {
                stage: "deskew",
                message: format!("Task join error: {}", e),
            })??;

        Ok(())
    }
}
