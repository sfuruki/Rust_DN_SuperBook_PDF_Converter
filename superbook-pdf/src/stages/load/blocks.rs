use crate::stage::{StageError, StageResult};

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const JPEG_SIGNATURE: &[u8; 3] = b"\xFF\xD8\xFF";
const TIFF_LE_SIGNATURE: &[u8; 4] = b"II*\0";
const TIFF_BE_SIGNATURE: &[u8; 4] = b"MM\0*";

fn find_signature_offset(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn normalize_pdftoppm_stdout<'a>(stdout: &'a [u8]) -> (&'a [u8], image::ImageFormat) {
    if let Some(offset) = find_signature_offset(stdout, PNG_SIGNATURE) {
        return (&stdout[offset..], image::ImageFormat::Png);
    }
    if let Some(offset) = find_signature_offset(stdout, JPEG_SIGNATURE) {
        return (&stdout[offset..], image::ImageFormat::Jpeg);
    }
    if let Some(offset) = find_signature_offset(stdout, TIFF_LE_SIGNATURE) {
        return (&stdout[offset..], image::ImageFormat::Tiff);
    }
    if let Some(offset) = find_signature_offset(stdout, TIFF_BE_SIGNATURE) {
        return (&stdout[offset..], image::ImageFormat::Tiff);
    }

    // PNM(P1..P6) はマジックナンバーが短いので先頭一致のみ扱う。
    if stdout.len() >= 2 && stdout[0] == b'P' && (b'1'..=b'6').contains(&stdout[1]) {
        return (stdout, image::ImageFormat::Pnm);
    }

    (stdout, image::ImageFormat::Pnm)
}

/// pdftoppm を使って PDF ページを WebP に抽出する
///
/// pdftoppm の stdout 出力を直接デコードして WebP に変換する。
/// Poppler 実装差で画像バイト列の先頭にログ断片が混入しても、
/// 画像シグネチャ位置を検出してデコードする。
pub fn extract_page_to_webp(
    pdf_path: &std::path::Path,
    page_id: usize,
    dpi: u32,
    output_path: &std::path::Path,
) -> StageResult {
    use std::process::Command;

    // pdftoppm は出力先を省略すると stdout に PNM を出力する。
    // `-png` や出力先 `-` 指定は環境差で空stdoutになるケースがあるため使わない。
    let output = Command::new("pdftoppm")
        .args([
            "-r",
            &dpi.to_string(),
            "-f",
            &page_id.to_string(),
            "-l",
            &page_id.to_string(),
        ])
        .arg(pdf_path)
        .output()
        .map_err(|e| StageError::Io {
            stage: "load",
            source: e,
        })?;

    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StageError::Image {
            stage: "load",
            message: format!(
                "pdftoppm failed with exit code {:?} for page {}: {}",
                output.status.code(), page_id, stderr
            ),
        });
    }

    if output.stdout.is_empty() {
        return Err(StageError::Image {
            stage: "load",
            message: format!(
                "pdftoppm returned empty stdout for page {} (stderr: {})",
                page_id, stderr
            ),
        });
    }

    let (normalized, guessed_format) = normalize_pdftoppm_stdout(&output.stdout);
    let img = image::load_from_memory_with_format(normalized, guessed_format)
        .or_else(|_| image::load_from_memory(normalized))
        .map_err(|e| {
            let head_hex: String = output
                .stdout
                .iter()
                .take(16)
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ");
            StageError::Image {
                stage: "load",
                message: format!(
                    "Failed to decode pdftoppm stdout for page {}: {} (stdout_head_hex: [{}], guessed_format: {:?}, stderr: {})",
                    page_id, e, head_hex, guessed_format, stderr
                ),
            }
        })?;

    // WebP lossless で保存（ファイルサイズ最小化・I/O 高速化）
    let output_file = std::fs::File::create(output_path).map_err(|e| StageError::Io {
        stage: "load",
        source: e,
    })?;
    let encoder = image::codecs::webp::WebPEncoder::new_lossless(output_file);
    encoder
        .encode(
            &img.to_rgba8(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| StageError::Image {
            stage: "load",
            message: format!("Failed to save WebP (lossless): {}", e),
        })?;

    Ok(())
}
