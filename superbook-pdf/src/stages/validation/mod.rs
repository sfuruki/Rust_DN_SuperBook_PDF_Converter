//! Validation ステージ
//!
//! OCR 結果の品質チェック（文字数・画像サイズなど）を行う。

mod blocks;

use crate::stage::{PageContext, Stage, StageResult};
use async_trait::async_trait;

/// 出力検証ステージ
pub struct ValidationStage {
    /// 最小文字数（OCR テキストの品質チェック用）
    pub min_chars: usize,
    /// 有効フラグ
    pub enabled: bool,
}

impl ValidationStage {
    pub fn new(enabled: bool, min_chars: usize) -> Self {
        Self { enabled, min_chars }
    }
}

#[async_trait]
impl Stage for ValidationStage {
    fn name(&self) -> &'static str {
        "validation"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    async fn run(&self, ctx: &mut PageContext) -> StageResult {
        ctx.set_processing(self.name());
        blocks::validate_output(ctx, self.min_chars)
    }
}
