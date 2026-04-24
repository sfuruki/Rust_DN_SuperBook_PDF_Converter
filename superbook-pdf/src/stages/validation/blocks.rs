use crate::stage::{PageContext, StageError, StageResult};

pub fn validate_output(ctx: &PageContext, min_chars: usize) -> StageResult {
    // 画像が存在するか確認
    if !ctx.image_path.exists() {
        return Err(StageError::Validation {
            stage: "validation",
            message: format!(
                "Page {} image not found: {}",
                ctx.page_id,
                ctx.image_path.display()
            ),
        });
    }

    // OCR テキストの品質チェック（OCR が有効な場合のみ）
    if let Some(text_path) = &ctx.text_path {
        if let Ok(text) = std::fs::read_to_string(text_path) {
            let char_count = text.chars().filter(|c| !c.is_whitespace()).count();
            if min_chars > 0 && char_count < min_chars {
                eprintln!(
                    "[validation] Page {} OCR: only {} non-whitespace chars (min: {})",
                    ctx.page_id, char_count, min_chars
                );
                // 警告のみ（Skipped として継続）
                return Err(StageError::Skipped {
                    stage: "validation",
                    reason: format!("OCR char count {} below minimum {}", char_count, min_chars),
                });
            }
        }
    }

    Ok(())
}
