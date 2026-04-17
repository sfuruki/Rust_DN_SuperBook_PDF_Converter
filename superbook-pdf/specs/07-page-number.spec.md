# 07-page-number.spec.md - Page Number Detection Specification

## Overview

スキャン画像からページ番号を検出し、オフセット補正に使用するモジュール。
OCR（Tesseract）とヒューリスティック分析を組み合わせて使用。

---

## Responsibilities

1. ページ番号の位置検出
2. ページ番号のOCR読み取り
3. 奇数/偶数ページパターンの分析
4. スキャンオフセットの算出
5. ページ順序の検証

---

## Data Structures

```rust
use std::path::Path;

/// ページ番号検出オプション
#[derive(Debug, Clone)]
pub struct PageNumberOptions {
    /// 検索領域（画像の上下何%を検索）
    pub search_region_percent: f32,
    /// OCR言語
    pub ocr_language: String,
    /// 最小信頼度閾値
    pub min_confidence: f32,
    /// 数字のみを検出
    pub numbers_only: bool,
    /// ページ番号の位置ヒント
    pub position_hint: Option<PageNumberPosition>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PageNumberPosition {
    /// 下端中央
    BottomCenter,
    /// 下端外側（奇数右、偶数左）
    BottomOutside,
    /// 下端内側
    BottomInside,
    /// 上端中央
    TopCenter,
    /// 上端外側
    TopOutside,
}

/// 検出されたページ番号
#[derive(Debug, Clone)]
pub struct DetectedPageNumber {
    /// ページインデックス（0-indexed）
    pub page_index: usize,
    /// 検出された番号
    pub number: Option<i32>,
    /// 検出位置（ピクセル座標）
    pub position: PageNumberRect,
    /// OCR信頼度
    pub confidence: f32,
    /// 生のOCRテキスト
    pub raw_text: String,
}

#[derive(Debug, Clone, Copy)]
pub struct PageNumberRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// ページ番号分析結果
#[derive(Debug, Clone)]
pub struct PageNumberAnalysis {
    /// 各ページの検出結果
    pub detections: Vec<DetectedPageNumber>,
    /// 推定されるページ番号位置パターン
    pub position_pattern: PageNumberPosition,
    /// 奇数ページのX座標オフセット（ピクセル）
    pub odd_page_offset_x: i32,
    /// 偶数ページのX座標オフセット
    pub even_page_offset_x: i32,
    /// 全体の検出信頼度
    pub overall_confidence: f32,
    /// 欠落ページ番号
    pub missing_pages: Vec<usize>,
    /// 重複ページ番号
    pub duplicate_pages: Vec<i32>,
}

/// オフセット補正結果
#[derive(Debug, Clone)]
pub struct OffsetCorrection {
    /// ページごとの水平オフセット
    pub page_offsets: Vec<(usize, i32)>,
    /// 推奨される統一オフセット
    pub unified_offset: i32,
}

/// エラー型
#[derive(Debug, thiserror::Error)]
pub enum PageNumberError {
    #[error("Image not found: {0}")]
    ImageNotFound(PathBuf),

    #[error("OCR failed: {0}")]
    OcrFailed(String),

    #[error("No page numbers detected")]
    NoPageNumbersDetected,

    #[error("Inconsistent page numbers")]
    InconsistentPageNumbers,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, PageNumberError>;
```

---

## Public API

```rust
/// PageNumberDetector トレイト
pub trait PageNumberDetector {
    /// 単一画像からページ番号を検出
    fn detect_single(
        image_path: &Path,
        page_index: usize,
        options: &PageNumberOptions,
    ) -> Result<DetectedPageNumber>;

    /// 複数画像を分析
    fn analyze_batch(
        images: &[PathBuf],
        options: &PageNumberOptions,
    ) -> Result<PageNumberAnalysis>;

    /// オフセット補正値を算出
    fn calculate_offset(
        analysis: &PageNumberAnalysis,
        image_width: u32,
    ) -> Result<OffsetCorrection>;

    /// ページ順序を検証
    fn validate_order(
        analysis: &PageNumberAnalysis,
    ) -> Result<bool>;
}

/// Tesseract実装
pub struct TesseractPageDetector;

impl Default for PageNumberOptions {
    fn default() -> Self {
        Self {
            search_region_percent: 10.0, // 下端10%を検索
            ocr_language: "jpn+eng".to_string(),
            min_confidence: 60.0,
            numbers_only: true,
            position_hint: None,
        }
    }
}
```

---

## Test Cases

### TC-PGN-001: 単一ページ番号検出

```rust
#[test]
fn test_detect_single_page_number() {
    let detection = TesseractPageDetector::detect_single(
        Path::new("tests/fixtures/page_with_number_42.png"),
        0,
        &PageNumberOptions::default(),
    ).unwrap();

    assert_eq!(detection.number, Some(42));
    assert!(detection.confidence > 0.6);
}
```

### TC-PGN-002: ページ番号なし

```rust
#[test]
fn test_no_page_number() {
    let detection = TesseractPageDetector::detect_single(
        Path::new("tests/fixtures/page_no_number.png"),
        0,
        &PageNumberOptions::default(),
    ).unwrap();

    assert!(detection.number.is_none());
}
```

### TC-PGN-003: バッチ分析

```rust
#[test]
fn test_batch_analysis() {
    let images: Vec<_> = (1..=10)
        .map(|i| PathBuf::from(format!("tests/fixtures/book_page_{}.png", i)))
        .collect();

    let analysis = TesseractPageDetector::analyze_batch(
        &images,
        &PageNumberOptions::default(),
    ).unwrap();

    assert_eq!(analysis.detections.len(), 10);
    assert!(analysis.overall_confidence > 0.5);
}
```

### TC-PGN-004: 奇偶パターン検出

```rust
#[test]
fn test_odd_even_pattern() {
    let images: Vec<_> = (1..=6)
        .map(|i| PathBuf::from(format!("tests/fixtures/book_page_{}.png", i)))
        .collect();

    let analysis = TesseractPageDetector::analyze_batch(
        &images,
        &PageNumberOptions::default(),
    ).unwrap();

    // 奇数ページと偶数ページでオフセットが異なる
    assert_ne!(analysis.odd_page_offset_x, analysis.even_page_offset_x);
}
```

### TC-PGN-005: オフセット算出

```rust
#[test]
fn test_offset_calculation() {
    let images: Vec<_> = (1..=10)
        .map(|i| PathBuf::from(format!("tests/fixtures/book_page_{}.png", i)))
        .collect();

    let analysis = TesseractPageDetector::analyze_batch(
        &images,
        &PageNumberOptions::default(),
    ).unwrap();

    let offset = TesseractPageDetector::calculate_offset(&analysis, 2480).unwrap();

    // 各ページにオフセットが設定される
    assert!(!offset.page_offsets.is_empty());
}
```

### TC-PGN-006: 欠落ページ検出

```rust
#[test]
fn test_missing_page_detection() {
    // ページ1,2,3,5,6（4が欠落）
    let images = vec![
        PathBuf::from("tests/fixtures/page_1.png"),
        PathBuf::from("tests/fixtures/page_2.png"),
        PathBuf::from("tests/fixtures/page_3.png"),
        PathBuf::from("tests/fixtures/page_5.png"),
        PathBuf::from("tests/fixtures/page_6.png"),
    ];

    let analysis = TesseractPageDetector::analyze_batch(
        &images,
        &PageNumberOptions::default(),
    ).unwrap();

    assert!(analysis.missing_pages.contains(&3)); // インデックス3（ページ4）が欠落
}
```

### TC-PGN-007: 重複ページ検出

```rust
#[test]
fn test_duplicate_page_detection() {
    // ページ番号5が2回出現
    let images = vec![
        PathBuf::from("tests/fixtures/page_4.png"),
        PathBuf::from("tests/fixtures/page_5.png"),
        PathBuf::from("tests/fixtures/page_5_dup.png"),
        PathBuf::from("tests/fixtures/page_6.png"),
    ];

    let analysis = TesseractPageDetector::analyze_batch(
        &images,
        &PageNumberOptions::default(),
    ).unwrap();

    assert!(analysis.duplicate_pages.contains(&5));
}
```

### TC-PGN-008: ローマ数字対応

```rust
#[test]
fn test_roman_numerals() {
    let detection = TesseractPageDetector::detect_single(
        Path::new("tests/fixtures/page_roman_iii.png"),
        0,
        &PageNumberOptions {
            numbers_only: false,
            ..Default::default()
        },
    ).unwrap();

    // ローマ数字 "iii" を 3 として解釈
    assert_eq!(detection.number, Some(3));
}
```

### TC-PGN-009: 位置ヒント使用

```rust
#[test]
fn test_position_hint() {
    let detection = TesseractPageDetector::detect_single(
        Path::new("tests/fixtures/page_number_top.png"),
        0,
        &PageNumberOptions {
            position_hint: Some(PageNumberPosition::TopCenter),
            ..Default::default()
        },
    ).unwrap();

    // 上端でページ番号を検出
    assert!(detection.position.y < 100);
}
```

### TC-PGN-010: 低信頼度フィルタリング

```rust
#[test]
fn test_confidence_filtering() {
    let detection_high = TesseractPageDetector::detect_single(
        Path::new("tests/fixtures/blurry_page.png"),
        0,
        &PageNumberOptions {
            min_confidence: 80.0,
            ..Default::default()
        },
    ).unwrap();

    let detection_low = TesseractPageDetector::detect_single(
        Path::new("tests/fixtures/blurry_page.png"),
        0,
        &PageNumberOptions {
            min_confidence: 30.0,
            ..Default::default()
        },
    ).unwrap();

    // 高閾値ではNone、低閾値では検出される可能性
    assert!(detection_high.number.is_none() || detection_low.number.is_some());
}
```

---

## Implementation Notes

### Tesseract を使用した実装

```rust
use tesseract::Tesseract;
use image::{DynamicImage, GenericImageView};

impl TesseractPageDetector {
    pub fn detect_single(
        image_path: &Path,
        page_index: usize,
        options: &PageNumberOptions,
    ) -> Result<DetectedPageNumber> {
        let img = image::open(image_path)
            .map_err(|e| PageNumberError::ImageNotFound(image_path.to_path_buf()))?;

        let (width, height) = img.dimensions();

        // 検索領域を切り出し（下端の指定%）
        let search_height = (height as f32 * options.search_region_percent / 100.0) as u32;
        let search_region = img.crop_imm(0, height - search_height, width, search_height);

        // 一時ファイルに保存してTesseractで処理
        let temp_path = std::env::temp_dir().join(format!("pn_detect_{}.png", page_index));
        search_region.save(&temp_path)?;

        let mut tesseract = Tesseract::new(None, Some(&options.ocr_language))
            .map_err(|e| PageNumberError::OcrFailed(e.to_string()))?;

        tesseract = tesseract
            .set_image(&temp_path.to_string_lossy())
            .map_err(|e| PageNumberError::OcrFailed(e.to_string()))?;

        if options.numbers_only {
            tesseract = tesseract
                .set_variable("tessedit_char_whitelist", "0123456789")
                .map_err(|e| PageNumberError::OcrFailed(e.to_string()))?;
        }

        let raw_text = tesseract
            .get_text()
            .map_err(|e| PageNumberError::OcrFailed(e.to_string()))?;

        let confidence = tesseract
            .mean_text_conf()
            .unwrap_or(0) as f32;

        // 数字を抽出
        let number = Self::extract_page_number(&raw_text);

        // クリーンアップ
        let _ = std::fs::remove_file(&temp_path);

        Ok(DetectedPageNumber {
            page_index,
            number: if confidence >= options.min_confidence { number } else { None },
            position: PageNumberRect {
                x: 0, // TODO: 詳細な位置検出
                y: height - search_height,
                width,
                height: search_height,
            },
            confidence: confidence / 100.0,
            raw_text: raw_text.trim().to_string(),
        })
    }

    fn extract_page_number(text: &str) -> Option<i32> {
        // 数字のみを抽出
        let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();

        if digits.is_empty() {
            // ローマ数字チェック
            return Self::parse_roman_numeral(text);
        }

        digits.parse().ok()
    }

    fn parse_roman_numeral(text: &str) -> Option<i32> {
        let text = text.to_lowercase().trim().to_string();
        let roman_map = [
            ("m", 1000), ("cm", 900), ("d", 500), ("cd", 400),
            ("c", 100), ("xc", 90), ("l", 50), ("xl", 40),
            ("x", 10), ("ix", 9), ("v", 5), ("iv", 4), ("i", 1),
        ];

        let mut result = 0;
        let mut remaining = text.as_str();

        for (numeral, value) in &roman_map {
            while remaining.starts_with(numeral) {
                result += value;
                remaining = &remaining[numeral.len()..];
            }
        }

        if remaining.is_empty() && result > 0 {
            Some(result)
        } else {
            None
        }
    }

    pub fn analyze_batch(
        images: &[PathBuf],
        options: &PageNumberOptions,
    ) -> Result<PageNumberAnalysis> {
        let detections: Vec<DetectedPageNumber> = images
            .par_iter()
            .enumerate()
            .map(|(i, path)| Self::detect_single(path, i, options))
            .collect::<Result<Vec<_>>>()?;

        // パターン分析
        let (position_pattern, odd_offset, even_offset) = Self::analyze_pattern(&detections);

        // 欠落・重複検出
        let detected_numbers: Vec<_> = detections.iter().filter_map(|d| d.number).collect();
        let missing_pages = Self::find_missing_pages(&detected_numbers);
        let duplicate_pages = Self::find_duplicate_pages(&detected_numbers);

        let overall_confidence = detections.iter().map(|d| d.confidence).sum::<f32>() / detections.len() as f32;

        Ok(PageNumberAnalysis {
            detections,
            position_pattern,
            odd_page_offset_x: odd_offset,
            even_page_offset_x: even_offset,
            overall_confidence,
            missing_pages,
            duplicate_pages,
        })
    }

    fn analyze_pattern(detections: &[DetectedPageNumber]) -> (PageNumberPosition, i32, i32) {
        // 奇数/偶数ページの位置を分析
        // TODO: 実装
        (PageNumberPosition::BottomOutside, 0, 0)
    }

    fn find_missing_pages(numbers: &[i32]) -> Vec<usize> {
        if numbers.is_empty() {
            return vec![];
        }

        let min = *numbers.iter().min().unwrap();
        let max = *numbers.iter().max().unwrap();

        (min..=max)
            .filter(|n| !numbers.contains(n))
            .map(|n| (n - min) as usize)
            .collect()
    }

    fn find_duplicate_pages(numbers: &[i32]) -> Vec<i32> {
        let mut seen = std::collections::HashSet::new();
        numbers.iter().filter(|n| !seen.insert(*n)).cloned().collect()
    }
}
```

---

## Acceptance Criteria

- [ ] 単一ページからページ番号を正確に検出できる
- [ ] ページ番号なしの画像で適切にNoneを返す
- [ ] 複数ページのバッチ分析が正しく動作する
- [ ] 奇数/偶数ページのパターンを検出できる
- [ ] オフセット補正値を正しく算出できる
- [ ] 欠落ページを検出できる
- [ ] 重複ページを検出できる
- [ ] ローマ数字に対応できる
- [ ] 位置ヒントが機能する
- [ ] 信頼度フィルタリングが機能する

---

## Dependencies

```toml
[dependencies]
image = "0.25"
tesseract = "0.15"
thiserror = "2"
rayon = "1.10"

[dev-dependencies]
tempfile = "3"
```
