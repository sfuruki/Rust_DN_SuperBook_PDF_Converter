use crate::lib_markdown_renderer::MarkdownGenerator;
use crate::stage::{StageError, StageResult};
use std::path::Path;

pub fn merge_pages(output_dir: &Path, title: &str, total_pages: usize) -> StageResult {
    let generator = MarkdownGenerator::new(output_dir).map_err(|e| StageError::Image {
        stage: "markdown_merge",
        message: format!("Failed to initialize markdown generator: {}", e),
    })?;

    generator
        .merge_pages(title, total_pages)
        .map(|_| ())
        .map_err(|e| StageError::Io {
            stage: "markdown_merge",
            source: std::io::Error::other(e.to_string()),
        })
}
