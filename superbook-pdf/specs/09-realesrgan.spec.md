# 09-realesrgan.spec.md - RealESRGAN Integration Specification

## Overview

RealESRGAN AI超解像モデルとの統合仕様。
AiBridgeを通じてPythonプロセスを制御し、画像アップスケーリングを実行。

---

## Responsibilities

1. RealESRGANプロセスの起動・制御
2. アップスケール処理の実行
3. タイル処理によるメモリ管理
4. 複数モデルのサポート
5. 処理結果の検証

---

## Data Structures

```rust
use std::path::{Path, PathBuf};

/// RealESRGANオプション
#[derive(Debug, Clone)]
pub struct RealEsrganOptions {
    /// アップスケール倍率
    pub scale: u32,
    /// モデル選択
    pub model: RealEsrganModel,
    /// タイルサイズ（ピクセル）
    pub tile_size: u32,
    /// タイルパディング
    pub tile_padding: u32,
    /// 出力フォーマット
    pub output_format: OutputFormat,
    /// 顔補正を有効化
    pub face_enhance: bool,
    /// GPU ID（Noneで自動）
    pub gpu_id: Option<u32>,
    /// FP16使用（高速化）
    pub fp16: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum RealEsrganModel {
    /// RealESRGAN_x4plus（高品質、汎用）
    #[default]
    X4Plus,
    /// RealESRGAN_x4plus_anime（アニメ/イラスト向け）
    X4PlusAnime,
    /// RealESRNet_x4plus（高速、やや低品質）
    NetX4Plus,
    /// RealESRGAN_x2plus
    X2Plus,
    /// カスタムモデル
    Custom(String),
}

impl RealEsrganModel {
    pub fn default_scale(&self) -> u32 {
        match self {
            Self::X4Plus | Self::X4PlusAnime | Self::NetX4Plus => 4,
            Self::X2Plus => 2,
            Self::Custom(_) => 4,
        }
    }

    pub fn model_name(&self) -> &str {
        match self {
            Self::X4Plus => "RealESRGAN_x4plus",
            Self::X4PlusAnime => "RealESRGAN_x4plus_anime_6B",
            Self::NetX4Plus => "RealESRNet_x4plus",
            Self::X2Plus => "RealESRGAN_x2plus",
            Self::Custom(name) => name,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum OutputFormat {
    #[default]
    Png,
    Jpg { quality: u8 },
    Webp { quality: u8 },
}

/// 処理結果
#[derive(Debug)]
pub struct UpscaleResult {
    /// 入力ファイル
    pub input_path: PathBuf,
    /// 出力ファイル
    pub output_path: PathBuf,
    /// 元の解像度
    pub original_size: (u32, u32),
    /// アップスケール後の解像度
    pub upscaled_size: (u32, u32),
    /// 実際の倍率
    pub actual_scale: f32,
    /// 処理時間
    pub processing_time: std::time::Duration,
    /// VRAM使用量（MB）
    pub vram_used_mb: Option<u64>,
}

/// バッチ処理結果
#[derive(Debug)]
pub struct BatchUpscaleResult {
    /// 成功した処理
    pub successful: Vec<UpscaleResult>,
    /// 失敗した処理
    pub failed: Vec<(PathBuf, String)>,
    /// 総処理時間
    pub total_time: std::time::Duration,
    /// ピークVRAM使用量
    pub peak_vram_mb: Option<u64>,
}

/// エラー型
#[derive(Debug, thiserror::Error)]
pub enum RealEsrganError {
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Invalid scale: {0} (must be 2 or 4)")]
    InvalidScale(u32),

    #[error("Input image not found: {0}")]
    InputNotFound(PathBuf),

    #[error("Output directory not writable: {0}")]
    OutputNotWritable(PathBuf),

    #[error("Processing failed: {0}")]
    ProcessingFailed(String),

    #[error("GPU memory insufficient (need {required}MB, available {available}MB)")]
    InsufficientVram { required: u64, available: u64 },

    #[error("Bridge error: {0}")]
    BridgeError(#[from] AiBridgeError),
}

pub type Result<T> = std::result::Result<T, RealEsrganError>;
```

---

## Public API

```rust
/// RealEsrganProcessor トレイト
pub trait RealEsrganProcessor {
    /// 単一画像をアップスケール
    fn upscale(
        &self,
        input_path: &Path,
        output_path: &Path,
        options: &RealEsrganOptions,
    ) -> Result<UpscaleResult>;

    /// バッチアップスケール
    fn upscale_batch(
        &self,
        input_files: &[PathBuf],
        output_dir: &Path,
        options: &RealEsrganOptions,
        progress: Option<Box<dyn Fn(usize, usize) + Send>>,
    ) -> Result<BatchUpscaleResult>;

    /// ディレクトリ内の全画像をアップスケール
    fn upscale_directory(
        &self,
        input_dir: &Path,
        output_dir: &Path,
        options: &RealEsrganOptions,
        progress: Option<Box<dyn Fn(usize, usize) + Send>>,
    ) -> Result<BatchUpscaleResult>;

    /// 利用可能なモデルを取得
    fn available_models(&self) -> Vec<RealEsrganModel>;

    /// 推奨タイルサイズを算出
    fn recommended_tile_size(&self, image_size: (u32, u32), available_vram_mb: u64) -> u32;
}

/// 実装
pub struct RealEsrgan {
    bridge: Box<dyn AiBridge>,
}

impl RealEsrgan {
    pub fn new(bridge: Box<dyn AiBridge>) -> Self {
        Self { bridge }
    }
}

impl Default for RealEsrganOptions {
    fn default() -> Self {
        Self {
            scale: 2,
            model: RealEsrganModel::X4Plus,
            tile_size: 400,
            tile_padding: 10,
            output_format: OutputFormat::Png,
            face_enhance: false,
            gpu_id: None,
            fp16: true,
        }
    }
}
```

---

## Test Cases

### TC-RES-001: 単一画像アップスケール

```rust
#[test]
fn test_single_image_upscale() {
    let processor = create_test_processor();
    let temp_dir = tempfile::tempdir().unwrap();
    let output = temp_dir.path().join("upscaled.png");

    let result = processor.upscale(
        Path::new("tests/fixtures/small_image.png"),
        &output,
        &RealEsrganOptions::default(),
    ).unwrap();

    assert!(output.exists());
    assert_eq!(result.actual_scale, 2.0);
    assert_eq!(result.upscaled_size.0, result.original_size.0 * 2);
    assert_eq!(result.upscaled_size.1, result.original_size.1 * 2);
}
```

### TC-RES-002: 4倍アップスケール

```rust
#[test]
fn test_4x_upscale() {
    let processor = create_test_processor();
    let temp_dir = tempfile::tempdir().unwrap();
    let output = temp_dir.path().join("upscaled.png");

    let result = processor.upscale(
        Path::new("tests/fixtures/small_image.png"),
        &output,
        &RealEsrganOptions {
            scale: 4,
            ..Default::default()
        },
    ).unwrap();

    assert_eq!(result.actual_scale, 4.0);
}
```

### TC-RES-003: バッチ処理

```rust
#[test]
fn test_batch_upscale() {
    let processor = create_test_processor();
    let temp_dir = tempfile::tempdir().unwrap();

    let input_files: Vec<_> = (1..=5)
        .map(|i| PathBuf::from(format!("tests/fixtures/image_{}.png", i)))
        .collect();

    let result = processor.upscale_batch(
        &input_files,
        temp_dir.path(),
        &RealEsrganOptions::default(),
        None,
    ).unwrap();

    assert_eq!(result.successful.len(), 5);
    assert!(result.failed.is_empty());
}
```

### TC-RES-004: ディレクトリ処理

```rust
#[test]
fn test_directory_upscale() {
    let processor = create_test_processor();
    let temp_dir = tempfile::tempdir().unwrap();

    let result = processor.upscale_directory(
        Path::new("tests/fixtures/images"),
        temp_dir.path(),
        &RealEsrganOptions::default(),
        None,
    ).unwrap();

    // ディレクトリ内の全画像が処理される
    assert!(!result.successful.is_empty());
}
```

### TC-RES-005: 異なるモデル使用

```rust
#[test]
fn test_different_models() {
    let processor = create_test_processor();
    let temp_dir = tempfile::tempdir().unwrap();

    for model in [
        RealEsrganModel::X4Plus,
        RealEsrganModel::X4PlusAnime,
        RealEsrganModel::NetX4Plus,
    ] {
        let output = temp_dir.path().join(format!("{:?}.png", model));
        let result = processor.upscale(
            Path::new("tests/fixtures/small_image.png"),
            &output,
            &RealEsrganOptions {
                model,
                ..Default::default()
            },
        );

        assert!(result.is_ok(), "Model {:?} failed", model);
    }
}
```

### TC-RES-006: タイルサイズ調整

```rust
#[test]
fn test_tile_size_adjustment() {
    let processor = create_test_processor();
    let temp_dir = tempfile::tempdir().unwrap();

    // 小さいタイルサイズ（メモリ節約）
    let result_small = processor.upscale(
        Path::new("tests/fixtures/large_image.png"),
        &temp_dir.path().join("small_tile.png"),
        &RealEsrganOptions {
            tile_size: 128,
            ..Default::default()
        },
    );

    // 大きいタイルサイズ（高速）
    let result_large = processor.upscale(
        Path::new("tests/fixtures/large_image.png"),
        &temp_dir.path().join("large_tile.png"),
        &RealEsrganOptions {
            tile_size: 512,
            ..Default::default()
        },
    );

    // どちらも成功するが、出力サイズは同じ
    assert!(result_small.is_ok());
    assert!(result_large.is_ok());
    assert_eq!(
        result_small.as_ref().unwrap().upscaled_size,
        result_large.as_ref().unwrap().upscaled_size
    );
}
```

### TC-RES-007: JPEG出力

```rust
#[test]
fn test_jpeg_output() {
    let processor = create_test_processor();
    let temp_dir = tempfile::tempdir().unwrap();
    let output = temp_dir.path().join("upscaled.jpg");

    processor.upscale(
        Path::new("tests/fixtures/small_image.png"),
        &output,
        &RealEsrganOptions {
            output_format: OutputFormat::Jpg { quality: 90 },
            ..Default::default()
        },
    ).unwrap();

    assert!(output.exists());
    // JPEGマジックバイト確認
    let bytes = std::fs::read(&output).unwrap();
    assert_eq!(&bytes[0..2], &[0xFF, 0xD8]);
}
```

### TC-RES-008: 進捗コールバック

```rust
#[test]
fn test_progress_callback() {
    let processor = create_test_processor();
    let temp_dir = tempfile::tempdir().unwrap();

    let progress = Arc::new(Mutex::new(Vec::new()));
    let progress_clone = progress.clone();

    let input_files: Vec<_> = (1..=3)
        .map(|i| PathBuf::from(format!("tests/fixtures/image_{}.png", i)))
        .collect();

    processor.upscale_batch(
        &input_files,
        temp_dir.path(),
        &RealEsrganOptions::default(),
        Some(Box::new(move |current, total| {
            progress_clone.lock().unwrap().push((current, total));
        })),
    ).unwrap();

    let recorded = progress.lock().unwrap();
    assert_eq!(recorded.len(), 3);
    assert_eq!(recorded.last(), Some(&(3, 3)));
}
```

### TC-RES-009: 推奨タイルサイズ算出

```rust
#[test]
fn test_recommended_tile_size() {
    let processor = create_test_processor();

    // 8GB VRAM
    let tile_8gb = processor.recommended_tile_size((1920, 1080), 8192);
    assert!(tile_8gb >= 400);

    // 4GB VRAM
    let tile_4gb = processor.recommended_tile_size((1920, 1080), 4096);
    assert!(tile_4gb < tile_8gb);

    // 2GB VRAM
    let tile_2gb = processor.recommended_tile_size((1920, 1080), 2048);
    assert!(tile_2gb < tile_4gb);
}
```

### TC-RES-010: 入力ファイルなしエラー

```rust
#[test]
fn test_input_not_found_error() {
    let processor = create_test_processor();
    let temp_dir = tempfile::tempdir().unwrap();

    let result = processor.upscale(
        Path::new("/nonexistent/image.png"),
        &temp_dir.path().join("output.png"),
        &RealEsrganOptions::default(),
    );

    assert!(matches!(result, Err(RealEsrganError::InputNotFound(_))));
}
```

---

## Implementation Notes

### AiBridgeを使用した実装

```rust
impl RealEsrgan {
    pub fn upscale(
        &self,
        input_path: &Path,
        output_path: &Path,
        options: &RealEsrganOptions,
    ) -> Result<UpscaleResult> {
        if !input_path.exists() {
            return Err(RealEsrganError::InputNotFound(input_path.to_path_buf()));
        }

        // 元画像のサイズを取得
        let img = image::open(input_path)
            .map_err(|e| RealEsrganError::ProcessingFailed(e.to_string()))?;
        let original_size = (img.width(), img.height());

        let start_time = std::time::Instant::now();

        // AiBridge経由で実行
        let result = self.bridge.execute(
            AiTool::RealESRGAN,
            &[input_path.to_path_buf()],
            output_path.parent().unwrap_or(Path::new(".")),
            options,
        ).map_err(|e| RealEsrganError::BridgeError(e))?;

        if !result.failed_files.is_empty() {
            let (_, error) = &result.failed_files[0];
            return Err(RealEsrganError::ProcessingFailed(error.clone()));
        }

        // 出力画像のサイズを確認
        let output_img = image::open(output_path)
            .map_err(|e| RealEsrganError::ProcessingFailed(e.to_string()))?;
        let upscaled_size = (output_img.width(), output_img.height());

        let actual_scale = upscaled_size.0 as f32 / original_size.0 as f32;

        Ok(UpscaleResult {
            input_path: input_path.to_path_buf(),
            output_path: output_path.to_path_buf(),
            original_size,
            upscaled_size,
            actual_scale,
            processing_time: start_time.elapsed(),
            vram_used_mb: result.gpu_stats.map(|s| s.peak_vram_mb),
        })
    }

    pub fn upscale_batch(
        &self,
        input_files: &[PathBuf],
        output_dir: &Path,
        options: &RealEsrganOptions,
        progress: Option<Box<dyn Fn(usize, usize) + Send>>,
    ) -> Result<BatchUpscaleResult> {
        let start_time = std::time::Instant::now();
        let mut successful = Vec::new();
        let mut failed = Vec::new();

        for (i, input_path) in input_files.iter().enumerate() {
            let output_path = output_dir.join(
                input_path.file_name().unwrap_or_default()
            );

            match self.upscale(input_path, &output_path, options) {
                Ok(result) => successful.push(result),
                Err(e) => failed.push((input_path.clone(), e.to_string())),
            }

            if let Some(ref callback) = progress {
                callback(i + 1, input_files.len());
            }
        }

        Ok(BatchUpscaleResult {
            successful,
            failed,
            total_time: start_time.elapsed(),
            peak_vram_mb: None, // TODO
        })
    }

    pub fn recommended_tile_size(&self, image_size: (u32, u32), available_vram_mb: u64) -> u32 {
        // 経験則に基づく推奨タイルサイズ
        // 4x upscale with FP16: ~100MB per 400x400 tile
        let base_tile = 400;
        let base_vram = 4096; // 4GB

        let scale_factor = (available_vram_mb as f64 / base_vram as f64).sqrt();
        let recommended = (base_tile as f64 * scale_factor) as u32;

        // 最小128、最大1024
        recommended.clamp(128, 1024)
    }
}
```

---

## Acceptance Criteria

- [ ] 単一画像のアップスケールが正常に動作する
- [ ] 4倍アップスケールが正しく機能する
- [ ] バッチ処理が正常に動作する
- [ ] ディレクトリ処理が正常に動作する
- [ ] 異なるモデルで処理できる
- [ ] タイルサイズ調整が機能する
- [ ] JPEG出力が正しく動作する
- [ ] 進捗コールバックが呼ばれる
- [ ] 推奨タイルサイズが適切に算出される
- [ ] 存在しないファイルで適切なエラーを返す

---

## Dependencies

```toml
[dependencies]
image = "0.25"
thiserror = "2"

[dev-dependencies]
tempfile = "3"
```
