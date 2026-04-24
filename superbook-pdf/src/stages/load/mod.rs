//! Load ステージ
//!
//! PDF からページ画像を抽出し、WebP 形式で work_dir に保存する。
//! 処理後、ctx.image_path を更新する。

mod blocks;

use crate::stage::{PageContext, Stage, StageError, StageResult};
use async_trait::async_trait;
use std::path::PathBuf;

/// PDF ページを WebP として抽出するステージ
pub struct LoadStage {
    /// 元の PDF ファイルパス
    pub pdf_path: PathBuf,
    /// 抽出 DPI
    pub dpi: u32,
}

impl LoadStage {
    pub fn new(pdf_path: PathBuf, dpi: u32) -> Self {
        Self { pdf_path, dpi }
    }
}

#[async_trait]
impl Stage for LoadStage {
    fn name(&self) -> &'static str {
        "load"
    }

    async fn run(&self, ctx: &mut PageContext) -> StageResult {
        ctx.set_processing(self.name());
        ctx.ensure_work_dir().map_err(|e| StageError::Io {
            stage: "load",
            source: e,
        })?;

        let pdf_path = self.pdf_path.clone();
        let dpi = self.dpi;
        let page_id = ctx.page_id;
        let output_path = ctx.image_path.clone();

        tokio::task::spawn_blocking(move || {
            blocks::extract_page_to_webp(&pdf_path, page_id, dpi, &output_path)
        })
        .await
        .map_err(|e| StageError::Image {
            stage: "load",
            message: format!("Task join error: {}", e),
        })??;

        Ok(())
    }
}
