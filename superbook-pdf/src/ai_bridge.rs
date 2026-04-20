//! AI Tools Bridge module
//!
//! Provides communication with external AI services (`RealESRGAN`, `YomiToku`).
//!
//! # Features
//!
//! - HTTP API communication for AI tools
//! - GPU/CPU configuration with VRAM limits
//! - Automatic retry on failure
//! - Progress and timeout handling
//!
//! # Example
//!
//! ```rust,no_run
//! use superbook_pdf::{AiBridgeConfig, AiTool};
//! use superbook_pdf::ai_bridge::{AiBridge, HttpApiBridge};
//!
//! // Configure AI bridge
//! let config = AiBridgeConfig::builder()
//!     .gpu_enabled(true)
//!     .max_retries(3)
//!     .build();
//!
//! // Create HTTP bridge for RealESRGAN / YomiToku services
//! let bridge = HttpApiBridge::new(config).unwrap();
//! let _available = futures::executor::block_on(bridge.check_tool(AiTool::RealESRGAN));
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
//use std::sync::Arc;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio;

// ============================================================
// Constants
// ============================================================

/// Default timeout for AI processing (1 hour)
const DEFAULT_TIMEOUT_SECS: u64 = 3600;

/// Low VRAM limit (2GB)
const LOW_VRAM_MB: u64 = 2048;

/// Low VRAM tile size
const LOW_VRAM_TILE_SIZE: u32 = 128;

/// Default tile size for GPU processing
const DEFAULT_GPU_TILE_SIZE: u32 = 400;

/// AI Bridge error types
#[derive(Debug, Error)]
pub enum AiBridgeError {
    #[error("Tool not installed: {0:?}")]
    ToolNotInstalled(AiTool),

    #[error("Process failed: {0}")]
    ProcessFailed(String),

    #[error("Process timed out after {0:?}")]
    Timeout(Duration),

    #[error("GPU not available")]
    GpuNotAvailable,

    #[error("Out of memory")]
    OutOfMemory,

    #[error("All retries exhausted")]
    RetriesExhausted,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, AiBridgeError>;

/// AI Bridge configuration
#[derive(Debug, Clone)]
pub struct AiBridgeConfig {
    /// GPU configuration
    pub gpu_config: GpuConfig,
    /// Timeout duration
    pub timeout: Duration,
    /// Retry configuration
    pub retry_config: RetryConfig,
    /// Log level
    pub log_level: LogLevel,
}

impl Default for AiBridgeConfig {
    fn default() -> Self {
        Self {
            gpu_config: GpuConfig::default(),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            retry_config: RetryConfig::default(),
            log_level: LogLevel::Info,
        }
    }
}

impl AiBridgeConfig {
    /// Create a new config builder
    #[must_use]
    pub fn builder() -> AiBridgeConfigBuilder {
        AiBridgeConfigBuilder::default()
    }

    /// Create config for CPU-only processing
    #[must_use]
    pub fn cpu_only() -> Self {
        Self {
            gpu_config: GpuConfig {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Create config for low VRAM systems
    #[must_use]
    pub fn low_vram() -> Self {
        Self {
            gpu_config: GpuConfig {
                enabled: true,
                max_vram_mb: Some(LOW_VRAM_MB),
                tile_size: Some(LOW_VRAM_TILE_SIZE),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

/// Builder for [`AiBridgeConfig`]
#[derive(Debug, Default)]
pub struct AiBridgeConfigBuilder {
    config: AiBridgeConfig,
}

impl AiBridgeConfigBuilder {
    /// Set GPU configuration
    #[must_use]
    pub fn gpu_config(mut self, config: GpuConfig) -> Self {
        self.config.gpu_config = config;
        self
    }

    /// Enable or disable GPU
    #[must_use]
    pub fn gpu_enabled(mut self, enabled: bool) -> Self {
        self.config.gpu_config.enabled = enabled;
        self
    }

    /// Set GPU device ID
    #[must_use]
    pub fn gpu_device(mut self, id: u32) -> Self {
        self.config.gpu_config.device_id = Some(id);
        self
    }

    /// Set timeout duration
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    /// Set retry configuration
    #[must_use]
    pub fn retry_config(mut self, config: RetryConfig) -> Self {
        self.config.retry_config = config;
        self
    }

    /// Set maximum retry count
    #[must_use]
    pub fn max_retries(mut self, count: u32) -> Self {
        self.config.retry_config.max_retries = count;
        self
    }

    /// Set log level
    #[must_use]
    pub fn log_level(mut self, level: LogLevel) -> Self {
        self.config.log_level = level;
        self
    }

    /// Build the configuration
    #[must_use]
    pub fn build(self) -> AiBridgeConfig {
        self.config
    }
}

/// GPU configuration
#[derive(Debug, Clone)]
pub struct GpuConfig {
    /// Enable GPU
    pub enabled: bool,
    /// GPU device ID (None for auto)
    pub device_id: Option<u32>,
    /// Maximum VRAM usage (MB)
    pub max_vram_mb: Option<u64>,
    /// Tile size for memory efficiency
    pub tile_size: Option<u32>,
}

impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            device_id: None,
            max_vram_mb: None,
            tile_size: Some(DEFAULT_GPU_TILE_SIZE),
        }
    }
}

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum retry count
    pub max_retries: u32,
    /// Retry interval
    pub retry_interval: Duration,
    /// Use exponential backoff
    pub exponential_backoff: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_interval: Duration::from_secs(5),
            exponential_backoff: true,
        }
    }
}

/// Log levels
#[derive(Debug, Clone, Copy, Default)]
pub enum LogLevel {
    #[default]
    Info,
    Debug,
    Warn,
    Error,
}

/// Process status
#[derive(Debug, Clone)]
pub enum ProcessStatus {
    /// Preparing
    Preparing,
    /// Running with progress
    Running { progress: f32 },
    /// Completed
    Completed { duration: Duration },
    /// Failed
    Failed { error: String, retries: u32 },
    /// Timed out
    TimedOut,
    /// Cancelled
    Cancelled,
}

/// AI task result
#[derive(Debug)]
pub struct AiTaskResult {
    /// Successfully processed files
    pub processed_files: Vec<PathBuf>,
    /// Skipped files
    pub skipped_files: Vec<(PathBuf, String)>,
    /// Failed files
    pub failed_files: Vec<(PathBuf, String)>,
    /// Total duration
    pub duration: Duration,
    /// GPU statistics
    pub gpu_stats: Option<GpuStats>,
}

/// GPU statistics
#[derive(Debug, Clone)]
pub struct GpuStats {
    /// Peak VRAM usage (MB)
    pub peak_vram_mb: u64,
    /// Average GPU utilization
    pub avg_utilization: f32,
}

/// AI tool types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiTool {
    RealESRGAN,
    YomiToku,
}

impl AiTool {
    /// Get the display name for user-facing messages
    #[must_use]
    pub fn display_name(&self) -> &str {
        match self {
            AiTool::RealESRGAN => "RealESRGAN",
            AiTool::YomiToku => "YomiToku",
        }
    }
}

impl std::fmt::Display for AiTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// AI Bridge trait
#[async_trait]
pub trait AiBridge: Send + Sync {
    // 🚀 Send + Sync を追加してスレッド間共有を可能に
    /// Initialize bridge
    fn new(config: AiBridgeConfig) -> Result<Self>
    where
        Self: Sized;

    /// Get the bridge configuration (🚀 yomitoku.rs 等からの参照に必須)
    fn config(&self) -> &AiBridgeConfig;

    /// Check if tool is available (非同期化)
    async fn check_tool(&self, tool: AiTool) -> Result<bool>;

    /// Check GPU status (非同期化)
    async fn check_gpu(&self) -> Result<GpuStats>;

    /// Execute task (非同期化)
    async fn execute(
        &self,
        tool: AiTool,
        input_files: &[PathBuf],
        output_dir: &Path,
        tool_options: &(dyn std::any::Any + Send + Sync),
    ) -> Result<AiTaskResult>;

    /// Cancel running process
    async fn cancel(&self) -> Result<()>;

    /// Clean up cached models and free GPU memory for a tool
    async fn call_cleanup(&self, tool: AiTool) -> Result<()>;
}

// --- HttpApiBridge の完全実装 ---
#[async_trait]
impl AiBridge for HttpApiBridge {
    /// トレイトの new メソッド。固有実装の new を呼び出す
    fn new(config: AiBridgeConfig) -> Result<Self> {
        Self::new(config)
    }

    fn config(&self) -> &AiBridgeConfig {
        &self.config
    }

    async fn check_tool(&self, tool: AiTool) -> Result<bool> {
        let url = self
            .service_urls
            .get(&tool)
            .ok_or_else(|| AiBridgeError::ProcessFailed("Service URL not configured".into()))?;

        // ハンドシェイク: 最大3回までリトライ [構築方針]
        for attempt in 1..=3 {
            match self
                .client
                .get(format!("{}/version", url))
                .timeout(Duration::from_secs(5))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(info) = resp.json::<serde_json::Value>().await {
                        // Torch バージョンを抽出 [構築方針 提案8]
                        let torch_version = info
                            .get("torch_version")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let cuda_available = info
                            .get("cuda_available")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let device = info
                            .get("device")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");

                        // バージョン互換性チェック [構築方針 提案8]
                        let torch_major_version = torch_version
                            .split('.')
                            .next()
                            .and_then(|v| v.parse::<u32>().ok())
                            .unwrap_or(0);

                        let version_ok = match tool {
                            AiTool::RealESRGAN => {
                                // 現行マイクロサービス構成では RealESRGAN は Torch 2.x + CUDA 11.8 を使用する。
                                // 旧来の Torch 1.x 前提警告は現状と合っておらず、運用判断を誤らせるため出さない。
                                if torch_major_version == 0 {
                                    eprintln!(
                                        "⚠️  WARNING: {} returned an unknown Torch version: {}",
                                        tool, torch_version
                                    );
                                }
                                true
                            }
                            AiTool::YomiToku => {
                                // YomiToku は Torch 2.x を期待 [構築方針]
                                if torch_major_version < 2 {
                                    eprintln!(
                                        "⚠️  WARNING: {} is running on Torch {}, expected Torch 2.x. \
                                         This may cause compatibility issues.",
                                        tool, torch_version
                                    );
                                    true // 警告だが続行
                                } else {
                                    true
                                }
                            }
                        };

                        if version_ok {
                            println!(
                                "  ✨ {} API: Torch={}, CUDA={}, Device={}",
                                tool,
                                torch_version,
                                if cuda_available { "✅" } else { "❌" },
                                device
                            );
                            return Ok(true);
                        }
                    }
                    return Ok(true);
                }
                Err(e) if attempt < 3 => {
                    // リトライ [構築方針]
                    eprintln!(
                        "⚠️  Attempt {}/3 failed to reach {}: {}. Retrying...",
                        attempt, tool, e
                    );
                    tokio::time::sleep(Duration::from_secs(attempt as u64)).await;
                    continue;
                }
                Err(e) => {
                    eprintln!("❌ Failed to reach {} API after 3 attempts: {}", tool, e);
                    return Ok(false);
                }
                Ok(resp) => {
                    eprintln!("❌ {} API returned error: {}", tool, resp.status());
                    return Ok(false);
                }
            }
        }

        Ok(false)
    }

    async fn check_gpu(&self) -> Result<GpuStats> {
        // モック実装。本来は各サービスから情報を取得する [4]
        Ok(GpuStats {
            peak_vram_mb: 0,
            avg_utilization: 0.0,
        })
    }

    async fn execute(
        &self,
        tool: AiTool,
        input_files: &[PathBuf],
        output_dir: &Path,
        tool_options: &(dyn std::any::Any + Send + Sync),
    ) -> Result<AiTaskResult> {
        let start_time = Instant::now();
        let url = self
            .service_urls
            .get(&tool)
            .ok_or_else(|| AiBridgeError::ProcessFailed("Service URL not configured".into()))?;
        std::fs::create_dir_all(output_dir)?;

        let mut processed = Vec::new();
        let mut failed = Vec::new();

        for input_file in input_files {
            match tool {
                AiTool::RealESRGAN => {
                    let opts = tool_options
                        .downcast_ref::<crate::realesrgan::RealEsrganOptions>()
                        .ok_or_else(|| {
                            AiBridgeError::ProcessFailed("Invalid options for RealESRGAN".into())
                        })?;

                    let output_path = output_dir.join(format!(
                        "{}{}x.png",
                        input_file.file_stem().unwrap().to_string_lossy(),
                        opts.scale
                    ));

                    let payload = UpscaleRequest {
                        input_path: input_file.to_string_lossy().into(),
                        output_path: output_path.to_string_lossy().into(),
                        scale: opts.scale,
                        tile: opts.tile_size,
                        model_name: opts.model.model_name().to_string(),
                        fp32: !opts.fp16,
                        gpu_id: opts.gpu_id.unwrap_or(0) as i32,
                    };

                    // リトライロジック付きでアップスケール実行
                    let mut last_error = String::new();
                    for attempt in 1..=self.config.retry_config.max_retries {
                        match self
                            .client
                            .post(format!("{}/upscale", url))
                            .json(&payload)
                            .send()
                            .await
                        {
                            Ok(resp) if resp.status().is_success() => {
                                processed.push(output_path.clone());
                                break;
                            }
                            Ok(resp) => {
                                last_error = format!("HTTP error: {}", resp.status());
                                if attempt < self.config.retry_config.max_retries {
                                    let backoff_secs =
                                        if self.config.retry_config.exponential_backoff {
                                            2u64.pow(attempt - 1)
                                        } else {
                                            self.config.retry_config.retry_interval.as_secs()
                                        };
                                    eprintln!(
                                        "⚠️  Upscale attempt {}/{} failed ({}). Retrying in {}s...",
                                        attempt,
                                        self.config.retry_config.max_retries,
                                        last_error,
                                        backoff_secs
                                    );
                                    tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                                }
                            }
                            Err(e) => {
                                last_error = e.to_string();
                                if attempt < self.config.retry_config.max_retries {
                                    let backoff_secs =
                                        if self.config.retry_config.exponential_backoff {
                                            2u64.pow(attempt - 1)
                                        } else {
                                            self.config.retry_config.retry_interval.as_secs()
                                        };
                                    eprintln!(
                                        "⚠️  Upscale attempt {}/{} failed ({}). Retrying in {}s...",
                                        attempt,
                                        self.config.retry_config.max_retries,
                                        last_error,
                                        backoff_secs
                                    );
                                    tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                                }
                            }
                        }
                    }

                    if !processed.contains(&output_path) {
                        failed.push((input_file.clone(), last_error));
                    }
                }
                AiTool::YomiToku => {
                    let opts = tool_options
                        .downcast_ref::<crate::yomitoku::YomiTokuOptions>()
                        .ok_or_else(|| {
                            AiBridgeError::ProcessFailed("Invalid options for YomiToku".into())
                        })?;

                    let payload = OcrRequest {
                        input_path: input_file.to_string_lossy().to_string(),
                        gpu_id: opts.gpu_id.unwrap_or(0) as i32,
                        confidence: opts.confidence_threshold,
                        format: "json".into(),
                    };

                    let output_json_path = {
                        let stem = input_file
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "ocr_result".to_string());
                        output_dir.join(format!("{}.json", stem))
                    };

                    // リトライロジック付きで OCR 実行
                    let mut last_error = String::new();
                    for attempt in 1..=self.config.retry_config.max_retries {
                        match self
                            .client
                            .post(format!("{}/ocr", url))
                            .json(&payload)
                            .send()
                            .await
                        {
                            Ok(resp) if resp.status().is_success() => match resp.text().await {
                                Ok(body) => {
                                    if let Err(e) = std::fs::write(&output_json_path, body) {
                                        last_error = format!("Failed to write OCR output: {}", e);
                                        if attempt < self.config.retry_config.max_retries {
                                            let backoff_secs = if self
                                                .config
                                                .retry_config
                                                .exponential_backoff
                                            {
                                                2u64.pow(attempt - 1)
                                            } else {
                                                self.config.retry_config.retry_interval.as_secs()
                                            };
                                            tokio::time::sleep(Duration::from_secs(backoff_secs))
                                                .await;
                                        }
                                    } else {
                                        processed.push(output_json_path.clone());
                                        break;
                                    }
                                }
                                Err(e) => {
                                    last_error = format!("Failed to read OCR response body: {}", e);
                                    if attempt < self.config.retry_config.max_retries {
                                        let backoff_secs =
                                            if self.config.retry_config.exponential_backoff {
                                                2u64.pow(attempt - 1)
                                            } else {
                                                self.config.retry_config.retry_interval.as_secs()
                                            };
                                        eprintln!(
                                            "⚠️  OCR attempt {}/{} failed ({}). Retrying in {}s...",
                                            attempt,
                                            self.config.retry_config.max_retries,
                                            last_error,
                                            backoff_secs
                                        );
                                        tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                                    }
                                }
                            },
                            Ok(resp) => {
                                last_error = format!("HTTP error: {}", resp.status());
                                if attempt < self.config.retry_config.max_retries {
                                    let backoff_secs =
                                        if self.config.retry_config.exponential_backoff {
                                            2u64.pow(attempt - 1)
                                        } else {
                                            self.config.retry_config.retry_interval.as_secs()
                                        };
                                    eprintln!(
                                        "⚠️  OCR attempt {}/{} failed ({}). Retrying in {}s...",
                                        attempt,
                                        self.config.retry_config.max_retries,
                                        last_error,
                                        backoff_secs
                                    );
                                    tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                                }
                            }
                            Err(e) => {
                                last_error = e.to_string();
                                if attempt < self.config.retry_config.max_retries {
                                    let backoff_secs =
                                        if self.config.retry_config.exponential_backoff {
                                            2u64.pow(attempt - 1)
                                        } else {
                                            self.config.retry_config.retry_interval.as_secs()
                                        };
                                    eprintln!(
                                        "⚠️  OCR attempt {}/{} failed ({}). Retrying in {}s...",
                                        attempt,
                                        self.config.retry_config.max_retries,
                                        last_error,
                                        backoff_secs
                                    );
                                    tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                                }
                            }
                        }
                    }

                    if !processed.contains(&output_json_path) {
                        failed.push((input_file.clone(), last_error));
                    }
                }
            }
        }

        Ok(AiTaskResult {
            processed_files: processed,
            skipped_files: vec![],
            failed_files: failed,
            duration: start_time.elapsed(),
            gpu_stats: None,
        })
    }

    async fn cancel(&self) -> Result<()> {
        Ok(())
    }

    async fn call_cleanup(&self, tool: AiTool) -> Result<()> {
        HttpApiBridge::call_cleanup(self, tool).await
    }
}

#[derive(Serialize, Deserialize)]
struct UpscaleRequest {
    input_path: String,
    output_path: String,
    scale: u32,
    tile: u32,
    model_name: String,
    fp32: bool,
    gpu_id: i32,
}

#[derive(Serialize, Deserialize)]
struct OcrRequest {
    input_path: String,
    gpu_id: i32,
    confidence: f32,
    format: String,
}
/// HTTP APIベースのブリッジ実装 (マイクロサービス構成用)
pub struct HttpApiBridge {
    config: AiBridgeConfig,
    client: Client,
    service_urls: HashMap<AiTool, String>,
}

/// 🚀 Error E0599 対策: HttpApiBridge 自身の impl ブロックを作成
impl HttpApiBridge {
    /// 新しい HttpApiBridge を生成
    /// 🚀 Error E0107 対策: 戻り値を Result<Self> に修正
    pub fn new(config: AiBridgeConfig) -> Result<Self> {
        let mut service_urls = HashMap::new();

        // docker-compose.yml 等の環境変数からURLを取得
        service_urls.insert(
            AiTool::RealESRGAN,
            std::env::var("REALESRGAN_API_URL")
                .unwrap_or_else(|_| "http://realesrgan-api:8000".into()),
        );
        service_urls.insert(
            AiTool::YomiToku,
            std::env::var("YOMITOKU_API_URL").unwrap_or_else(|_| "http://yomitoku-api:8000".into()),
        );

        Ok(Self {
            config,
            client: Client::builder()
                // 接続プール設定：156ページテストで安定動作させるため
                .pool_max_idle_per_host(16) // ホストあたりのアイドル接続数を増加
                // 個別タイムアウト設定
                .connect_timeout(Duration::from_secs(15)) // 接続確立: 15秒
                .timeout(Duration::from_secs(900)) // 全体: 15分（N=5並行 × ~120s + 余裕）
                .build()
                .map_err(|e| AiBridgeError::ProcessFailed(e.to_string()))?,
            service_urls,
        })
    }

    /// Call cleanup endpoint to free GPU memory after processing
    pub async fn call_cleanup(&self, tool: AiTool) -> Result<()> {
        let url = self
            .service_urls
            .get(&tool)
            .ok_or_else(|| AiBridgeError::ProcessFailed("Service URL not configured".into()))?;

        match self
            .client
            .post(format!("{}/cleanup", url))
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                if resp.status() == reqwest::StatusCode::ACCEPTED {
                    eprintln!("ℹ️  {} cleanup deferred (service busy)", tool);
                } else {
                    eprintln!("✅ {} cache and GPU memory freed", tool);
                }
                Ok(())
            }
            Ok(resp) => {
                eprintln!("⚠️  {} cleanup returned error: {}", tool, resp.status());
                Ok(()) // Non-fatal: continue even if cleanup fails
            }
            Err(e) => {
                eprintln!("⚠️  {} cleanup request failed: {}", tool, e);
                Ok(()) // Non-fatal: continue even if cleanup fails
            }
        }
    }

    /// ハンドシェイク：起動時に全AIサービスの互換性を確認 [構築方針]
    pub async fn handshake(&self) -> Result<()> {
        println!("🔗 Starting AI Service Handshake...");
        println!();

        let mut all_ok = true;

        // RealESRGAN チェック
        println!("Checking RealESRGAN (Torch 1.x / CUDA 11.8)...");
        match self.check_tool(AiTool::RealESRGAN).await {
            Ok(true) => {
                println!("  ✅ RealESRGAN OK");
            }
            _ => {
                eprintln!("  ❌ RealESRGAN NOT READY");
                all_ok = false;
            }
        }
        println!();

        // YomiToku チェック
        println!("Checking YomiToku (Torch 2.x / CUDA 12.1)...");
        match self.check_tool(AiTool::YomiToku).await {
            Ok(true) => {
                println!("  ✅ YomiToku OK");
            }
            _ => {
                eprintln!("  ❌ YomiToku NOT READY");
                all_ok = false;
            }
        }
        println!();

        if all_ok {
            println!("✅ All AI Services Ready!");
            Ok(())
        } else {
            eprintln!("⚠️  Some AI Services are not ready. Proceeding anyway...");
            // 警告だが、部分的に利用可能な状況で続行することを許可
            Ok(())
        }
    }
}
