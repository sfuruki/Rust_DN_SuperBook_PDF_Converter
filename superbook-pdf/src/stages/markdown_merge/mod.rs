//! MarkdownMerge ステージ
//!
//! ページ単位 Markdown を最終結合する。

mod blocks;

use crate::stage::{PageContext, Stage, StageResult};
use async_trait::async_trait;
use std::path::PathBuf;

/// 最終 Markdown 結合ステージ
pub struct MarkdownMergeStage {
    /// 出力ディレクトリ
    pub output_dir: PathBuf,
    /// 出力タイトル
    pub title: String,
    /// 総ページ数
    pub total_pages: usize,
}

impl MarkdownMergeStage {
    pub fn new(output_dir: PathBuf, title: impl Into<String>, total_pages: usize) -> Self {
        Self {
            output_dir,
            title: title.into(),
            total_pages,
        }
    }
}

#[async_trait]
impl Stage for MarkdownMergeStage {
    fn name(&self) -> &'static str {
        "markdown_merge"
    }

    async fn run(&self, ctx: &mut PageContext) -> StageResult {
        ctx.set_processing(self.name());

        // Markdown 結合は最終ページ到達時のみ実行する。
        if ctx.page_id != self.total_pages {
            return Ok(());
        }

        blocks::merge_pages(&self.output_dir, &self.title, self.total_pages)
    }
}
