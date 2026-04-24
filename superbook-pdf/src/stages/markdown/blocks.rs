use crate::lib_markdown_renderer::{ContentElement, MarkdownGenerator, PageContent};
use crate::stage::StageError;
use std::path::{Path, PathBuf};

pub fn generate_page_markdown(
    output_dir: &Path,
    page_index: usize,
    image_path: &Path,
    text_path: Option<&Path>,
    include_page_numbers: bool,
    detected_page_number: Option<i32>,
) -> Result<PathBuf, StageError> {
    let generator = MarkdownGenerator::new(output_dir).map_err(|e| StageError::Image {
        stage: "markdown",
        message: format!("Failed to initialize markdown generator: {}", e),
    })?;

    let text = read_text_payload(text_path);
    let page_content = build_page_content(
        page_index,
        image_path,
        &text,
        include_page_numbers,
        detected_page_number,
    );

    let markdown = generator
        .generate_page_markdown(&page_content)
        .map_err(|e| StageError::Image {
            stage: "markdown",
            message: format!("Failed to generate page markdown: {}", e),
        })?;

    generator
        .save_page_markdown(page_index, &markdown)
        .map_err(|e| StageError::Io {
            stage: "markdown",
            source: std::io::Error::other(e.to_string()),
        })
}

fn read_text_payload(text_path: Option<&Path>) -> String {
    text_path
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default()
}

fn build_page_content(
    page_index: usize,
    image_path: &Path,
    text: &str,
    include_page_numbers: bool,
    detected_page_number: Option<i32>,
) -> PageContent {
    let mut elements = Vec::new();

    if !text.trim().is_empty() {
        let mut content = text.trim().to_string();
        if include_page_numbers {
            if let Some(n) = detected_page_number {
                content = format!("[Page {}]\n\n{}", n, content);
            }
        }
        elements.push(ContentElement::Text {
            content,
            direction: crate::TextDirection::Horizontal,
        });
    } else {
        elements.push(ContentElement::FullPageImage {
            image_path: image_path.to_path_buf(),
        });
    }

    elements.push(ContentElement::PageBreak);

    PageContent {
        page_index,
        elements,
    }
}

