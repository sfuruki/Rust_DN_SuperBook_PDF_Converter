//! Color ステージ
//!
//! HSV ブリードスルー抑制による色補正を独立して実行する。

mod blocks;

use crate::stage::{PageContext, Stage, StageError, StageResult};
use async_trait::async_trait;

/// 色補正ステージ
pub struct ColorStage {
    /// 有効フラグ
    pub enabled: bool,
}

impl ColorStage {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait]
impl Stage for ColorStage {
    fn name(&self) -> &'static str {
        "color"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    async fn run(&self, ctx: &mut PageContext) -> StageResult {
        ctx.set_processing(self.name());

        let image_path = ctx.image_path.clone();

        tokio::task::spawn_blocking(move || blocks::apply_color_correction(&image_path))
            .await
            .map_err(|e| StageError::Image {
                stage: "color",
                message: format!("Task join error: {}", e),
            })??;

        Ok(())
    }
}
