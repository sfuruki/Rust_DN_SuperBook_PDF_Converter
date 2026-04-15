# 29-yomitoku.spec.md - YomiToku Japanese AI-OCR Specification

## Overview

YomiTokuは日本語AI-OCRエンジンで、スキャン画像から高精度でテキストを抽出する。
Pythonブリッジスクリプト経由で実行され、GPU加速をサポート。

---

## Responsibilities

1. 日本語テキストのOCR認識
2. 縦書き/横書きテキストの検出
3. 検索可能PDFレイヤーの生成
4. バッチ処理による複数ページ処理
5. 信頼度スコアに基づくフィルタリング

---

## Data Structures

```rust
/// YomiToku OCR options
#[derive(Debug, Clone)]
pub struct YomiTokuOptions {
    /// Output format (JSON, Text, PDF)
    pub output_format: OutputFormat,
    /// Enable GPU acceleration
    pub use_gpu: bool,
    /// GPU device ID
    pub gpu_id: Option<u32>,
    /// Confidence threshold (0.0-1.0)
    pub confidence_threshold: f32,
    /// Timeout in seconds
    pub timeout_secs: u64,
    /// Enable vertical text detection
    pub detect_vertical: bool,
    /// Language hint
    pub language: Language,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum OutputFormat {
    #[default]
    Json,
    Text,
    Pdf,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Language {
    #[default]
    Japanese,
    English,
    Mixed,
}

/// OCR recognition result
#[derive(Debug, Clone)]
pub struct OcrResult {
    /// Recognized text blocks
    pub text_blocks: Vec<TextBlock>,
    /// Full extracted text
    pub full_text: String,
    /// Overall confidence (0.0-1.0)
    pub confidence: f32,
    /// Detected text direction
    pub direction: TextDirection,
    /// Processing time in seconds
    pub processing_time: f64,
}

/// Single text block
#[derive(Debug, Clone)]
pub struct TextBlock {
    /// Recognized text
    pub text: String,
    /// Bounding box (x, y, width, height)
    pub bbox: (u32, u32, u32, u32),
    /// Confidence score
    pub confidence: f32,
    /// Text direction
    pub direction: TextDirection,
    /// Estimated font size (optional)
    pub font_size: Option<f32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum TextDirection {
    #[default]
    Horizontal,
    Vertical,
}
```

---

## API

### Builder Pattern

```rust
// Default options
let options = YomiTokuOptions::default();

// Custom options with builder
let options = YomiTokuOptions::builder()
    .language(Language::Japanese)
    .confidence_threshold(0.5)
    .use_gpu(true)
    .gpu_id(Some(0))
    .output_format(OutputFormat::Json)
    .detect_vertical(true)
    .timeout_secs(300)
    .build();

// Preset for book scanning (lower threshold)
let options = YomiTokuOptions::for_books();
```

### Single Image OCR

```rust
let yomitoku = YomiToku::new();
let result = yomitoku.recognize(Path::new("page.png"), &options)?;

println!("Full text: {}", result.full_text);
println!("Confidence: {:.2}", result.confidence);
println!("Text blocks: {}", result.text_blocks.len());
```

### Batch Processing

```rust
let images = vec![
    Path::new("page1.png"),
    Path::new("page2.png"),
    Path::new("page3.png"),
];

let results = yomitoku.recognize_batch(&images, &options)?;
for (i, result) in results.iter().enumerate() {
    println!("Page {}: {} blocks", i + 1, result.text_blocks.len());
}
```

### PDF with OCR Layer

```rust
// Generate searchable PDF
yomitoku.create_searchable_pdf(
    Path::new("scanned.pdf"),
    Path::new("searchable.pdf"),
    &options,
)?;
```

---

## Test Cases

### TC-YOMI-001: 基本OCR認識

```rust
#[test]
fn test_basic_ocr_recognition() {
    let yomitoku = YomiToku::new();
    let options = YomiTokuOptions::default();

    let result = yomitoku.recognize(
        Path::new("tests/fixtures/japanese_text.png"),
        &options,
    ).unwrap();

    assert!(!result.full_text.is_empty());
    assert!(result.confidence > 0.5);
}
```

### TC-YOMI-002: 縦書きテキスト検出

```rust
#[test]
fn test_vertical_text_detection() {
    let yomitoku = YomiToku::new();
    let options = YomiTokuOptions::builder()
        .detect_vertical(true)
        .build();

    let result = yomitoku.recognize(
        Path::new("tests/fixtures/vertical_japanese.png"),
        &options,
    ).unwrap();

    assert!(result.text_blocks.iter().any(|b| b.direction == TextDirection::Vertical));
}
```

### TC-YOMI-003: 信頼度フィルタリング

```rust
#[test]
fn test_confidence_filtering() {
    let yomitoku = YomiToku::new();

    // High threshold
    let high_options = YomiTokuOptions::builder()
        .confidence_threshold(0.9)
        .build();

    let high_result = yomitoku.recognize(
        Path::new("tests/fixtures/noisy_text.png"),
        &high_options,
    ).unwrap();

    // Low threshold
    let low_options = YomiTokuOptions::builder()
        .confidence_threshold(0.3)
        .build();

    let low_result = yomitoku.recognize(
        Path::new("tests/fixtures/noisy_text.png"),
        &low_options,
    ).unwrap();

    // Low threshold should capture more text
    assert!(low_result.text_blocks.len() >= high_result.text_blocks.len());
}
```

### TC-YOMI-004: バッチ処理

```rust
#[test]
fn test_batch_processing() {
    let yomitoku = YomiToku::new();
    let options = YomiTokuOptions::default();

    let images: Vec<_> = (1..=5)
        .map(|i| PathBuf::from(format!("tests/fixtures/page_{}.png", i)))
        .collect();

    let results = yomitoku.recognize_batch(&images, &options).unwrap();

    assert_eq!(results.len(), 5);
    assert!(results.iter().all(|r| r.is_some()));
}
```

### TC-YOMI-005: GPU加速

```rust
#[test]
fn test_gpu_acceleration() {
    let yomitoku = YomiToku::new();

    if !yomitoku.is_gpu_available() {
        return; // Skip if no GPU
    }

    let gpu_options = YomiTokuOptions::builder()
        .use_gpu(true)
        .gpu_id(Some(0))
        .build();

    let result = yomitoku.recognize(
        Path::new("tests/fixtures/large_page.png"),
        &gpu_options,
    ).unwrap();

    assert!(!result.full_text.is_empty());
}
```

### TC-YOMI-006: タイムアウト処理

```rust
#[test]
fn test_timeout_handling() {
    let yomitoku = YomiToku::new();
    let options = YomiTokuOptions::builder()
        .timeout_secs(1) // Very short timeout
        .build();

    // Large image should timeout
    let result = yomitoku.recognize(
        Path::new("tests/fixtures/very_large_page.png"),
        &options,
    );

    assert!(result.is_err());
}
```

### TC-YOMI-007: 出力形式

```rust
#[test]
fn test_output_formats() {
    let yomitoku = YomiToku::new();

    for format in [OutputFormat::Json, OutputFormat::Text, OutputFormat::Pdf] {
        let options = YomiTokuOptions::builder()
            .output_format(format)
            .build();

        let result = yomitoku.recognize(
            Path::new("tests/fixtures/sample_page.png"),
            &options,
        ).unwrap();

        assert!(!result.full_text.is_empty());
    }
}
```

### TC-YOMI-008: エラーハンドリング

```rust
#[test]
fn test_error_handling() {
    let yomitoku = YomiToku::new();
    let options = YomiTokuOptions::default();

    // Non-existent file
    let result = yomitoku.recognize(
        Path::new("nonexistent.png"),
        &options,
    );
    assert!(matches!(result, Err(YomiTokuError::InputNotFound(_))));

    // Invalid image
    let result = yomitoku.recognize(
        Path::new("tests/fixtures/invalid.txt"),
        &options,
    );
    assert!(result.is_err());
}
```

### TC-YOMI-009: テキストブロック境界

```rust
#[test]
fn test_text_block_bounding_boxes() {
    let yomitoku = YomiToku::new();
    let options = YomiTokuOptions::default();

    let result = yomitoku.recognize(
        Path::new("tests/fixtures/multiblock_page.png"),
        &options,
    ).unwrap();

    for block in &result.text_blocks {
        let (x, y, w, h) = block.bbox;
        assert!(w > 0, "Width should be positive");
        assert!(h > 0, "Height should be positive");
        assert!(x + w < 10000, "Bbox should be within reasonable bounds");
        assert!(y + h < 15000, "Bbox should be within reasonable bounds");
    }
}
```

### TC-YOMI-010: 言語検出

```rust
#[test]
fn test_language_options() {
    let yomitoku = YomiToku::new();

    for lang in [Language::Japanese, Language::English, Language::Mixed] {
        let options = YomiTokuOptions::builder()
            .language(lang)
            .build();

        assert_eq!(options.language, lang);
    }
}
```

---

## Implementation Notes

### Python Bridge Integration

```rust
impl YomiToku {
    pub fn recognize(&self, image_path: &Path, options: &YomiTokuOptions) -> Result<OcrResult> {
        // Use SubprocessBridge to call Python script
        let bridge = SubprocessBridge::new(AiTool::YomiToku);

        let args = self.build_args(image_path, options);
        let output = bridge.execute(&args, Duration::from_secs(options.timeout_secs))?;

        self.parse_bridge_output(&output)
    }

    fn build_args(&self, image_path: &Path, options: &YomiTokuOptions) -> Vec<String> {
        vec![
            image_path.to_string_lossy().to_string(),
            "--confidence".to_string(),
            options.confidence_threshold.to_string(),
            if options.use_gpu { "--gpu" } else { "--cpu" }.to_string(),
            "--format".to_string(),
            format!("{:?}", options.output_format).to_lowercase(),
        ]
    }
}
```

### Bridge Script Protocol

The Python bridge script (`ai_bridge/yomitoku_bridge.py`) returns JSON:

```json
{
    "text_blocks": [
        {
            "text": "認識されたテキスト",
            "bbox": [100, 200, 300, 50],
            "confidence": 0.95,
            "direction": "horizontal"
        }
    ],
    "full_text": "認識されたテキスト\n次の行...",
    "processing_time": 2.5
}
```

---

## Acceptance Criteria

- [ ] 日本語テキストを高精度で認識できる
- [ ] 縦書き/横書きを正しく検出できる
- [ ] 信頼度閾値によるフィルタリングが機能する
- [ ] バッチ処理が効率的に動作する
- [ ] GPU加速が有効な場合に高速化される
- [ ] タイムアウトが適切に処理される
- [ ] 各出力形式 (JSON/Text/PDF) をサポート
- [ ] エラーハンドリングが適切に動作する
- [ ] テキストブロックの境界ボックスが正確
- [ ] 言語オプションが機能する

---

## Dependencies

```toml
[dependencies]
thiserror = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Python environment
# - Python 3.10+
# - yomitoku (pip install yomitoku)
# - CUDA (optional, for GPU)
```

---

## Constants

| 定数 | 値 | 説明 |
|------|-----|------|
| `DEFAULT_CONFIDENCE_THRESHOLD` | 0.5 | デフォルト信頼度閾値 |
| `BOOK_CONFIDENCE_THRESHOLD` | 0.3 | 書籍スキャン用低閾値 |
| `DEFAULT_TIMEOUT_SECS` | 300 | デフォルトタイムアウト (5分) |
| `MIN_CONFIDENCE` | 0.0 | 最小信頼度 |
| `MAX_CONFIDENCE` | 1.0 | 最大信頼度 |

---

## Related Specs

- [08-ai-bridge.spec.md](./08-ai-bridge.spec.md) - Python subprocess管理
- [09-realesrgan.spec.md](./09-realesrgan.spec.md) - AI画像アップスケーリング
- [03-pdf-writer.spec.md](./03-pdf-writer.spec.md) - OCRレイヤー付きPDF生成
