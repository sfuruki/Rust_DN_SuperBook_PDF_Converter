# 03-pdf-writer.spec.md - PDF Writer Specification

## Overview

処理済み画像からPDFを生成するモジュール。
`printpdf` または `lopdf` を使用してPDFを作成する。

---

## Responsibilities

1. 画像ファイル群からPDFを生成
2. メタデータの設定
3. ページサイズの統一
4. 圧縮オプションの適用
5. OCRレイヤーの埋め込み（透明テキスト）

---

## Data Structures

```rust
use std::path::PathBuf;

/// PDF生成オプション
#[derive(Debug, Clone)]
pub struct PdfWriterOptions {
    /// 出力DPI
    pub dpi: u32,
    /// JPEG品質 (1-100)
    pub jpeg_quality: u8,
    /// 画像圧縮方式
    pub compression: ImageCompression,
    /// ページサイズ統一モード
    pub page_size_mode: PageSizeMode,
    /// メタデータ
    pub metadata: Option<PdfMetadata>,
    /// OCRテキストレイヤー
    pub ocr_layer: Option<OcrLayer>,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum ImageCompression {
    #[default]
    Jpeg,
    JpegLossless,
    Flate,
    None,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum PageSizeMode {
    /// 最初のページに合わせる
    #[default]
    FirstPage,
    /// 最大サイズに合わせる
    MaxSize,
    /// 指定サイズ
    Fixed { width_pt: f64, height_pt: f64 },
    /// 各ページの元サイズを維持
    Original,
}

/// OCRテキストレイヤー
#[derive(Debug, Clone)]
pub struct OcrLayer {
    pub pages: Vec<OcrPageText>,
}

#[derive(Debug, Clone)]
pub struct OcrPageText {
    pub page_index: usize,
    pub blocks: Vec<TextBlock>,
}

#[derive(Debug, Clone)]
pub struct TextBlock {
    /// 座標（ポイント単位、左下原点）
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub text: String,
    pub font_size: f64,
    /// 縦書きかどうか
    pub vertical: bool,
}

/// エラー型
#[derive(Debug, thiserror::Error)]
pub enum PdfWriterError {
    #[error("No images provided")]
    NoImages,

    #[error("Image not found: {0}")]
    ImageNotFound(PathBuf),

    #[error("Unsupported image format: {0}")]
    UnsupportedFormat(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("PDF generation error: {0}")]
    GenerationError(String),
}

pub type Result<T> = std::result::Result<T, PdfWriterError>;
```

---

## Public API

```rust
/// PdfWriter トレイト
pub trait PdfWriter {
    /// 画像群からPDFを生成
    fn create_from_images(
        images: &[PathBuf],
        output: &Path,
        options: &PdfWriterOptions,
    ) -> Result<()>;

    /// ストリーミング生成（メモリ効率重視）
    fn create_streaming(
        images: impl Iterator<Item = PathBuf>,
        output: &Path,
        options: &PdfWriterOptions,
    ) -> Result<()>;
}

/// デフォルト実装
pub struct PrintPdfWriter;

impl PrintPdfWriter {
    pub fn new() -> Self;
}

impl Default for PdfWriterOptions {
    fn default() -> Self {
        Self {
            dpi: 300,
            jpeg_quality: 90,
            compression: ImageCompression::Jpeg,
            page_size_mode: PageSizeMode::FirstPage,
            metadata: None,
            ocr_layer: None,
        }
    }
}
```

---

## Test Cases

### TC-PDW-001: 単一画像からPDF生成

```rust
#[test]
fn test_single_image_to_pdf() {
    let temp_dir = tempfile::tempdir().unwrap();
    let output = temp_dir.path().join("output.pdf");

    let images = vec![PathBuf::from("tests/fixtures/page1.jpg")];
    let options = PdfWriterOptions::default();

    PrintPdfWriter::create_from_images(&images, &output, &options).unwrap();

    assert!(output.exists());

    // Verify it's a valid PDF
    let doc = lopdf::Document::load(&output).unwrap();
    assert_eq!(doc.get_pages().len(), 1);
}
```

### TC-PDW-002: 複数画像からPDF生成

```rust
#[test]
fn test_multiple_images_to_pdf() {
    let temp_dir = tempfile::tempdir().unwrap();
    let output = temp_dir.path().join("output.pdf");

    let images: Vec<_> = (1..=10)
        .map(|i| PathBuf::from(format!("tests/fixtures/page{}.jpg", i)))
        .collect();
    let options = PdfWriterOptions::default();

    PrintPdfWriter::create_from_images(&images, &output, &options).unwrap();

    let doc = lopdf::Document::load(&output).unwrap();
    assert_eq!(doc.get_pages().len(), 10);
}
```

### TC-PDW-003: 空の画像リストエラー

```rust
#[test]
fn test_empty_images_error() {
    let temp_dir = tempfile::tempdir().unwrap();
    let output = temp_dir.path().join("output.pdf");

    let result = PrintPdfWriter::create_from_images(&[], &output, &PdfWriterOptions::default());

    assert!(matches!(result, Err(PdfWriterError::NoImages)));
}
```

### TC-PDW-004: 存在しない画像エラー

```rust
#[test]
fn test_nonexistent_image_error() {
    let temp_dir = tempfile::tempdir().unwrap();
    let output = temp_dir.path().join("output.pdf");

    let images = vec![PathBuf::from("/nonexistent/image.jpg")];

    let result = PrintPdfWriter::create_from_images(&images, &output, &PdfWriterOptions::default());

    assert!(matches!(result, Err(PdfWriterError::ImageNotFound(_))));
}
```

### TC-PDW-005: メタデータ設定

```rust
#[test]
fn test_metadata_setting() {
    let temp_dir = tempfile::tempdir().unwrap();
    let output = temp_dir.path().join("output.pdf");

    let images = vec![PathBuf::from("tests/fixtures/page1.jpg")];
    let options = PdfWriterOptions {
        metadata: Some(PdfMetadata {
            title: Some("Test Document".to_string()),
            author: Some("Test Author".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };

    PrintPdfWriter::create_from_images(&images, &output, &options).unwrap();

    // Verify metadata
    let doc = lopdf::Document::load(&output).unwrap();
    // Check /Info dictionary
    // ...
}
```

### TC-PDW-006: JPEG品質設定

```rust
#[test]
fn test_jpeg_quality() {
    let temp_dir = tempfile::tempdir().unwrap();

    let images = vec![PathBuf::from("tests/fixtures/page1.jpg")];

    // High quality
    let output_high = temp_dir.path().join("high.pdf");
    let options_high = PdfWriterOptions {
        jpeg_quality: 95,
        ..Default::default()
    };
    PrintPdfWriter::create_from_images(&images, &output_high, &options_high).unwrap();

    // Low quality
    let output_low = temp_dir.path().join("low.pdf");
    let options_low = PdfWriterOptions {
        jpeg_quality: 50,
        ..Default::default()
    };
    PrintPdfWriter::create_from_images(&images, &output_low, &options_low).unwrap();

    // High quality should be larger
    let size_high = std::fs::metadata(&output_high).unwrap().len();
    let size_low = std::fs::metadata(&output_low).unwrap().len();
    assert!(size_high > size_low);
}
```

### TC-PDW-007: ストリーミング生成（メモリ効率）

```rust
#[test]
fn test_streaming_memory_efficiency() {
    let temp_dir = tempfile::tempdir().unwrap();
    let output = temp_dir.path().join("output.pdf");

    // 100枚の画像をストリーミング処理
    let images = (1..=100).map(|i| PathBuf::from(format!("tests/fixtures/page{}.jpg", i)));

    PrintPdfWriter::create_streaming(images, &output, &PdfWriterOptions::default()).unwrap();

    // メモリ使用量チェック（ピーク < 200MB）
    // Note: 実際のテストでは別プロセスで測定が必要
}
```

### TC-PDW-008: OCRレイヤー埋め込み

```rust
#[test]
fn test_ocr_layer_embedding() {
    let temp_dir = tempfile::tempdir().unwrap();
    let output = temp_dir.path().join("output.pdf");

    let images = vec![PathBuf::from("tests/fixtures/page1.jpg")];
    let options = PdfWriterOptions {
        ocr_layer: Some(OcrLayer {
            pages: vec![OcrPageText {
                page_index: 0,
                blocks: vec![TextBlock {
                    x: 100.0,
                    y: 700.0,
                    width: 400.0,
                    height: 20.0,
                    text: "テスト文字列".to_string(),
                    font_size: 12.0,
                    vertical: false,
                }],
            }],
        }),
        ..Default::default()
    };

    PrintPdfWriter::create_from_images(&images, &output, &options).unwrap();

    // PDFからテキスト抽出して検証
    // ...
}
```

### TC-PDW-009: 縦書きOCR対応

```rust
#[test]
fn test_vertical_text_ocr() {
    let temp_dir = tempfile::tempdir().unwrap();
    let output = temp_dir.path().join("output.pdf");

    let images = vec![PathBuf::from("tests/fixtures/vertical_text.jpg")];
    let options = PdfWriterOptions {
        ocr_layer: Some(OcrLayer {
            pages: vec![OcrPageText {
                page_index: 0,
                blocks: vec![TextBlock {
                    x: 500.0,
                    y: 100.0,
                    width: 20.0,
                    height: 600.0,
                    text: "縦書きテスト".to_string(),
                    font_size: 12.0,
                    vertical: true,
                }],
            }],
        }),
        ..Default::default()
    };

    PrintPdfWriter::create_from_images(&images, &output, &options).unwrap();

    // 縦書きテキストが正しく配置されていることを検証
}
```

### TC-PDW-010: 異なるサイズの画像処理

```rust
#[test]
fn test_mixed_size_images() {
    let temp_dir = tempfile::tempdir().unwrap();
    let output = temp_dir.path().join("output.pdf");

    let images = vec![
        PathBuf::from("tests/fixtures/a4.jpg"),    // A4サイズ
        PathBuf::from("tests/fixtures/letter.jpg"), // Letterサイズ
        PathBuf::from("tests/fixtures/square.jpg"), // 正方形
    ];

    let options = PdfWriterOptions {
        page_size_mode: PageSizeMode::Original,
        ..Default::default()
    };

    PrintPdfWriter::create_from_images(&images, &output, &options).unwrap();

    // 各ページのサイズを検証
    let doc = lopdf::Document::load(&output).unwrap();
    // ...
}
```

---

## Implementation Notes

### printpdf を使用する場合

```rust
use printpdf::*;
use image::GenericImageView;

impl PrintPdfWriter {
    pub fn create_from_images(
        images: &[PathBuf],
        output: &Path,
        options: &PdfWriterOptions,
    ) -> Result<()> {
        if images.is_empty() {
            return Err(PdfWriterError::NoImages);
        }

        // 最初の画像からサイズを取得
        let first_img = image::open(&images[0])
            .map_err(|e| PdfWriterError::GenerationError(e.to_string()))?;
        let (width, height) = first_img.dimensions();

        let dpi = options.dpi as f64;
        let width_mm = Mm((width as f64 / dpi) * 25.4);
        let height_mm = Mm((height as f64 / dpi) * 25.4);

        let (doc, page1, layer1) = PdfDocument::new(
            options.metadata.as_ref().map(|m| m.title.as_deref().unwrap_or("Document")).unwrap_or("Document"),
            width_mm,
            height_mm,
            "Layer 1",
        );

        // 画像を追加
        for (i, img_path) in images.iter().enumerate() {
            // Add image to page
            // ...
        }

        doc.save(&mut BufWriter::new(File::create(output)?))
            .map_err(|e| PdfWriterError::GenerationError(e.to_string()))?;

        Ok(())
    }
}
```

---

## Acceptance Criteria

- [ ] 単一画像からPDFを生成できる
- [ ] 複数画像から複数ページPDFを生成できる
- [ ] 空の画像リストで適切なエラーを返す
- [ ] 存在しない画像で適切なエラーを返す
- [ ] メタデータが正しく設定される
- [ ] JPEG品質が出力サイズに反映される
- [ ] ストリーミング生成でメモリ効率が良い
- [ ] OCRテキストレイヤーが埋め込まれる
- [ ] 縦書きテキストが正しく配置される
- [ ] 異なるサイズの画像を処理できる

---

## Dependencies

```toml
[dependencies]
printpdf = "0.7"
lopdf = "0.34"
image = "0.25"
thiserror = "2"

[dev-dependencies]
tempfile = "3"
```
