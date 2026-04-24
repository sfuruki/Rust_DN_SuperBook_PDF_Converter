//! Markdown ステージ
//!
//! OCR 結果をページ単位 Markdown として保存する。

mod blocks;

use crate::stage::{PageContext, Stage, StageError, StageResult};
use async_trait::async_trait;
use std::path::PathBuf;

/// ページMarkdown生成ステージ
pub struct MarkdownStage {
    /// 出力ディレクトリ
    pub output_dir: PathBuf,
    /// ページ番号を本文に含めるか
    pub include_page_numbers: bool,
}

impl MarkdownStage {
    pub fn new(output_dir: PathBuf, include_page_numbers: bool) -> Self {
        Self {
            output_dir,
            include_page_numbers,
        }
    }
}

#[async_trait]
impl Stage for MarkdownStage {
    fn name(&self) -> &'static str {
        "markdown"
    }

    async fn run(&self, ctx: &mut PageContext) -> StageResult {
        ctx.set_processing(self.name());

        let output_dir = self.output_dir.clone();
        let page_index = ctx.page_id.saturating_sub(1);
        let image_path = ctx.image_path.clone();
        let text_path = ctx.text_path.clone();
        let include_page_numbers = self.include_page_numbers;
        let detected_page_number = ctx.detected_page_number;

        let markdown_path = tokio::task::spawn_blocking(move || {
            blocks::generate_page_markdown(
                &output_dir,
                page_index,
                &image_path,
                text_path.as_deref(),
                include_page_numbers,
                detected_page_number,
            )
        })
        .await
        .map_err(|e| StageError::Image {
            stage: "markdown",
            message: format!("Task join error: {}", e),
        })??;

        ctx.markdown_path = Some(markdown_path);
        Ok(())
    }
}
