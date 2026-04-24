//! PageNumber ステージ
//!
//! ページ画像からページ番号を検出し、PageContext に保持する。

mod blocks;

use crate::stage::{PageContext, Stage, StageError, StageResult};
use async_trait::async_trait;

/// ページ番号検出ステージ
pub struct PageNumberStage {
    /// 有効フラグ
    pub enabled: bool,
}

impl PageNumberStage {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait]
impl Stage for PageNumberStage {
    fn name(&self) -> &'static str {
        "page_number"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    async fn run(&self, ctx: &mut PageContext) -> StageResult {
        ctx.set_processing(self.name());

        let image_path = ctx.image_path.clone();
        let page_index = ctx.page_id.saturating_sub(1);

        let detected = tokio::task::spawn_blocking(move || {
            blocks::detect_page_number(&image_path, page_index)
        })
        .await
        .map_err(|e| StageError::Image {
            stage: "page_number",
            message: format!("Task join error: {}", e),
        })??;

        ctx.detected_page_number = detected;
        Ok(())
    }
}
