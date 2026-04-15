# 01-cli.spec.md - CLI Interface Specification

## Overview

`superbook-pdf` のコマンドラインインターフェース仕様。
clap を使用し、直感的かつ強力なオプション体系を提供する。

---

## Commands

### `convert` - メインコマンド

PDFの変換処理を実行する。

```bash
superbook-pdf convert <INPUT> [OUTPUT] [OPTIONS]
```

#### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `INPUT`  | Yes      | 入力PDFファイルまたはディレクトリ |
| `OUTPUT` | No       | 出力先ディレクトリ（省略時: `./output`） |

#### Options

| Option | Short | Type | Default | Description |
|--------|-------|------|---------|-------------|
| `--ocr` | `-o` | bool | false | YomiToku OCRを有効化 |
| `--upscale` | `-u` | bool | true | RealESRGAN 2x アップスケール |
| `--deskew` | `-d` | bool | true | 傾き補正を有効化 |
| `--margin-trim` | `-m` | f32 | 0.5 | マージントリム率 (%) |
| `--dpi` | | u32 | 300 | 出力DPI |
| `--threads` | `-t` | usize | auto | 並列処理スレッド数 |
| `--gpu` | `-g` | bool | true | GPU処理を有効化 |
| `--verbose` | `-v` | count | 0 | ログ詳細度 (-v, -vv, -vvv) |
| `--quiet` | `-q` | bool | false | 進捗表示を抑制 |
| `--dry-run` | | bool | false | 実際の処理を行わずプランを表示 |

---

## Test Cases

### TC-CLI-001: ヘルプ表示

```rust
#[test]
fn test_help_display() {
    let output = Command::new("superbook-pdf")
        .arg("--help")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("superbook-pdf"));
    assert!(stdout.contains("convert"));
}
```

### TC-CLI-002: バージョン表示

```rust
#[test]
fn test_version_display() {
    let output = Command::new("superbook-pdf")
        .arg("--version")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}
```

### TC-CLI-003: 入力ファイルなしエラー

```rust
#[test]
fn test_missing_input_error() {
    let output = Command::new("superbook-pdf")
        .arg("convert")
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("required"));
}
```

### TC-CLI-004: 存在しないファイルエラー

```rust
#[test]
fn test_nonexistent_file_error() {
    let output = Command::new("superbook-pdf")
        .args(["convert", "/nonexistent/path.pdf"])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found") || stderr.contains("does not exist"));
}
```

### TC-CLI-005: ディレクトリ入力処理

```rust
#[test]
fn test_directory_input() {
    let temp_dir = tempfile::tempdir().unwrap();
    // Create test PDF files in temp_dir

    let output = Command::new("superbook-pdf")
        .args(["convert", temp_dir.path().to_str().unwrap(), "--dry-run"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
}
```

### TC-CLI-006: オプション解析

```rust
#[test]
fn test_option_parsing() {
    let args = Args::try_parse_from([
        "superbook-pdf",
        "convert",
        "input.pdf",
        "--ocr",
        "--no-upscale",
        "--dpi", "600",
        "-vvv",
    ]).unwrap();

    assert!(args.ocr);
    assert!(!args.upscale);
    assert_eq!(args.dpi, 600);
    assert_eq!(args.verbose, 3);
}
```

### TC-CLI-007: 進捗バー表示

```rust
#[test]
fn test_progress_bar_display() {
    // indicatif ProgressBar が正しく表示されることを確認
    let pb = ProgressBar::new(100);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
        .unwrap());

    for i in 0..100 {
        pb.set_position(i);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    pb.finish_with_message("done");
}
```

---

## Data Structures

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "superbook-pdf")]
#[command(author = "DN_SuperBook_PDF_Converter Contributors")]
#[command(version)]
#[command(about = "High-quality PDF converter for scanned books", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Convert PDF files with AI enhancement
    Convert(ConvertArgs),
    /// Show system information
    Info,
}

#[derive(clap::Args)]
pub struct ConvertArgs {
    /// Input PDF file or directory
    pub input: PathBuf,

    /// Output directory
    #[arg(default_value = "./output")]
    pub output: PathBuf,

    /// Enable Japanese OCR (YomiToku)
    #[arg(short, long)]
    pub ocr: bool,

    /// Enable AI upscaling (RealESRGAN)
    #[arg(short, long, default_value_t = true)]
    pub upscale: bool,

    /// Enable deskew correction
    #[arg(short, long, default_value_t = true)]
    pub deskew: bool,

    /// Margin trim percentage
    #[arg(short, long, default_value_t = 0.5)]
    pub margin_trim: f32,

    /// Output DPI
    #[arg(long, default_value_t = 300)]
    pub dpi: u32,

    /// Number of parallel threads
    #[arg(short, long)]
    pub threads: Option<usize>,

    /// Enable GPU processing
    #[arg(short, long, default_value_t = true)]
    pub gpu: bool,

    /// Verbosity level (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Suppress progress output
    #[arg(short, long)]
    pub quiet: bool,

    /// Show execution plan without processing
    #[arg(long)]
    pub dry_run: bool,
}
```

---

## Exit Codes

| Code | Name | Description |
|------|------|-------------|
| 0 | SUCCESS | 正常終了 |
| 1 | GENERAL_ERROR | 一般的なエラー |
| 2 | INVALID_ARGS | 引数エラー |
| 3 | INPUT_NOT_FOUND | 入力ファイル/ディレクトリが見つからない |
| 4 | OUTPUT_ERROR | 出力エラー（書き込み権限など） |
| 5 | PROCESSING_ERROR | 処理中のエラー |
| 6 | GPU_ERROR | GPU初期化/処理エラー |
| 7 | EXTERNAL_TOOL_ERROR | 外部ツール（Python等）エラー |

---

## Acceptance Criteria

- [ ] `--help` で使用方法が表示される
- [ ] `--version` でバージョンが表示される
- [ ] 入力ファイル未指定時にエラーメッセージが表示される
- [ ] 存在しないファイル指定時にエラーメッセージが表示される
- [ ] ディレクトリ指定時に再帰的にPDFを検索する
- [ ] 全オプションが正しく解析される
- [ ] 進捗バーがターミナルに表示される
- [ ] `-q` オプションで進捗表示が抑制される
- [ ] Exit codeが適切に返される
