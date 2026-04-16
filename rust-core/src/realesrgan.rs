//! RealESRGAN Integration module
//!
//! Provides integration with RealESRGAN AI upscaling model.
//!
//! # Features
//!
//! - 2x/4x AI upscaling for scanned images
//! - Multiple model support (general, anime, video)
//! - VRAM-aware tile processing
//! - GPU acceleration with fallback
//!
//! # Example
//!
//! ```rust,no_run
//! use superbook_pdf::{RealEsrgan, RealEsrganOptions};
//!
//! // Configure upscaling
//! let options = RealEsrganOptions::builder()
//!     .scale(2)
//!     .tile_size(400)
//!     .build();
//!
//! // Upscale an image
//! // let result = RealEsrgan::new().upscale("input.png", "output.png", &options);
//! ```

use std::path::{Path, PathBuf};
use std::time::Duration;
use std::sync::Arc;
use thiserror::Error;
use async_trait::async_trait;
use crate::ai_bridge::{AiBridge, AiBridgeError, AiTool};

// ============================================================
// Constants
// ============================================================

/// Default tile size for balanced performance
const DEFAULT_TILE_SIZE: u32 = 400;

/// Tile size for high quality processing (more VRAM)
#[allow(dead_code)]
const HIGH_QUALITY_TILE_SIZE: u32 = 512;

/// Tile size for anime content optimization
const ANIME_TILE_SIZE: u32 = 256;

/// Tile size for low VRAM environments
const LOW_VRAM_TILE_SIZE: u32 = 128;

/// Default tile padding
const DEFAULT_TILE_PADDING: u32 = 10;

/// Minimum allowed tile size
const MIN_TILE_SIZE: u32 = 64;

/// Maximum allowed tile size
const MAX_TILE_SIZE: u32 = 1024;

/// Default scale factor
const DEFAULT_SCALE: u32 = 2;

/// Base VRAM for tile size calculation (4GB)
const BASE_VRAM_MB: u64 = 4096;

/// RealESRGAN error types
#[derive(Debug, Error)]
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

    #[error("Image error: {0}")]
    ImageError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, RealEsrganError>;

/// RealESRGAN options
#[derive(Debug, Clone)]
pub struct RealEsrganOptions {
    /// Upscale factor
    pub scale: u32,
    /// Model selection
    pub model: RealEsrganModel,
    /// Tile size (pixels)
    pub tile_size: u32,
    /// Tile padding
    pub tile_padding: u32,
    /// Output format
    pub output_format: OutputFormat,
    /// Enable face enhancement
    pub face_enhance: bool,
    /// GPU ID (None for auto)
    pub gpu_id: Option<u32>,
    /// Use FP16 for speed
    pub fp16: bool,
}

impl Default for RealEsrganOptions {
    fn default() -> Self {
        Self {
            scale: DEFAULT_SCALE,
            model: RealEsrganModel::X4Plus,
            tile_size: DEFAULT_TILE_SIZE,
            tile_padding: DEFAULT_TILE_PADDING,
            output_format: OutputFormat::Png,
            face_enhance: false,
            gpu_id: None,
            fp16: true,
        }
    }
}

impl RealEsrganOptions {
    /// Create a new options builder
    pub fn builder() -> RealEsrganOptionsBuilder {
        RealEsrganOptionsBuilder::default()
    }

    /// Create options for 4x upscaling (high quality)
    pub fn x4_high_quality() -> Self {
        Self {
            scale: 4,
            model: RealEsrganModel::X4Plus,
            tile_size: ANIME_TILE_SIZE, // Smaller tiles for quality
            fp16: false,                // More accurate
            ..Default::default()
        }
    }

    /// Create options optimized for anime/illustrations
    pub fn anime() -> Self {
        Self {
            scale: 4,
            model: RealEsrganModel::X4PlusAnime,
            ..Default::default()
        }
    }

    /// Create options for low VRAM (< 4GB)
    pub fn low_vram() -> Self {
        Self {
            tile_size: LOW_VRAM_TILE_SIZE,
            tile_padding: 8,
            fp16: true,
            ..Default::default()
        }
    }
}

/// Builder for RealEsrganOptions
#[derive(Debug, Default)]
pub struct RealEsrganOptionsBuilder {
    options: RealEsrganOptions,
}

impl RealEsrganOptionsBuilder {
    /// Set upscale factor (2 or 4)
    #[must_use]
    pub fn scale(mut self, scale: u32) -> Self {
        self.options.scale = if scale >= 4 { 4 } else { 2 };
        self
    }

    /// Set model type
    #[must_use]
    pub fn model(mut self, model: RealEsrganModel) -> Self {
        self.options.model = model;
        self
    }

    /// Set tile size for memory efficiency
    #[must_use]
    pub fn tile_size(mut self, size: u32) -> Self {
        self.options.tile_size = size.clamp(MIN_TILE_SIZE, MAX_TILE_SIZE);
        self
    }

    /// Set tile padding
    #[must_use]
    pub fn tile_padding(mut self, padding: u32) -> Self {
        self.options.tile_padding = padding;
        self
    }

    /// Set output format
    #[must_use]
    pub fn output_format(mut self, format: OutputFormat) -> Self {
        self.options.output_format = format;
        self
    }

    /// Enable face enhancement
    #[must_use]
    pub fn face_enhance(mut self, enable: bool) -> Self {
        self.options.face_enhance = enable;
        self
    }

    /// Set GPU device ID
    #[must_use]
    pub fn gpu_id(mut self, id: u32) -> Self {
        self.options.gpu_id = Some(id);
        self
    }

    /// Enable FP16 mode for speed
    #[must_use]
    pub fn fp16(mut self, enable: bool) -> Self {
        self.options.fp16 = enable;
        self
    }

    /// Build the options
    #[must_use]
    pub fn build(self) -> RealEsrganOptions {
        self.options
    }
}

/// RealESRGAN model types
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum RealEsrganModel {
    /// RealESRGAN_x4plus (high quality, general purpose)
    #[default]
    X4Plus,
    /// RealESRGAN_x4plus_anime (anime/illustration)
    X4PlusAnime,
    /// RealESRNet_x4plus (faster, slightly lower quality)
    NetX4Plus,
    /// RealESRGAN_x2plus
    X2Plus,
    /// Custom model
    Custom(String),
}

impl RealEsrganModel {
    /// Get default scale for model
    pub fn default_scale(&self) -> u32 {
        match self {
            Self::X4Plus | Self::X4PlusAnime | Self::NetX4Plus => 4,
            Self::X2Plus => 2,
            Self::Custom(_) => 4,
        }
    }

    /// Get model name
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

/// Output formats
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OutputFormat {
    #[default]
    Png,
    Jpg {
        quality: u8,
    },
    Webp {
        quality: u8,
    },
}

impl OutputFormat {
    /// Get file extension
    pub fn extension(&self) -> &str {
        match self {
            OutputFormat::Png => "png",
            OutputFormat::Jpg { .. } => "jpg",
            OutputFormat::Webp { .. } => "webp",
        }
    }
}

/// Upscale result for single image
#[derive(Debug, Clone)]
pub struct UpscaleResult {
    /// Input file path
    pub input_path: PathBuf,
    /// Output file path
    pub output_path: PathBuf,
    /// Original resolution
    pub original_size: (u32, u32),
    /// Upscaled resolution
    pub upscaled_size: (u32, u32),
    /// Actual scale factor
    pub actual_scale: f32,
    /// Processing time
    pub processing_time: Duration,
    /// VRAM usage (MB)
    pub vram_used_mb: Option<u64>,
}

/// Batch upscale result
#[derive(Debug)]
pub struct BatchUpscaleResult {
    /// Successful results
    pub successful: Vec<UpscaleResult>,
    /// Failed files
    pub failed: Vec<(PathBuf, String)>,
    /// Total processing time
    pub total_time: Duration,
    /// Peak VRAM usage
    pub peak_vram_mb: Option<u64>,
}

/// RealESRGAN processor trait
#[async_trait] // これを追加
pub trait RealEsrganProcessor: Send + Sync {
    /// 単一画像の超解像 (非同期)
    async fn upscale(
        &self,
        input_path: &Path,
        output_path: &Path,
        options: &RealEsrganOptions,
    ) -> Result<UpscaleResult>;

    /// 複数画像のバッチ超解像 (非同期)
    async fn upscale_batch(
        &self,
        input_files: &[PathBuf],
        output_dir: &Path,
        options: &RealEsrganOptions,
        progress: Option<Box<dyn Fn(usize, usize) + Send + Sync>>,
    ) -> Result<BatchUpscaleResult>;

    /// ディレクトリ内の全画像の超解像 (非同期)
    async fn upscale_directory(
        &self,
        input_dir: &Path,
        output_dir: &Path,
        options: &RealEsrganOptions,
        progress: Option<Box<dyn Fn(usize, usize) + Send + Sync>>,
    ) -> Result<BatchUpscaleResult>;

    fn available_models(&self) -> Vec<RealEsrganModel>;
    fn recommended_tile_size(&self, image_size: (u32, u32), available_vram_mb: u64) -> u32;
}

/// RealESRGAN implementation
pub struct RealEsrgan {
    bridge: Arc<dyn AiBridge>,
}

impl RealEsrgan {
    pub fn new(bridge: Arc<dyn AiBridge>) -> Self {
        Self { bridge }
    }

    /// 単一画像の超解像処理 (既存の async fn)
    pub async fn upscale(
        &self,
        input_path: &Path,
        output_path: &Path,
        options: &RealEsrganOptions,
    ) -> Result<UpscaleResult> {
        // 入力ファイルの存在確認
        if !input_path.exists() {
            return Err(RealEsrganError::InputNotFound(input_path.to_path_buf()));
        }

        // 元画像のサイズを取得し、処理時間を計測開始 [2]
        let img = image::open(input_path).map_err(|e| RealEsrganError::ImageError(e.to_string()))?;
        let original_size = (img.width(), img.height());
        let start_time = std::time::Instant::now();

        // 出力先ディレクトリの準備
        let output_dir = output_path.parent().unwrap_or(Path::new("."));
        if !output_dir.exists() {
            std::fs::create_dir_all(output_dir)
                .map_err(|_| RealEsrganError::OutputNotWritable(output_dir.to_path_buf()))?;
        }

        // 🚀 AIブリッジ経由で実行。execute は非同期メソッドのため .await が必須 [2]
        let result = self
            .bridge
            .execute(
                AiTool::RealESRGAN,
                &[input_path.to_path_buf()],
                output_dir,
                options,
            )
            .await
            .map_err(RealEsrganError::BridgeError)?;

        // 実行結果の成否を確認
        if !result.failed_files.is_empty() {
            if let Some((_, error)) = result.failed_files.first() {
                return Err(RealEsrganError::ProcessingFailed(error.clone()));
            }
        }

        // 最終的な出力ファイルの存在を確認し、拡大後の画像サイズを取得 [6]
        if !output_path.exists() {
            return Err(RealEsrganError::ProcessingFailed(format!(
                "Output file not found: {}",
                output_path.display()
            )));
        }

        let output_img = image::open(output_path).map_err(|e| RealEsrganError::ImageError(e.to_string()))?;
        let upscaled_size = (output_img.width(), output_img.height());
        let actual_scale = upscaled_size.0 as f32 / original_size.0 as f32;

        // 🚀 修正(E0308対策): 末尾にセミコロン「;」を付けないでください
        // これにより、Result<UpscaleResult> が戻り値として返されます
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

    /// 🚀 修正: pub fn から pub async fn に変更
    pub async fn upscale_batch(
        &self,
        input_files: &[PathBuf],
        output_dir: &Path,
        options: &RealEsrganOptions,
        progress: Option<Box<dyn Fn(usize, usize) + Send + Sync>>,
    ) -> Result<BatchUpscaleResult> {
        let start_time = std::time::Instant::now();
        let mut successful = Vec::new();
        let mut failed = Vec::new();

        if !output_dir.exists() {
            std::fs::create_dir_all(output_dir)
                .map_err(|_| RealEsrganError::OutputNotWritable(output_dir.to_path_buf()))?;
        }

        for (i, input_path) in input_files.iter().enumerate() {
            let output_path = output_dir.join(format!(
                "{}{}x.png",
                input_path.file_stem().unwrap().to_string_lossy()
            ));

            // 🚀 修正: upscale() は非同期関数なので .await を追加
            match self.upscale(input_path, &output_path, options).await {
                Ok(res) => successful.push(res),
                Err(e) => failed.push((input_path.clone(), e.to_string())),
            }

            if let Some(ref cb) = progress {
                cb(i + 1, input_files.len());
            }
        }

        Ok(BatchUpscaleResult {
            successful,
            failed,
            total_time: start_time.elapsed(),
            peak_vram_mb: None,
        })
    }

    /// 🚀 修正: pub fn から pub async fn に変更
    pub async fn upscale_directory(
        &self,
        input_dir: &Path,
        output_dir: &Path,
        options: &RealEsrganOptions,
        progress: Option<Box<dyn Fn(usize, usize) + Send + Sync>>,
    ) -> Result<BatchUpscaleResult> {
        // ディレクトリ内のすべての対応画像ファイルを検索
        let mut input_files = Vec::new();
        let extensions = ["png", "jpg", "jpeg", "bmp", "tiff", "webp"];

        if input_dir.is_dir() {
            // ディレクトリを読み込み、エラーがあれば RealEsrganError::IoError に変換して返す [2]
            for entry in std::fs::read_dir(input_dir).map_err(RealEsrganError::IoError)? {
                let entry = entry.map_err(RealEsrganError::IoError)?;
                let path = entry.path();
                
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        let ext_lower = ext.to_string_lossy().to_lowercase();
                        // 定義された拡張子に一致する場合のみリストに追加 [1, 3]
                        if extensions.contains(&ext_lower.as_str()) {
                            input_files.push(path);
                        }
                    }
                }
            }
        }

        // ページ順などの整合性を保つため、ファイルパスでソート [3]
        input_files.sort();

        // 🚀 修正: upscale_batch() は非同期関数(async fn)として定義されているため、.await が必須です
        // これにより、Future オブジェクトではなく Result<BatchUpscaleResult> が返されます [1, 4]
        self.upscale_batch(&input_files, output_dir, options, progress).await
    }
    pub fn available_models_list(&self) -> Vec<RealEsrganModel> {
        vec![
            RealEsrganModel::X4Plus,
            RealEsrganModel::X4PlusAnime,
            RealEsrganModel::NetX4Plus,
            RealEsrganModel::X2Plus,
        ]
    }

    /// 🚀 修正(無限再帰対策): 同様に固有メソッド名を変更
    pub fn calculate_recommended_tile_size(&self, _image_size: (u32, u32), available_vram_mb: u64) -> u32 {
        let scale_factor = (available_vram_mb as f64 / BASE_VRAM_MB as f64).sqrt();
        let recommended = (DEFAULT_TILE_SIZE as f64 * scale_factor) as u32;
        recommended.clamp(MIN_TILE_SIZE, MAX_TILE_SIZE)
    }
}

#[async_trait]
impl RealEsrganProcessor for RealEsrgan {
    async fn upscale(&self, i: &Path, o: &Path, opt: &RealEsrganOptions) -> Result<UpscaleResult> {
        self.upscale(i, o, opt).await
    }

    async fn upscale_batch(
        &self, 
        files: &[PathBuf], 
        out: &Path, 
        opt: &RealEsrganOptions,
        prog: Option<Box<dyn Fn(usize, usize) + Send + Sync>>
    ) -> Result<BatchUpscaleResult> {
        self.upscale_batch(files, out, opt, prog).await
    }

    async fn upscale_directory(
        &self,
        dir: &Path,
        out: &Path,
        opt: &RealEsrganOptions,
        prog: Option<Box<dyn Fn(usize, usize) + Send + Sync>>
    ) -> Result<BatchUpscaleResult> {
        self.upscale_directory(dir, out, opt, prog).await
    }

    fn available_models(&self) -> Vec<RealEsrganModel> {
        // 🚀 改名した固有メソッドを呼び出すことで無限再帰を回避
        self.available_models_list()
    }

    fn recommended_tile_size(&self, size: (u32, u32), vram: u64) -> u32 {
        // 🚀 同様に固有メソッド名を呼ぶ
        self.calculate_recommended_tile_size(size, vram)
    }
}