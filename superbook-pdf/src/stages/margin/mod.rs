//! Margin ステージ
//!
//! マージン調整の独立ステージ。

mod blocks;

use crate::stage::{PageContext, Stage, StageResult};
use async_trait::async_trait;

/// マージン処理ステージ
pub struct MarginStage {
    /// 有効フラグ
    pub enabled: bool,
}

impl MarginStage {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait]
impl Stage for MarginStage {
    fn name(&self) -> &'static str {
        "margin"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    async fn run(&self, ctx: &mut PageContext) -> StageResult {
        ctx.set_processing(self.name());
        blocks::apply_margin(&ctx.image_path)
    }
}
