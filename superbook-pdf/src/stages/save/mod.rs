//! Save ステージ
//!
//! 処理済み WebP 画像を最終出力 PDF に書き込むための情報を蓄積する。
//! 実際の PDF 生成は PipelineRunner が全ページ処理後に行う。
//!
//! このステージでは出力先パスの確定と WebP の最終化を担当する。

mod blocks;

use crate::stage::{PageContext, Stage, StageError, StageResult};
use async_trait::async_trait;
use std::path::PathBuf;

/// 最終出力ステージ
pub struct SaveStage {
    /// 出力ディレクトリ
    pub output_dir: PathBuf,
    /// 出力高さ（px）
    pub output_height: u32,
    /// JPEG クオリティ（0-100）
    pub jpeg_quality: u8,
}

impl SaveStage {
    pub fn new(output_dir: PathBuf, output_height: u32, jpeg_quality: u8) -> Self {
        Self {
            output_dir,
            output_height,
            jpeg_quality,
        }
    }
}

#[async_trait]
impl Stage for SaveStage {
    fn name(&self) -> &'static str {
        "save"
    }

    async fn run(&self, ctx: &mut PageContext) -> StageResult {
        ctx.set_processing(self.name());

        let image_path = ctx.image_path.clone();
        let output_height = self.output_height;

        tokio::task::spawn_blocking(move || blocks::finalize_image(&image_path, output_height))
            .await
            .map_err(|e| StageError::Image {
                stage: "save",
                message: format!("Task join error: {}", e),
            })??;

        ctx.set_done();
        Ok(())
    }
}
