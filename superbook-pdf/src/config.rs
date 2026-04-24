//! Configuration file support for superbook-pdf
//!
//! Supports TOML configuration files with the following search order:
//! 1. `--config <path>` - explicitly specified path
//! 2. `./superbook.toml` - current directory
//! 3. `~/.config/superbook-pdf/config.toml` - user config
//! 4. Default values
//!
//! # Example Configuration
//!
//! ```toml
//! [general]
//! dpi = 300
//! threads = 4
//!
//! [processing]
//! deskew = true
//! margin_trim = 0.5
//!
//! [advanced]
//! internal_resolution = true
//! color_correction = true
//! ```

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::PipelineConfig;

/// Configuration file errors
#[derive(Debug, Error)]
pub enum ConfigError {
    /// IO error reading config file
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// TOML parse error
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    /// File not found
    #[error("Config file not found: {0}")]
    NotFound(PathBuf),

    /// Legacy key detected in pipeline config
    #[error("Unsupported legacy key '{0}'. Please migrate to the new concurrency keys.")]
    UnsupportedLegacyKey(String),
}

/// General configuration options
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GeneralConfig {
    /// Output DPI
    #[serde(default)]
    pub dpi: Option<u32>,

    /// Number of threads for parallel processing
    #[serde(default)]
    pub threads: Option<usize>,

    /// Verbosity level (0-2)
    #[serde(default)]
    pub verbose: Option<u8>,
}

/// Processing configuration options
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProcessingConfig {
    /// Enable deskew correction
    #[serde(default)]
    pub deskew: Option<bool>,

    /// Margin trim percentage
    #[serde(default)]
    pub margin_trim: Option<f64>,

    /// Enable AI upscaling
    #[serde(default)]
    pub upscale: Option<bool>,

    /// Enable GPU processing
    #[serde(default)]
    pub gpu: Option<bool>,

    // Issue #32: Content-aware margins
    /// Enable content-aware margin detection
    #[serde(default)]
    pub content_aware_margins: Option<bool>,

    /// Safety buffer percentage for margins (0.0-5.0)
    #[serde(default)]
    pub margin_safety: Option<f32>,

    /// Enable aggressive trimming
    #[serde(default)]
    pub aggressive_trim: Option<bool>,

}

/// Markdown conversion configuration (Issue #36)
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MarkdownConfig {
    /// Extract images from PDF
    #[serde(default)]
    pub extract_images: Option<bool>,

    /// Detect and convert tables
    #[serde(default)]
    pub detect_tables: Option<bool>,

    /// Text direction (auto, horizontal, vertical)
    #[serde(default)]
    pub text_direction: Option<String>,

    /// Include page numbers in output
    #[serde(default)]
    pub include_page_numbers: Option<bool>,

    /// Generate metadata JSON
    #[serde(default)]
    pub generate_metadata: Option<bool>,

    /// Validation settings
    #[serde(default)]
    pub validation: Option<MarkdownValidationConfig>,
}

/// Markdown validation configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MarkdownValidationConfig {
    /// Enable validation
    #[serde(default)]
    pub enabled: Option<bool>,

    /// API provider (claude, openai, local)
    #[serde(default)]
    pub provider: Option<String>,
}

/// Advanced processing configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AdvancedConfig {
    /// Enable internal resolution normalization (4960x7016)
    #[serde(default)]
    pub internal_resolution: Option<bool>,

    /// Enable global color correction
    #[serde(default)]
    pub color_correction: Option<bool>,

    /// Output height in pixels
    #[serde(default)]
    pub output_height: Option<u32>,
}

/// OCR configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct OcrConfig {
    /// Enable OCR
    #[serde(default)]
    pub enabled: Option<bool>,

    /// OCR language
    #[serde(default)]
    pub language: Option<String>,
}

/// Output configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct OutputConfig {
    /// JPEG quality (1-100)
    #[serde(default)]
    pub jpeg_quality: Option<u8>,

    /// Skip existing files
    #[serde(default)]
    pub skip_existing: Option<bool>,
}

/// Main configuration structure
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Config {
    /// General settings
    #[serde(default)]
    pub general: GeneralConfig,

    /// Processing settings
    #[serde(default)]
    pub processing: ProcessingConfig,

    /// Advanced settings
    #[serde(default)]
    pub advanced: AdvancedConfig,

    /// OCR settings
    #[serde(default)]
    pub ocr: OcrConfig,

    /// Output settings
    #[serde(default)]
    pub output: OutputConfig,

    /// Markdown conversion settings (Issue #36)
    #[serde(default)]
    pub markdown: MarkdownConfig,
}

impl Config {
    /// Create a new default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from the default search path
    ///
    /// Search order:
    /// 1. `./superbook.toml`
    /// 2. `~/.config/superbook-pdf/config.toml`
    /// 3. Default values (if no file found)
    pub fn load() -> Result<Self, ConfigError> {
        // Try current directory first
        let current_dir_config = PathBuf::from("superbook.toml");
        if current_dir_config.exists() {
            return Self::load_from_path(&current_dir_config);
        }

        // Try user config directory
        if let Some(config_dir) = dirs::config_dir() {
            let user_config = config_dir.join("superbook-pdf").join("config.toml");
            if user_config.exists() {
                return Self::load_from_path(&user_config);
            }
        }

        // Return default config if no file found
        Ok(Self::default())
    }

    /// Load configuration from a specific file path
    pub fn load_from_path(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Err(ConfigError::NotFound(path.to_path_buf()));
        }

        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Parse configuration from a TOML string
    pub fn from_toml(content: &str) -> Result<Self, ConfigError> {
        let config: Config = toml::from_str(content)?;
        Ok(config)
    }

    /// Serialize configuration to TOML string
    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    /// Convert to PipelineConfig
    pub fn to_pipeline_config(&self) -> PipelineConfig {
        let mut config = PipelineConfig::default();

        // Apply general settings
        if let Some(dpi) = self.general.dpi {
            config = config.with_dpi(dpi);
        }
        if let Some(threads) = self.general.threads {
            config.threads = Some(threads);
        }

        // Apply processing settings
        if let Some(deskew) = self.processing.deskew {
            config = config.with_deskew(deskew);
        }
        if let Some(margin_trim) = self.processing.margin_trim {
            config = config.with_margin_trim(margin_trim);
        }
        if let Some(upscale) = self.processing.upscale {
            config = config.with_upscale(upscale);
        }
        if let Some(gpu) = self.processing.gpu {
            config = config.with_gpu(gpu);
        }

        // Apply advanced settings
        if let Some(internal) = self.advanced.internal_resolution {
            config.internal_resolution = internal;
        }
        if let Some(color) = self.advanced.color_correction {
            config.color_correction = color;
        }
        if let Some(height) = self.advanced.output_height {
            config.output_height = height;
        }

        // Apply OCR settings
        if let Some(ocr) = self.ocr.enabled {
            config = config.with_ocr(ocr);
        }

        // Apply output settings
        if let Some(quality) = self.output.jpeg_quality {
            config.jpeg_quality = quality;
        }

        config
    }

    /// Merge with CLI arguments (CLI takes precedence)
    pub fn merge_with_cli(&self, cli: &CliOverrides) -> PipelineConfig {
        let mut config = self.to_pipeline_config();

        // CLI overrides take precedence
        if let Some(dpi) = cli.dpi {
            config = config.with_dpi(dpi);
        }
        if let Some(deskew) = cli.deskew {
            config = config.with_deskew(deskew);
        }
        if let Some(margin_trim) = cli.margin_trim {
            config = config.with_margin_trim(margin_trim);
        }
        if let Some(upscale) = cli.upscale {
            config = config.with_upscale(upscale);
        }
        if let Some(gpu) = cli.gpu {
            config = config.with_gpu(gpu);
        }
        if let Some(ocr) = cli.ocr {
            config = config.with_ocr(ocr);
        }
        if let Some(threads) = cli.threads {
            config.threads = Some(threads);
        }
        if let Some(internal) = cli.internal_resolution {
            config.internal_resolution = internal;
        }
        if let Some(color) = cli.color_correction {
            config.color_correction = color;
        }
        if let Some(height) = cli.output_height {
            config.output_height = height;
        }
        if let Some(quality) = cli.jpeg_quality {
            config.jpeg_quality = quality;
        }
        if let Some(max_pages) = cli.max_pages {
            config = config.with_max_pages(Some(max_pages));
        }
        if let Some(save_debug) = cli.save_debug {
            config.save_debug = save_debug;
        }

        config
    }

    /// Get config file search paths
    pub fn search_paths() -> Vec<PathBuf> {
        let mut paths = vec![PathBuf::from("superbook.toml")];

        if let Some(config_dir) = dirs::config_dir() {
            paths.push(config_dir.join("superbook-pdf").join("config.toml"));
        }

        paths
    }
}

/// CLI override values for merging with config file
#[derive(Debug, Clone, Default)]
pub struct CliOverrides {
    pub dpi: Option<u32>,
    pub deskew: Option<bool>,
    pub margin_trim: Option<f64>,
    pub upscale: Option<bool>,
    pub gpu: Option<bool>,
    pub ocr: Option<bool>,
    pub threads: Option<usize>,
    pub internal_resolution: Option<bool>,
    pub color_correction: Option<bool>,
    pub output_height: Option<u32>,
    pub jpeg_quality: Option<u8>,
    pub max_pages: Option<usize>,
    pub save_debug: Option<bool>,
}

impl CliOverrides {
    /// Create new empty overrides
    pub fn new() -> Self {
        Self::default()
    }

    /// Set DPI override
    pub fn with_dpi(mut self, dpi: u32) -> Self {
        self.dpi = Some(dpi);
        self
    }

    /// Set deskew override
    pub fn with_deskew(mut self, deskew: bool) -> Self {
        self.deskew = Some(deskew);
        self
    }

    /// Set margin trim override
    pub fn with_margin_trim(mut self, margin_trim: f64) -> Self {
        self.margin_trim = Some(margin_trim);
        self
    }

    /// Set upscale override
    pub fn with_upscale(mut self, upscale: bool) -> Self {
        self.upscale = Some(upscale);
        self
    }

    /// Set GPU override
    pub fn with_gpu(mut self, gpu: bool) -> Self {
        self.gpu = Some(gpu);
        self
    }

    /// Set OCR override
    pub fn with_ocr(mut self, ocr: bool) -> Self {
        self.ocr = Some(ocr);
        self
    }
}

    // ============================================================
    // PipelineTomlConfig（構築方針: pipeline.toml 形式）
    // ============================================================

    /// 傾き補正・色補正ステージの設定
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct CorrectTomlConfig {
        /// 傾き補正を有効にするか
        #[serde(default = "default_true_fn")]
        pub enable: bool,
        /// 傾き補正の強度（0.0-1.0）
        #[serde(default = "default_deskew_strength")]
        pub deskew_strength: f64,
        /// 色補正を有効にするか
        #[serde(default = "default_true_fn")]
        pub color_correction: bool,
    }

    impl Default for CorrectTomlConfig {
        fn default() -> Self {
            Self {
                enable: true,
                deskew_strength: 0.8,
                color_correction: true,
            }
        }
    }

    fn default_deskew_strength() -> f64 { 0.8 }
    fn default_true_fn() -> bool { true }

    /// AI 超解像ステージの設定
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct UpscaleTomlConfig {
        /// 拡大倍率（2 or 4）
        #[serde(default = "default_scale")]
        pub scale: u32,
        /// モデル名
        #[serde(default = "default_model")]
        pub model: String,
        /// 有効フラグ
        #[serde(default = "default_true_fn")]
        pub enable: bool,
        /// タイル処理サイズ（px）。0 = タイル無し
        #[serde(default = "default_tile")]
        pub tile: u32,
        /// FP32 モード（精度優先。GPU VRAM が少ない環境向け）
        #[serde(default)]
        pub fp32: bool,
    }

    impl Default for UpscaleTomlConfig {
        fn default() -> Self {
            Self {
                scale: 2,
                model: "realesrgan-x4plus".to_string(),
                enable: true,
                tile: 256,
                fp32: false,
            }
        }
    }

    fn default_scale() -> u32 { 2 }
    fn default_model() -> String { "realesrgan-x4plus".to_string() }
    fn default_tile() -> u32 { 256 }

    /// OCR ステージの設定
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct OcrTomlConfig {
        /// OCR を有効にするか
        #[serde(default)]
        pub enable: bool,
        /// OCR 言語
        #[serde(default = "default_language")]
        pub language: String,
        /// 確信度閾値
        #[serde(default = "default_confidence")]
        pub confidence: f64,
        /// 出力形式
        #[serde(default = "default_format")]
        pub format: String,
    }

    impl Default for OcrTomlConfig {
        fn default() -> Self {
            Self {
                enable: false,
                language: "jpn".to_string(),
                confidence: 0.5,
                format: "json".to_string(),
            }
        }
    }

    fn default_language() -> String { "jpn".to_string() }
    fn default_confidence() -> f64 { 0.5 }
    fn default_format() -> String { "json".to_string() }

    /// リトライ設定
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct RetryTomlConfig {
        /// 最大リトライ回数
        #[serde(default = "default_max_attempts")]
        pub max_attempts: u32,
        /// 初回バックオフ（ms）
        #[serde(default = "default_backoff_ms")]
        pub backoff_ms: u64,
    }

    impl Default for RetryTomlConfig {
        fn default() -> Self {
            Self {
                max_attempts: 3,
                backoff_ms: 500,
            }
        }
    }

    fn default_max_attempts() -> u32 { 3 }
    fn default_backoff_ms() -> u64 { 500 }

    /// 並列実行設定
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct ConcurrencyTomlConfig {
        /// 1ジョブ内のページ並列数
        #[serde(default = "default_page_parallel")]
        pub page_parallel: usize,
        /// 同時実行ジョブ数
        #[serde(default = "default_job_parallel")]
        pub job_parallel: usize,
        /// GPU ステージ（Upscale/OCR）同時実行数
        #[serde(default = "default_gpu_stage_parallel")]
        pub gpu_stage_parallel: usize,
        /// GPU 空きメモリの安全マージン（MB）
        #[serde(default = "default_gpu_safety_margin_mb")]
        pub gpu_safety_margin_mb: u64,
        /// GPU /status ポーリング間隔（ms）
        #[serde(default = "default_gpu_status_poll_ms")]
        pub gpu_status_poll_ms: u64,
        /// CPU 動的制御時の最小ページ並列数
        #[serde(default = "default_cpu_dynamic_min_parallel")]
        pub cpu_dynamic_min_parallel: usize,
        /// CPU 1分平均負荷 / コア の目標値
        #[serde(default = "default_cpu_target_load_per_core")]
        pub cpu_target_load_per_core: f64,
        /// CPU 負荷の再評価間隔（ms）
        #[serde(default = "default_cpu_status_poll_ms")]
        pub cpu_status_poll_ms: u64,
    }

    impl Default for ConcurrencyTomlConfig {
        fn default() -> Self {
            Self {
                page_parallel: 0,
                job_parallel: 0,
                gpu_stage_parallel: 0,
                gpu_safety_margin_mb: 3000,
                gpu_status_poll_ms: 100,
                cpu_dynamic_min_parallel: 0,
                cpu_target_load_per_core: 0.9,
                cpu_status_poll_ms: 200,
            }
        }
    }

    fn default_page_parallel() -> usize { 4 }
    fn default_job_parallel() -> usize { 2 }
    fn default_gpu_stage_parallel() -> usize { 4 }
    fn default_gpu_safety_margin_mb() -> u64 { 3000 }
    fn default_gpu_status_poll_ms() -> u64 { 100 }
    fn default_cpu_dynamic_min_parallel() -> usize { 1 }
    fn default_cpu_target_load_per_core() -> f64 { 0.9 }
    fn default_cpu_status_poll_ms() -> u64 { 200 }

    /// 抽出ステージの設定
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct LoadTomlConfig {
        /// 抽出 DPI
        #[serde(default = "default_dpi")]
        pub dpi: u32,
    }

    impl Default for LoadTomlConfig {
        fn default() -> Self {
            Self { dpi: 300 }
        }
    }

    fn default_dpi() -> u32 { 300 }

    /// 後処理ステージの設定
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct SaveTomlConfig {
        /// 出力高さ（px）
        #[serde(default = "default_output_height")]
        pub output_height: u32,
        /// JPEG クオリティ
        #[serde(default = "default_jpeg_quality")]
        pub jpeg_quality: u8,
    }

    impl Default for SaveTomlConfig {
        fn default() -> Self {
            Self {
                output_height: 3508,
                jpeg_quality: 90,
            }
        }
    }

    fn default_output_height() -> u32 { 3508 }
    fn default_jpeg_quality() -> u8 { 90 }

    /// クリーンアップ設定
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct CleanupTomlConfig {
        /// 成功時に work_dir を削除するか
        #[serde(default)]
        pub enable: bool,
    }

    impl Default for CleanupTomlConfig {
        fn default() -> Self {
            Self { enable: false }
        }
    }

    /// 検証ステージの設定
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct ValidationTomlConfig {
        /// 検証を有効にするか
        #[serde(default = "default_true_fn")]
        pub enable: bool,
        /// 最小文字数（OCR 品質チェック）
        #[serde(default)]
        pub min_chars: usize,
    }

    impl Default for ValidationTomlConfig {
        fn default() -> Self {
            Self {
                enable: true,
                min_chars: 0,
            }
        }
    }

    /// pipeline.toml のトップレベル設定（構築方針準拠）
    ///
    /// # Example pipeline.toml
    /// ```toml
    /// [load]
    /// dpi = 300
    ///
    /// [correct]
    /// enable = true
    /// deskew_strength = 0.8
    ///
    /// [upscale]
    /// scale = 2
    /// model = "realesrgan-x4plus"
    ///
    /// [ocr]
    /// enable = false
    /// language = "jpn"
    ///
    /// [retry]
    /// max_attempts = 3
    /// backoff_ms = 500
    ///
    /// [concurrency]
    /// page_parallel = 4
    /// job_parallel = 2
    /// gpu_stage_parallel = 4
    /// gpu_safety_margin_mb = 3000
    /// gpu_status_poll_ms = 100
    /// cpu_dynamic_min_parallel = 1
    /// cpu_target_load_per_core = 0.9
    /// cpu_status_poll_ms = 200
    /// ```
    #[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
    pub struct PipelineTomlConfig {
        /// 抽出ステージ設定
        #[serde(default)]
        pub load: LoadTomlConfig,
        /// 傾き補正・色補正ステージ設定
        #[serde(default)]
        pub correct: CorrectTomlConfig,
        /// AI 超解像ステージ設定
        #[serde(default)]
        pub upscale: UpscaleTomlConfig,
        /// OCR 前処理ステージ設定
        #[serde(default)]
        pub ocr_pre: OcrPreTomlConfig,
        /// OCR ステージ設定
        #[serde(default)]
        pub ocr: OcrTomlConfig,
        /// 後処理ステージ設定
        #[serde(default)]
        pub save: SaveTomlConfig,
        /// クリーンアップ設定
        #[serde(default)]
        pub cleanup: CleanupTomlConfig,
        /// 検証ステージ設定
        #[serde(default)]
        pub validation: ValidationTomlConfig,
        /// リトライ設定
        #[serde(default)]
        pub retry: RetryTomlConfig,
        /// 並列実行設定
        #[serde(default)]
        pub concurrency: ConcurrencyTomlConfig,
    }

    /// OCR 前処理ステージの設定
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct OcrPreTomlConfig {
        /// マージントリミング
        #[serde(default = "default_true_fn")]
        pub margin_trim: bool,
        /// 内部解像度正規化
        #[serde(default = "default_true_fn")]
        pub normalize_resolution: bool,
    }

    impl Default for OcrPreTomlConfig {
        fn default() -> Self {
            Self {
                margin_trim: false,
                normalize_resolution: false,
            }
        }
    }

    impl PipelineTomlConfig {
        fn dynamic_cpu_capacity() -> usize {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .max(1)
        }

        /// 1ジョブあたりのページ並列数
        pub fn resolved_page_parallel(&self) -> usize {
            if self.concurrency.page_parallel == 0 {
                Self::dynamic_cpu_capacity()
            } else {
                self.concurrency.page_parallel.max(1)
            }
        }

        /// 同時実行ジョブ数
        pub fn resolved_job_parallel(&self) -> usize {
            if self.concurrency.job_parallel == 0 {
                Self::dynamic_cpu_capacity()
            } else {
                self.concurrency.job_parallel.max(1)
            }
        }

        /// GPU ステージ同時実行数
        pub fn resolved_gpu_stage_parallel(&self) -> usize {
            if self.concurrency.gpu_stage_parallel == 0 {
                1024
            } else {
                self.concurrency.gpu_stage_parallel.max(1)
            }
        }

        /// GPU安全マージン（MB）
        pub fn resolved_gpu_safety_margin_mb(&self) -> u64 {
            self.concurrency.gpu_safety_margin_mb.max(1)
        }

        /// GPU statusポーリング間隔（ms）
        pub fn resolved_gpu_status_poll_ms(&self) -> u64 {
            self.concurrency.gpu_status_poll_ms.max(20)
        }

        /// CPU 動的制御時の最小ページ並列数
        pub fn resolved_cpu_dynamic_min_parallel(&self) -> usize {
            let min_parallel = if self.concurrency.cpu_dynamic_min_parallel == 0 {
                1
            } else {
                self.concurrency.cpu_dynamic_min_parallel.max(1)
            };
            min_parallel.min(self.resolved_page_parallel())
        }

        /// CPU 1分平均負荷 / コア の目標値
        pub fn resolved_cpu_target_load_per_core(&self) -> f64 {
            let value = self.concurrency.cpu_target_load_per_core;
            if value.is_finite() && value > 0.1 {
                value
            } else {
                0.9
            }
        }

        /// CPU 負荷の再評価間隔（ms）
        pub fn resolved_cpu_status_poll_ms(&self) -> u64 {
            self.concurrency.cpu_status_poll_ms.max(20)
        }

        /// pipeline.toml ファイルから読み込む
        pub fn load_from_path(path: &Path) -> Result<Self, ConfigError> {
            if !path.exists() {
                return Err(ConfigError::NotFound(path.to_path_buf()));
            }
            let content = std::fs::read_to_string(path)?;
            reject_legacy_parallel_keys(&content)?;
            let config: PipelineTomlConfig = toml::from_str(&content)?;
            Ok(config)
        }

        /// デフォルトの検索パスから読み込む
        ///
        /// 検索順序:
        /// 1. `--config` で指定したパス（呼び出し元で処理）
        /// 2. `./pipeline.toml`
        /// 3. `~/.config/superbook-pdf/pipeline.toml`
        /// 4. デフォルト値
        pub fn load() -> Result<Self, ConfigError> {
            let current = PathBuf::from("pipeline.toml");
            if current.exists() {
                return Self::load_from_path(&current);
            }
            if let Some(config_dir) = dirs::config_dir() {
                let user = config_dir.join("superbook-pdf").join("pipeline.toml");
                if user.exists() {
                    return Self::load_from_path(&user);
                }
            }
            Ok(Self::default())
        }

        /// TOML 文字列にシリアライズする
        pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
            toml::to_string_pretty(self)
        }

        /// TOML 文字列からパースする
        pub fn from_toml(content: &str) -> Result<Self, ConfigError> {
            reject_legacy_parallel_keys(content)?;
            let config: PipelineTomlConfig = toml::from_str(content)?;
            Ok(config)
        }
    }

    fn reject_legacy_parallel_keys(content: &str) -> Result<(), ConfigError> {
        if content.trim().is_empty() {
            return Ok(());
        }

        let value: toml::Value = toml::from_str(content)?;
        let Some(concurrency) = value.get("concurrency").and_then(|v| v.as_table()) else {
            return Ok(());
        };

        if concurrency.contains_key("max_parallel_pages") {
            return Err(ConfigError::UnsupportedLegacyKey(
                "concurrency.max_parallel_pages -> concurrency.page_parallel".to_string(),
            ));
        }

        if concurrency.contains_key("max_parallel_pages_cpu") {
            return Err(ConfigError::UnsupportedLegacyKey(
                "concurrency.max_parallel_pages_cpu -> concurrency.page_parallel".to_string(),
            ));
        }

        if concurrency.contains_key("max_parallel_pages_gpu") {
            return Err(ConfigError::UnsupportedLegacyKey(
                "concurrency.max_parallel_pages_gpu -> concurrency.gpu_stage_parallel".to_string(),
            ));
        }

        Ok(())
    }

    #[cfg(test)]
mod tests {
    use super::*;

    // CFG-001: Config::default
    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.general.dpi, None);
        assert_eq!(config.processing.deskew, None);
        assert_eq!(config.advanced.internal_resolution, None);
        assert_eq!(config.ocr.enabled, None);
        assert_eq!(config.output.jpeg_quality, None);
    }

    // CFG-002: Config::load_from_path (existing file)
    #[test]
    fn test_config_load_from_path_existing() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        std::fs::write(
            &config_path,
            r#"
[general]
dpi = 600

[processing]
deskew = true
"#,
        )
        .unwrap();

        let config = Config::load_from_path(&config_path).unwrap();
        assert_eq!(config.general.dpi, Some(600));
        assert_eq!(config.processing.deskew, Some(true));
    }

    // CFG-003: Config::load_from_path (non-existent file)
    #[test]
    fn test_config_load_from_path_not_found() {
        let result = Config::load_from_path(Path::new("/nonexistent/config.toml"));
        assert!(matches!(result, Err(ConfigError::NotFound(_))));
    }

    // CFG-004: Config::load (search order)
    #[test]
    fn test_config_search_paths() {
        let paths = Config::search_paths();
        assert!(!paths.is_empty());
        assert_eq!(paths[0], PathBuf::from("superbook.toml"));
    }

    // CFG-005: Config::merge (CLI priority)
    #[test]
    fn test_config_merge_cli_priority() {
        let config = Config {
            general: GeneralConfig {
                dpi: Some(300),
                ..Default::default()
            },
            processing: ProcessingConfig {
                deskew: Some(true),
                ..Default::default()
            },
            ..Default::default()
        };

        let cli = CliOverrides::new().with_dpi(600).with_deskew(false);

        let pipeline = config.merge_with_cli(&cli);
        assert_eq!(pipeline.dpi, 600); // CLI wins
        assert!(!pipeline.deskew); // CLI wins
    }

    // CFG-006: Config::to_pipeline_config
    #[test]
    fn test_config_to_pipeline_config() {
        let config = Config {
            general: GeneralConfig {
                dpi: Some(450),
                threads: Some(8),
                ..Default::default()
            },
            processing: ProcessingConfig {
                deskew: Some(false),
                margin_trim: Some(1.0),
                upscale: Some(true),
                gpu: Some(true),
                ..Default::default()
            },
            advanced: AdvancedConfig {
                internal_resolution: Some(true),
                color_correction: Some(true),
                output_height: Some(4000),
            },
            ocr: OcrConfig {
                enabled: Some(true),
                ..Default::default()
            },
            output: OutputConfig {
                jpeg_quality: Some(95),
                ..Default::default()
            },
            ..Default::default()
        };

        let pipeline = config.to_pipeline_config();
        assert_eq!(pipeline.dpi, 450);
        assert_eq!(pipeline.threads, Some(8));
        assert!(!pipeline.deskew);
        assert!((pipeline.margin_trim - 1.0).abs() < f64::EPSILON);
        assert!(pipeline.upscale);
        assert!(pipeline.gpu);
        assert!(pipeline.internal_resolution);
        assert!(pipeline.color_correction);
        assert_eq!(pipeline.output_height, 4000);
        assert!(pipeline.ocr);
        assert_eq!(pipeline.jpeg_quality, 95);
    }

    // CFG-007: TOML parse (complete config)
    #[test]
    fn test_config_toml_parse_complete() {
        let toml = r#"
[general]
dpi = 300
threads = 4
verbose = 2

[processing]
deskew = true
margin_trim = 0.5
upscale = true
gpu = true

[advanced]
internal_resolution = true
color_correction = true
output_height = 3508

[ocr]
enabled = true
language = "ja"

[output]
jpeg_quality = 90
skip_existing = true
"#;

        let config = Config::from_toml(toml).unwrap();
        assert_eq!(config.general.dpi, Some(300));
        assert_eq!(config.general.threads, Some(4));
        assert_eq!(config.general.verbose, Some(2));
        assert_eq!(config.processing.deskew, Some(true));
        assert_eq!(config.processing.margin_trim, Some(0.5));
        assert_eq!(config.advanced.internal_resolution, Some(true));
        assert_eq!(config.ocr.language, Some("ja".to_string()));
        assert_eq!(config.output.jpeg_quality, Some(90));
        assert_eq!(config.output.skip_existing, Some(true));
    }

    // CFG-008: TOML parse (partial config)
    #[test]
    fn test_config_toml_parse_partial() {
        let toml = r#"
[general]
dpi = 600
"#;

        let config = Config::from_toml(toml).unwrap();
        assert_eq!(config.general.dpi, Some(600));
        assert_eq!(config.general.threads, None);
        assert_eq!(config.processing.deskew, None);
    }

    // CFG-009: TOML parse (empty file)
    #[test]
    fn test_config_toml_parse_empty() {
        let config = Config::from_toml("").unwrap();
        assert_eq!(config, Config::default());
    }

    // CFG-010: TOML parse (invalid format)
    #[test]
    fn test_config_toml_parse_invalid() {
        let result = Config::from_toml("this is not valid toml [[[");
        assert!(matches!(result, Err(ConfigError::TomlParse(_))));
    }

    #[test]
    fn test_config_to_toml() {
        let config = Config {
            general: GeneralConfig {
                dpi: Some(300),
                ..Default::default()
            },
            ..Default::default()
        };

        let toml_str = config.to_toml().unwrap();
        assert!(toml_str.contains("dpi = 300"));
    }

    #[test]
    fn test_cli_overrides_builder() {
        let overrides = CliOverrides::new()
            .with_dpi(600)
            .with_deskew(false)
            .with_margin_trim(1.5)
            .with_upscale(true)
            .with_gpu(false)
            .with_ocr(true);

        assert_eq!(overrides.dpi, Some(600));
        assert_eq!(overrides.deskew, Some(false));
        assert_eq!(overrides.margin_trim, Some(1.5));
        assert_eq!(overrides.upscale, Some(true));
        assert_eq!(overrides.gpu, Some(false));
        assert_eq!(overrides.ocr, Some(true));
    }

    #[test]
    fn test_config_error_display() {
        let err = ConfigError::NotFound(PathBuf::from("/test/path"));
        assert!(err.to_string().contains("Config file not found"));
    }

    #[test]
    fn test_config_new() {
        let config = Config::new();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn test_config_merge_empty_cli() {
        let config = Config {
            general: GeneralConfig {
                dpi: Some(300),
                ..Default::default()
            },
            ..Default::default()
        };

        let cli = CliOverrides::new();
        let pipeline = config.merge_with_cli(&cli);
        assert_eq!(pipeline.dpi, 300); // Config value preserved
    }

    #[test]
    fn test_config_merge_partial_cli() {
        let config = Config {
            general: GeneralConfig {
                dpi: Some(300),
                threads: Some(4),
                ..Default::default()
            },
            processing: ProcessingConfig {
                deskew: Some(true),
                margin_trim: Some(0.5),
                ..Default::default()
            },
            ..Default::default()
        };

        let cli = CliOverrides::new().with_dpi(600);
        let pipeline = config.merge_with_cli(&cli);
        assert_eq!(pipeline.dpi, 600); // CLI wins
        assert_eq!(pipeline.threads, Some(4)); // Config preserved
        assert!(pipeline.deskew); // Config preserved
    }

    #[test]
    fn test_pipeline_toml_default_concurrency_values() {
        let cfg = PipelineTomlConfig::default();
        let expected_cpu = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .max(1);
        assert_eq!(cfg.resolved_page_parallel(), expected_cpu);
        assert_eq!(cfg.resolved_job_parallel(), expected_cpu);
        assert_eq!(cfg.resolved_gpu_stage_parallel(), 1024);
    }

    #[test]
    fn test_pipeline_toml_zero_means_dynamic_auto() {
        let mut cfg = PipelineTomlConfig::default();
        cfg.concurrency.page_parallel = 0;
        cfg.concurrency.job_parallel = 0;
        cfg.concurrency.gpu_stage_parallel = 0;

        let expected_cpu = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .max(1);
        assert_eq!(cfg.resolved_page_parallel(), expected_cpu);
        assert_eq!(cfg.resolved_job_parallel(), expected_cpu);
        assert_eq!(cfg.resolved_gpu_stage_parallel(), 1024);
    }

    #[test]
    fn test_pipeline_toml_rejects_legacy_parallel_keys() {
        let toml = r#"
[concurrency]
max_parallel_pages_cpu = 2
max_parallel_pages_gpu = 4
"#;

        let err = PipelineTomlConfig::from_toml(toml).unwrap_err();
        assert!(matches!(err, ConfigError::UnsupportedLegacyKey(_)));
        assert!(err.to_string().contains("Unsupported legacy key"));
    }

    #[test]
    fn test_pipeline_toml_rejects_legacy_parallel_single_key() {
        let toml = r#"
[concurrency]
max_parallel_pages = 2
"#;

        let err = PipelineTomlConfig::from_toml(toml).unwrap_err();
        assert!(matches!(err, ConfigError::UnsupportedLegacyKey(_)));
        assert!(err.to_string().contains("Unsupported legacy key"));
    }
}
