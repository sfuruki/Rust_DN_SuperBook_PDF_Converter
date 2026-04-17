# 06-margin.spec.md - Margin Detection & Trimming Specification

## Overview

スキャン画像の余白検出とトリミングを行うモジュール。
全ページで統一された最適マージンを算出する。

---

## Responsibilities

1. 単一画像の余白検出
2. 全ページ統一マージンの算出
3. マージントリミングの実行
4. パディング追加（統一サイズ化）

---

## Data Structures

```rust
use std::path::Path;

/// マージン検出オプション
#[derive(Debug, Clone)]
pub struct MarginOptions {
    /// 背景色判定の閾値（0-255）
    pub background_threshold: u8,
    /// 最小マージン（ピクセル）
    pub min_margin: u32,
    /// デフォルトトリム率（%）
    pub default_trim_percent: f32,
    /// エッジ検出感度
    pub edge_sensitivity: f32,
    /// コンテンツ検出モード
    pub detection_mode: ContentDetectionMode,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum ContentDetectionMode {
    /// 単純な背景色検出
    #[default]
    BackgroundColor,
    /// エッジ検出ベース
    EdgeDetection,
    /// ヒストグラム分析
    Histogram,
    /// 複合検出
    Combined,
}

/// マージン情報（ピクセル単位）
#[derive(Debug, Clone, Copy, Default)]
pub struct Margins {
    pub top: u32,
    pub bottom: u32,
    pub left: u32,
    pub right: u32,
}

impl Margins {
    pub fn uniform(value: u32) -> Self {
        Self { top: value, bottom: value, left: value, right: value }
    }

    pub fn total_horizontal(&self) -> u32 {
        self.left + self.right
    }

    pub fn total_vertical(&self) -> u32 {
        self.top + self.bottom
    }
}

/// 検出結果
#[derive(Debug, Clone)]
pub struct MarginDetection {
    /// 検出された余白
    pub margins: Margins,
    /// 画像サイズ
    pub image_size: (u32, u32),
    /// コンテンツ領域
    pub content_rect: ContentRect,
    /// 検出信頼度
    pub confidence: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct ContentRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// 統一マージン結果
#[derive(Debug, Clone)]
pub struct UnifiedMargins {
    /// 全ページ共通のマージン
    pub margins: Margins,
    /// 各ページの検出結果
    pub page_detections: Vec<MarginDetection>,
    /// 統一後のサイズ
    pub unified_size: (u32, u32),
}

/// トリミング結果
#[derive(Debug)]
pub struct TrimResult {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub original_size: (u32, u32),
    pub trimmed_size: (u32, u32),
    pub margins_applied: Margins,
}

/// エラー型
#[derive(Debug, thiserror::Error)]
pub enum MarginError {
    #[error("Image not found: {0}")]
    ImageNotFound(PathBuf),

    #[error("Invalid image: {0}")]
    InvalidImage(String),

    #[error("No content detected in image")]
    NoContentDetected,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, MarginError>;
```

---

## Public API

```rust
/// MarginDetector トレイト
pub trait MarginDetector {
    /// 単一画像のマージンを検出
    fn detect(image_path: &Path, options: &MarginOptions) -> Result<MarginDetection>;

    /// 複数画像の統一マージンを算出
    fn detect_unified(
        images: &[PathBuf],
        options: &MarginOptions,
    ) -> Result<UnifiedMargins>;

    /// マージンをトリミング
    fn trim(
        input_path: &Path,
        output_path: &Path,
        margins: &Margins,
    ) -> Result<TrimResult>;

    /// 統一サイズにパディング追加
    fn pad_to_size(
        input_path: &Path,
        output_path: &Path,
        target_size: (u32, u32),
        background: [u8; 3],
    ) -> Result<TrimResult>;

    /// 検出・トリミング・統一を一括実行
    fn process_batch(
        images: &[(PathBuf, PathBuf)],
        options: &MarginOptions,
    ) -> Result<Vec<TrimResult>>;
}

/// デフォルト実装
pub struct ImageMarginDetector;

impl Default for MarginOptions {
    fn default() -> Self {
        Self {
            background_threshold: 250,
            min_margin: 10,
            default_trim_percent: 0.5,
            edge_sensitivity: 0.5,
            detection_mode: ContentDetectionMode::BackgroundColor,
        }
    }
}
```

---

## Test Cases

### TC-MRG-001: 単一画像マージン検出

```rust
#[test]
fn test_detect_single_image_margins() {
    let detection = ImageMarginDetector::detect(
        Path::new("tests/fixtures/with_margins.png"),
        &MarginOptions::default(),
    ).unwrap();

    // 上下左右すべてにマージンが検出される
    assert!(detection.margins.top > 0);
    assert!(detection.margins.bottom > 0);
    assert!(detection.margins.left > 0);
    assert!(detection.margins.right > 0);
}
```

### TC-MRG-002: マージンなし画像

```rust
#[test]
fn test_detect_no_margins() {
    let detection = ImageMarginDetector::detect(
        Path::new("tests/fixtures/no_margins.png"),
        &MarginOptions::default(),
    ).unwrap();

    // マージンがほぼゼロ
    assert!(detection.margins.top < 5);
    assert!(detection.margins.bottom < 5);
    assert!(detection.margins.left < 5);
    assert!(detection.margins.right < 5);
}
```

### TC-MRG-003: 非対称マージン

```rust
#[test]
fn test_detect_asymmetric_margins() {
    // 左右非対称なマージンを持つ画像
    let detection = ImageMarginDetector::detect(
        Path::new("tests/fixtures/asymmetric_margins.png"),
        &MarginOptions::default(),
    ).unwrap();

    // 左右のマージンが異なる
    assert_ne!(detection.margins.left, detection.margins.right);
}
```

### TC-MRG-004: 統一マージン算出

```rust
#[test]
fn test_unified_margins() {
    let images: Vec<_> = (1..=5)
        .map(|i| PathBuf::from(format!("tests/fixtures/page_{}.png", i)))
        .collect();

    let unified = ImageMarginDetector::detect_unified(
        &images,
        &MarginOptions::default(),
    ).unwrap();

    // 全ページの最小マージンが使用される
    assert!(unified.margins.top <= unified.page_detections.iter().map(|d| d.margins.top).min().unwrap());

    // 統一サイズが設定される
    assert!(unified.unified_size.0 > 0);
    assert!(unified.unified_size.1 > 0);
}
```

### TC-MRG-005: マージントリミング

```rust
#[test]
fn test_trim_margins() {
    let temp_dir = tempfile::tempdir().unwrap();
    let output = temp_dir.path().join("trimmed.png");

    let result = ImageMarginDetector::trim(
        Path::new("tests/fixtures/with_margins.png"),
        &output,
        &Margins { top: 50, bottom: 50, left: 30, right: 30 },
    ).unwrap();

    assert!(output.exists());
    assert!(result.trimmed_size.0 < result.original_size.0);
    assert!(result.trimmed_size.1 < result.original_size.1);
}
```

### TC-MRG-006: パディング追加

```rust
#[test]
fn test_pad_to_size() {
    let temp_dir = tempfile::tempdir().unwrap();
    let output = temp_dir.path().join("padded.png");

    let result = ImageMarginDetector::pad_to_size(
        Path::new("tests/fixtures/small_image.png"),
        &output,
        (1000, 1500),
        [255, 255, 255],
    ).unwrap();

    let img = image::open(&output).unwrap();
    assert_eq!(img.width(), 1000);
    assert_eq!(img.height(), 1500);
}
```

### TC-MRG-007: 背景閾値設定

```rust
#[test]
fn test_background_threshold() {
    // 明るいグレー背景の画像
    let gray_bg = Path::new("tests/fixtures/gray_background.png");

    // 高閾値（白のみを背景と判定）
    let detection_high = ImageMarginDetector::detect(
        gray_bg,
        &MarginOptions {
            background_threshold: 250,
            ..Default::default()
        },
    ).unwrap();

    // 低閾値（グレーも背景と判定）
    let detection_low = ImageMarginDetector::detect(
        gray_bg,
        &MarginOptions {
            background_threshold: 200,
            ..Default::default()
        },
    ).unwrap();

    // 低閾値の方がより多くのマージンを検出
    assert!(detection_low.margins.top >= detection_high.margins.top);
}
```

### TC-MRG-008: エッジ検出モード

```rust
#[test]
fn test_edge_detection_mode() {
    let detection = ImageMarginDetector::detect(
        Path::new("tests/fixtures/document_page.png"),
        &MarginOptions {
            detection_mode: ContentDetectionMode::EdgeDetection,
            ..Default::default()
        },
    ).unwrap();

    assert!(detection.confidence > 0.5);
}
```

### TC-MRG-009: バッチ処理

```rust
#[test]
fn test_batch_processing() {
    let temp_dir = tempfile::tempdir().unwrap();

    let images: Vec<_> = (1..=3)
        .map(|i| {
            let input = PathBuf::from(format!("tests/fixtures/page_{}.png", i));
            let output = temp_dir.path().join(format!("trimmed_{}.png", i));
            (input, output)
        })
        .collect();

    let results = ImageMarginDetector::process_batch(
        &images,
        &MarginOptions::default(),
    ).unwrap();

    assert_eq!(results.len(), 3);

    // 全画像が同じサイズに統一される
    let sizes: Vec<_> = results.iter().map(|r| r.trimmed_size).collect();
    assert!(sizes.windows(2).all(|w| w[0] == w[1]));
}
```

### TC-MRG-010: コンテンツなし画像エラー

```rust
#[test]
fn test_no_content_error() {
    // 完全に白い画像
    let result = ImageMarginDetector::detect(
        Path::new("tests/fixtures/blank_white.png"),
        &MarginOptions::default(),
    );

    assert!(matches!(result, Err(MarginError::NoContentDetected)));
}
```

---

## Implementation Notes

### 背景色ベースの検出

```rust
use image::{GenericImageView, GrayImage, Pixel};

impl ImageMarginDetector {
    pub fn detect(image_path: &Path, options: &MarginOptions) -> Result<MarginDetection> {
        let img = image::open(image_path)
            .map_err(|e| MarginError::InvalidImage(e.to_string()))?;

        let gray = img.to_luma8();
        let (width, height) = img.dimensions();

        let is_background = |pixel: &image::Luma<u8>| -> bool {
            pixel.0[0] >= options.background_threshold
        };

        // 上マージン検出
        let top = Self::find_content_start_vertical(&gray, is_background, true);
        // 下マージン検出
        let bottom = height - Self::find_content_start_vertical(&gray, is_background, false);
        // 左マージン検出
        let left = Self::find_content_start_horizontal(&gray, is_background, true);
        // 右マージン検出
        let right = width - Self::find_content_start_horizontal(&gray, is_background, false);

        let margins = Margins {
            top: top.max(options.min_margin),
            bottom: bottom.max(options.min_margin),
            left: left.max(options.min_margin),
            right: right.max(options.min_margin),
        };

        let content_rect = ContentRect {
            x: margins.left,
            y: margins.top,
            width: width.saturating_sub(margins.total_horizontal()),
            height: height.saturating_sub(margins.total_vertical()),
        };

        if content_rect.width == 0 || content_rect.height == 0 {
            return Err(MarginError::NoContentDetected);
        }

        Ok(MarginDetection {
            margins,
            image_size: (width, height),
            content_rect,
            confidence: 1.0,
        })
    }

    fn find_content_start_vertical<F>(
        gray: &GrayImage,
        is_background: F,
        from_top: bool,
    ) -> u32
    where
        F: Fn(&image::Luma<u8>) -> bool,
    {
        let (width, height) = gray.dimensions();
        let rows: Box<dyn Iterator<Item = u32>> = if from_top {
            Box::new(0..height)
        } else {
            Box::new((0..height).rev())
        };

        for y in rows {
            let non_bg_count = (0..width)
                .filter(|&x| !is_background(gray.get_pixel(x, y)))
                .count();

            // 10%以上の非背景ピクセルがあればコンテンツ開始
            if non_bg_count as f32 / width as f32 > 0.1 {
                return if from_top { y } else { height - y };
            }
        }

        0
    }

    fn find_content_start_horizontal<F>(
        gray: &GrayImage,
        is_background: F,
        from_left: bool,
    ) -> u32
    where
        F: Fn(&image::Luma<u8>) -> bool,
    {
        let (width, height) = gray.dimensions();
        let cols: Box<dyn Iterator<Item = u32>> = if from_left {
            Box::new(0..width)
        } else {
            Box::new((0..width).rev())
        };

        for x in cols {
            let non_bg_count = (0..height)
                .filter(|&y| !is_background(gray.get_pixel(x, y)))
                .count();

            if non_bg_count as f32 / height as f32 > 0.1 {
                return if from_left { x } else { width - x };
            }
        }

        0
    }

    pub fn detect_unified(
        images: &[PathBuf],
        options: &MarginOptions,
    ) -> Result<UnifiedMargins> {
        let detections: Vec<MarginDetection> = images
            .par_iter()
            .map(|path| Self::detect(path, options))
            .collect::<Result<Vec<_>>>()?;

        // 最小マージンを使用（コンテンツを切らない）
        let margins = Margins {
            top: detections.iter().map(|d| d.margins.top).min().unwrap_or(0),
            bottom: detections.iter().map(|d| d.margins.bottom).min().unwrap_or(0),
            left: detections.iter().map(|d| d.margins.left).min().unwrap_or(0),
            right: detections.iter().map(|d| d.margins.right).min().unwrap_or(0),
        };

        // 統一サイズ（最大コンテンツサイズ）
        let max_content_width = detections.iter().map(|d| d.content_rect.width).max().unwrap_or(0);
        let max_content_height = detections.iter().map(|d| d.content_rect.height).max().unwrap_or(0);

        Ok(UnifiedMargins {
            margins,
            page_detections: detections,
            unified_size: (max_content_width, max_content_height),
        })
    }
}
```

---

## Acceptance Criteria

- [ ] 単一画像のマージンを正確に検出できる
- [ ] マージンなし画像で小さな値を返す
- [ ] 非対称マージンを正しく検出できる
- [ ] 複数画像の統一マージンを算出できる
- [ ] マージントリミングが正しく動作する
- [ ] パディング追加が正しく動作する
- [ ] 背景閾値設定が機能する
- [ ] エッジ検出モードが動作する
- [ ] バッチ処理で全画像が統一サイズになる
- [ ] コンテンツなし画像でエラーを返す

---

## Dependencies

```toml
[dependencies]
image = "0.25"
imageproc = "0.25"
thiserror = "2"
rayon = "1.10"

[dev-dependencies]
tempfile = "3"
```
