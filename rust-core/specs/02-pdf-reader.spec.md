# 02-pdf-reader.spec.md - PDF Reader Specification

## Overview

PDFファイルの読み込みとメタデータ抽出を担当するモジュール。
`pdf` クレートまたは `lopdf` を使用してPDFを解析する。

---

## Responsibilities

1. PDFファイルのオープンと検証
2. ページ数の取得
3. ページサイズの取得
4. メタデータ（タイトル、作者等）の抽出
5. 埋め込み画像の情報取得
6. 暗号化PDFの検出

---

## Data Structures

```rust
use std::path::PathBuf;

/// PDF読み込み結果
#[derive(Debug, Clone)]
pub struct PdfDocument {
    pub path: PathBuf,
    pub page_count: usize,
    pub metadata: PdfMetadata,
    pub pages: Vec<PdfPage>,
    pub is_encrypted: bool,
}

/// PDFメタデータ
#[derive(Debug, Clone, Default)]
pub struct PdfMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
    pub creation_date: Option<String>,
    pub modification_date: Option<String>,
}

/// ページ情報
#[derive(Debug, Clone)]
pub struct PdfPage {
    pub index: usize,          // 0-indexed
    pub width_pt: f64,         // ポイント単位
    pub height_pt: f64,
    pub rotation: u16,         // 0, 90, 180, 270
    pub has_images: bool,
    pub has_text: bool,
}

/// エラー型
#[derive(Debug, thiserror::Error)]
pub enum PdfReaderError {
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Invalid PDF format: {0}")]
    InvalidFormat(String),

    #[error("Encrypted PDF not supported")]
    EncryptedPdf,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("PDF parse error: {0}")]
    ParseError(String),
}

pub type Result<T> = std::result::Result<T, PdfReaderError>;
```

---

## Public API

```rust
/// PdfReader トレイト
pub trait PdfReader {
    /// PDFファイルを開く
    fn open(path: impl AsRef<Path>) -> Result<PdfDocument>;

    /// ページ情報を取得
    fn get_page(&self, index: usize) -> Result<&PdfPage>;

    /// 全ページのイテレータ
    fn pages(&self) -> impl Iterator<Item = &PdfPage>;

    /// メタデータを取得
    fn metadata(&self) -> &PdfMetadata;

    /// PDFが暗号化されているか
    fn is_encrypted(&self) -> bool;
}

/// デフォルト実装
pub struct LopdfReader {
    document: lopdf::Document,
    info: PdfDocument,
}

impl LopdfReader {
    pub fn new(path: impl AsRef<Path>) -> Result<Self>;
}
```

---

## Test Cases

### TC-PDR-001: 正常なPDF読み込み

```rust
#[test]
fn test_open_valid_pdf() {
    let path = PathBuf::from("tests/fixtures/sample.pdf");
    let doc = LopdfReader::new(&path).unwrap();

    assert!(doc.info.page_count > 0);
    assert_eq!(doc.info.path, path);
}
```

### TC-PDR-002: 存在しないファイル

```rust
#[test]
fn test_open_nonexistent_file() {
    let result = LopdfReader::new("/nonexistent/file.pdf");

    assert!(matches!(result, Err(PdfReaderError::FileNotFound(_))));
}
```

### TC-PDR-003: 無効なPDFフォーマット

```rust
#[test]
fn test_open_invalid_pdf() {
    // Create a non-PDF file
    let temp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp.path(), "This is not a PDF").unwrap();

    let result = LopdfReader::new(temp.path());

    assert!(matches!(result, Err(PdfReaderError::InvalidFormat(_))));
}
```

### TC-PDR-004: ページ数取得

```rust
#[test]
fn test_page_count() {
    let doc = LopdfReader::new("tests/fixtures/10pages.pdf").unwrap();

    assert_eq!(doc.info.page_count, 10);
}
```

### TC-PDR-005: ページサイズ取得

```rust
#[test]
fn test_page_dimensions() {
    let doc = LopdfReader::new("tests/fixtures/a4.pdf").unwrap();
    let page = doc.get_page(0).unwrap();

    // A4: 595 x 842 points
    assert!((page.width_pt - 595.0).abs() < 1.0);
    assert!((page.height_pt - 842.0).abs() < 1.0);
}
```

### TC-PDR-006: メタデータ抽出

```rust
#[test]
fn test_metadata_extraction() {
    let doc = LopdfReader::new("tests/fixtures/with_metadata.pdf").unwrap();
    let meta = doc.metadata();

    assert!(meta.title.is_some());
    assert!(meta.author.is_some());
}
```

### TC-PDR-007: 回転ページの検出

```rust
#[test]
fn test_rotated_page() {
    let doc = LopdfReader::new("tests/fixtures/rotated.pdf").unwrap();
    let page = doc.get_page(0).unwrap();

    assert_eq!(page.rotation, 90);
}
```

### TC-PDR-008: 暗号化PDF検出

```rust
#[test]
fn test_encrypted_pdf_detection() {
    let doc = LopdfReader::new("tests/fixtures/encrypted.pdf").unwrap();

    assert!(doc.is_encrypted());
}
```

### TC-PDR-009: 大容量PDF（メモリ効率）

```rust
#[test]
fn test_large_pdf_memory() {
    // 1000ページのPDFでもメモリ使用量が適切であることを確認
    let doc = LopdfReader::new("tests/fixtures/large_1000pages.pdf").unwrap();

    assert_eq!(doc.info.page_count, 1000);

    // メモリ使用量チェック（プロセスメモリ < 500MB）
    let mem_info = procfs::process::Process::myself()
        .unwrap()
        .statm()
        .unwrap();
    let mem_mb = mem_info.resident * 4096 / 1024 / 1024;
    assert!(mem_mb < 500, "Memory usage too high: {}MB", mem_mb);
}
```

### TC-PDR-010: 並行読み込み

```rust
#[test]
fn test_concurrent_open() {
    use rayon::prelude::*;

    let paths: Vec<_> = (0..10)
        .map(|i| format!("tests/fixtures/sample_{}.pdf", i))
        .collect();

    let results: Vec<_> = paths
        .par_iter()
        .map(|p| LopdfReader::new(p))
        .collect();

    assert!(results.iter().all(|r| r.is_ok()));
}
```

---

## Implementation Notes

### lopdf を使用する場合

```rust
use lopdf::Document;

impl LopdfReader {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        if !path.exists() {
            return Err(PdfReaderError::FileNotFound(path.to_path_buf()));
        }

        let document = Document::load(path)
            .map_err(|e| PdfReaderError::ParseError(e.to_string()))?;

        let page_count = document.get_pages().len();
        let metadata = Self::extract_metadata(&document);
        let pages = Self::extract_pages(&document)?;
        let is_encrypted = document.is_encrypted();

        Ok(Self {
            document,
            info: PdfDocument {
                path: path.to_path_buf(),
                page_count,
                metadata,
                pages,
                is_encrypted,
            },
        })
    }

    fn extract_metadata(doc: &Document) -> PdfMetadata {
        // Extract from /Info dictionary
        todo!()
    }

    fn extract_pages(doc: &Document) -> Result<Vec<PdfPage>> {
        // Iterate through pages and extract dimensions
        todo!()
    }
}
```

---

## Acceptance Criteria

- [ ] 有効なPDFファイルを正常に開ける
- [ ] 存在しないファイルで適切なエラーを返す
- [ ] 無効なPDFフォーマットで適切なエラーを返す
- [ ] ページ数を正確に取得できる
- [ ] 各ページのサイズを正確に取得できる
- [ ] メタデータを抽出できる
- [ ] 回転ページを検出できる
- [ ] 暗号化PDFを検出できる
- [ ] 大容量PDFでもメモリ効率が良い
- [ ] 並行読み込みが安全に動作する

---

## Dependencies

```toml
[dependencies]
lopdf = "0.34"
thiserror = "2"

[dev-dependencies]
tempfile = "3"
procfs = "0.17"
rayon = "1.10"
```
