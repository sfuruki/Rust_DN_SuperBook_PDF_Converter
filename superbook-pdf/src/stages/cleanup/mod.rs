//! Cleanup ステージ
//!
//! 成功時に work_dir を削除する（設定で切替可能）。

mod blocks;

use crate::stage::{PageContext, StageResult};
use async_trait::async_trait;

/// 作業ディレクトリクリーンアップステージ
pub struct CleanupStage {
    /// クリーンアップを有効にするか
    pub enabled: bool,
}

impl CleanupStage {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait]
impl crate::stage::Stage for CleanupStage {
    fn name(&self) -> &'static str {
        "cleanup"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    async fn run(&self, ctx: &mut PageContext) -> StageResult {
        ctx.set_processing(self.name());
        blocks::cleanup_work_dir(&ctx.work_dir).await
    }
}
