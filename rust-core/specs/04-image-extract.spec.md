# 04-image-extract.spec.md - Image Extraction Specification

## Overview

PDFからページ画像を抽出するモジュール。
ImageMagick/Ghostscriptまたは純Rustライブラリを使用してPDFをラスタライズする。

---

## Responsibilities

1. PDFページを画像ファイルとして抽出
2. DPI設定の適用
3. カラースペース変換（CMYK→RGB）
4. 透明度処理
5. マルチページの並列抽出

---

## Data Structures

```rust
use std::path::{Path, PathBuf};

/// 画像抽出オプション
#[derive(Debug, Clone)]
pub struct ExtractOptions {
    /// 出力DPI
    pub dpi: u32,
    /// 出力フォーマット
    pub format: ImageFormat,
    /// カラースペース
    pub colorspace: ColorSpace,
    /// 背景色（透明部分の処理用）
    pub background: Option<[u8; 3]>,
    /// 並列処理数
    pub parallel: usize,
    /// 進捗コールバック
    pub progress_callback: Option<Box<dyn Fn(usize, usize) + Send + Sync>>,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum ImageFormat {
    #[default]
    Png,
    Jpeg { quality: u8 },
    Bmp,
    Tiff,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum ColorSpace {
    #[default]
    Rgb,
    Grayscale,
    Cmyk,
}

/// 抽出結果
#[derive(Debug)]
pub struct ExtractedPage {
    pub page_index: usize,
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
}

/// エラー型
#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("PDF file not found: {0}")]
    PdfNotFound(PathBuf),

    #[error("Output directory not writable: {0}")]
    OutputNotWritable(PathBuf),

    #[error("Extraction failed for page {page}: {reason}")]
    ExtractionFailed { page: usize, reason: String },

    #[error("External tool error: {0}")]
    ExternalToolError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ExtractError>;
```

---

## Public API

```rust
/// ImageExtractor トレイト
pub trait ImageExtractor {
    /// 全ページを抽出
    fn extract_all(
        pdf_path: &Path,
        output_dir: &Path,
        options: &ExtractOptions,
    ) -> Result<Vec<ExtractedPage>>;

    /// 指定ページを抽出
    fn extract_page(
        pdf_path: &Path,
        page_index: usize,
        output_path: &Path,
        options: &ExtractOptions,
    ) -> Result<ExtractedPage>;

    /// ストリーミング抽出（イテレータ）
    fn extract_streaming(
        pdf_path: &Path,
        output_dir: &Path,
        options: &ExtractOptions,
    ) -> Result<impl Iterator<Item = Result<ExtractedPage>>>;
}

/// ImageMagick/Ghostscript実装
pub struct MagickExtractor;

/// 純Rust実装（pdfium-render使用）
pub struct RustExtractor;

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            dpi: 300,
            format: ImageFormat::Png,
            colorspace: ColorSpace::Rgb,
            background: Some([255, 255, 255]), // 白背景
            parallel: num_cpus::get(),
            progress_callback: None,
        }
    }
}
```

---

## Test Cases

### TC-EXT-001: 単一ページ抽出

```rust
#[test]
fn test_extract_single_page() {
    let temp_dir = tempfile::tempdir().unwrap();
    let output = temp_dir.path().join("page_0.png");

    let result = MagickExtractor::extract_page(
        Path::new("tests/fixtures/sample.pdf"),
        0,
        &output,
        &ExtractOptions::default(),
    ).unwrap();

    assert!(output.exists());
    assert_eq!(result.page_index, 0);
    assert!(result.width > 0);
    assert!(result.height > 0);
}
```

### TC-EXT-002: 全ページ抽出

```rust
#[test]
fn test_extract_all_pages() {
    let temp_dir = tempfile::tempdir().unwrap();

    let results = MagickExtractor::extract_all(
        Path::new("tests/fixtures/10pages.pdf"),
        temp_dir.path(),
        &ExtractOptions::default(),
    ).unwrap();

    assert_eq!(results.len(), 10);
    for (i, result) in results.iter().enumerate() {
        assert_eq!(result.page_index, i);
        assert!(result.path.exists());
    }
}
```

### TC-EXT-003: DPI設定

```rust
#[test]
fn test_dpi_setting() {
    let temp_dir = tempfile::tempdir().unwrap();

    // 72 DPI
    let output_72 = temp_dir.path().join("72dpi.png");
    let result_72 = MagickExtractor::extract_page(
        Path::new("tests/fixtures/a4.pdf"),
        0,
        &output_72,
        &ExtractOptions { dpi: 72, ..Default::default() },
    ).unwrap();

    // 300 DPI
    let output_300 = temp_dir.path().join("300dpi.png");
    let result_300 = MagickExtractor::extract_page(
        Path::new("tests/fixtures/a4.pdf"),
        0,
        &output_300,
        &ExtractOptions { dpi: 300, ..Default::default() },
    ).unwrap();

    // 300 DPI画像は72 DPIより約4倍大きいはず
    assert!(result_300.width > result_72.width * 3);
    assert!(result_300.height > result_72.height * 3);
}
```

### TC-EXT-004: JPEG出力

```rust
#[test]
fn test_jpeg_output() {
    let temp_dir = tempfile::tempdir().unwrap();
    let output = temp_dir.path().join("page_0.jpg");

    let result = MagickExtractor::extract_page(
        Path::new("tests/fixtures/sample.pdf"),
        0,
        &output,
        &ExtractOptions {
            format: ImageFormat::Jpeg { quality: 85 },
            ..Default::default()
        },
    ).unwrap();

    assert!(output.exists());
    // JPEG magic bytes チェック
    let bytes = std::fs::read(&output).unwrap();
    assert_eq!(&bytes[0..2], &[0xFF, 0xD8]);
}
```

### TC-EXT-005: グレースケール変換

```rust
#[test]
fn test_grayscale_extraction() {
    let temp_dir = tempfile::tempdir().unwrap();
    let output = temp_dir.path().join("gray.png");

    MagickExtractor::extract_page(
        Path::new("tests/fixtures/color.pdf"),
        0,
        &output,
        &ExtractOptions {
            colorspace: ColorSpace::Grayscale,
            ..Default::default()
        },
    ).unwrap();

    // 画像がグレースケールであることを確認
    let img = image::open(&output).unwrap();
    let rgb = img.to_rgb8();
    // 各ピクセルでR=G=Bであることを検証
    for pixel in rgb.pixels() {
        assert_eq!(pixel[0], pixel[1]);
        assert_eq!(pixel[1], pixel[2]);
    }
}
```

### TC-EXT-006: 透明部分の処理

```rust
#[test]
fn test_transparency_handling() {
    let temp_dir = tempfile::tempdir().unwrap();

    // 白背景
    let output_white = temp_dir.path().join("white_bg.png");
    MagickExtractor::extract_page(
        Path::new("tests/fixtures/transparent.pdf"),
        0,
        &output_white,
        &ExtractOptions {
            background: Some([255, 255, 255]),
            ..Default::default()
        },
    ).unwrap();

    // 黒背景
    let output_black = temp_dir.path().join("black_bg.png");
    MagickExtractor::extract_page(
        Path::new("tests/fixtures/transparent.pdf"),
        0,
        &output_black,
        &ExtractOptions {
            background: Some([0, 0, 0]),
            ..Default::default()
        },
    ).unwrap();

    // 2つの画像が異なることを確認
    let img_white = std::fs::read(&output_white).unwrap();
    let img_black = std::fs::read(&output_black).unwrap();
    assert_ne!(img_white, img_black);
}
```

### TC-EXT-007: 並列抽出

```rust
#[test]
fn test_parallel_extraction() {
    let temp_dir = tempfile::tempdir().unwrap();

    let start = std::time::Instant::now();
    let results = MagickExtractor::extract_all(
        Path::new("tests/fixtures/20pages.pdf"),
        temp_dir.path(),
        &ExtractOptions {
            parallel: 4,
            ..Default::default()
        },
    ).unwrap();
    let parallel_time = start.elapsed();

    assert_eq!(results.len(), 20);

    // シングルスレッドより速いことを確認（ベンチマーク用）
    // 実際のテストでは時間比較は不安定なので省略可
}
```

### TC-EXT-008: 進捗コールバック

```rust
#[test]
fn test_progress_callback() {
    let temp_dir = tempfile::tempdir().unwrap();
    let progress = Arc::new(Mutex::new(Vec::new()));
    let progress_clone = progress.clone();

    MagickExtractor::extract_all(
        Path::new("tests/fixtures/5pages.pdf"),
        temp_dir.path(),
        &ExtractOptions {
            progress_callback: Some(Box::new(move |current, total| {
                progress_clone.lock().unwrap().push((current, total));
            })),
            ..Default::default()
        },
    ).unwrap();

    let recorded = progress.lock().unwrap();
    assert_eq!(recorded.len(), 5);
    assert_eq!(recorded.last(), Some(&(5, 5)));
}
```

### TC-EXT-009: 存在しないPDFエラー

```rust
#[test]
fn test_nonexistent_pdf_error() {
    let temp_dir = tempfile::tempdir().unwrap();

    let result = MagickExtractor::extract_all(
        Path::new("/nonexistent/file.pdf"),
        temp_dir.path(),
        &ExtractOptions::default(),
    );

    assert!(matches!(result, Err(ExtractError::PdfNotFound(_))));
}
```

### TC-EXT-010: 書き込み不可ディレクトリエラー

```rust
#[test]
fn test_unwritable_output_error() {
    let result = MagickExtractor::extract_all(
        Path::new("tests/fixtures/sample.pdf"),
        Path::new("/root/unwritable"),
        &ExtractOptions::default(),
    );

    assert!(matches!(result, Err(ExtractError::OutputNotWritable(_))));
}
```

---

## Implementation Notes

### ImageMagick/Ghostscript を使用する場合

```rust
use std::process::Command;

impl MagickExtractor {
    pub fn extract_page(
        pdf_path: &Path,
        page_index: usize,
        output_path: &Path,
        options: &ExtractOptions,
    ) -> Result<ExtractedPage> {
        if !pdf_path.exists() {
            return Err(ExtractError::PdfNotFound(pdf_path.to_path_buf()));
        }

        let mut cmd = Command::new("magick");
        cmd.arg("-density").arg(options.dpi.to_string());

        if let Some(bg) = options.background {
            cmd.arg("-background")
                .arg(format!("rgb({},{},{})", bg[0], bg[1], bg[2]));
            cmd.arg("-alpha").arg("remove");
        }

        match options.colorspace {
            ColorSpace::Grayscale => {
                cmd.arg("-colorspace").arg("gray");
            }
            _ => {}
        }

        cmd.arg(format!("{}[{}]", pdf_path.display(), page_index));

        match options.format {
            ImageFormat::Jpeg { quality } => {
                cmd.arg("-quality").arg(quality.to_string());
            }
            _ => {}
        }

        cmd.arg(output_path);

        let output = cmd.output()?;
        if !output.status.success() {
            return Err(ExtractError::ExternalToolError(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }

        // 画像サイズを取得
        let img = image::open(output_path)
            .map_err(|e| ExtractError::ExtractionFailed {
                page: page_index,
                reason: e.to_string(),
            })?;

        Ok(ExtractedPage {
            page_index,
            path: output_path.to_path_buf(),
            width: img.width(),
            height: img.height(),
            format: options.format,
        })
    }
}
```

### 純Rust実装（pdfium-render）

```rust
use pdfium_render::prelude::*;

impl RustExtractor {
    pub fn extract_all(
        pdf_path: &Path,
        output_dir: &Path,
        options: &ExtractOptions,
    ) -> Result<Vec<ExtractedPage>> {
        let pdfium = Pdfium::new(
            Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
                .or_else(|_| Pdfium::bind_to_system_library())?,
        );

        let document = pdfium.load_pdf_from_file(pdf_path, None)?;
        let mut results = Vec::new();

        for (i, page) in document.pages().iter().enumerate() {
            let render_config = PdfRenderConfig::new()
                .set_target_width(
                    (page.width().value * options.dpi as f32 / 72.0) as i32
                )
                .set_target_height(
                    (page.height().value * options.dpi as f32 / 72.0) as i32
                );

            let bitmap = page.render_with_config(&render_config)?;
            let output_path = output_dir.join(format!("page_{:05}.png", i));
            bitmap.as_image().save(&output_path)?;

            results.push(ExtractedPage {
                page_index: i,
                path: output_path,
                width: bitmap.width() as u32,
                height: bitmap.height() as u32,
                format: options.format,
            });

            if let Some(ref callback) = options.progress_callback {
                callback(i + 1, document.pages().len());
            }
        }

        Ok(results)
    }
}
```

---

## Acceptance Criteria

- [ ] 単一ページを正常に抽出できる
- [ ] 全ページを正常に抽出できる
- [ ] DPI設定が正しく反映される
- [ ] JPEG出力が正しく動作する
- [ ] グレースケール変換が正しく動作する
- [ ] 透明部分が指定背景色で処理される
- [ ] 並列抽出が正しく動作する
- [ ] 進捗コールバックが呼び出される
- [ ] 存在しないPDFで適切なエラーを返す
- [ ] 書き込み不可ディレクトリで適切なエラーを返す

---

## Dependencies

```toml
[dependencies]
image = "0.25"
thiserror = "2"
num_cpus = "1"
rayon = "1.10"

# Optional: 純Rust実装
pdfium-render = { version = "0.8", optional = true }

[dev-dependencies]
tempfile = "3"
```
