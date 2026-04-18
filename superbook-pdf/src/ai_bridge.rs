//! AI Tools Bridge module
//!
//! Provides communication with external AI tools (Python: `RealESRGAN`, `YomiToku`, etc.)
//!
//! # Features
//!
//! - Subprocess management for Python AI tools
//! - GPU/CPU configuration with VRAM limits
//! - Automatic retry on failure
//! - Progress and timeout handling
//!
//! # Example
//!
//! ```rust,no_run
//! use superbook_pdf::{AiBridgeConfig, SubprocessBridge, AiTool};
//!
//! // Configure AI bridge
//! let config = AiBridgeConfig::builder()
//!     .gpu_enabled(true)
//!     .max_retries(3)
//!     .build();
//!
//! // Create bridge for RealESRGAN
//! // let bridge = SubprocessBridge::new(AiTool::RealEsrgan, &config);
//! ```

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use std::collections::HashMap;
//use std::sync::Arc;
use thiserror::Error;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use async_trait::async_trait;
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
    #[error("Python virtual environment not found: {0}")]
    VenvNotFound(PathBuf),

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
    /// Python virtual environment path
    pub venv_path: PathBuf,
    /// GPU configuration
    pub gpu_config: GpuConfig,
    /// Timeout duration
    pub timeout: Duration,
    /// Retry configuration
    pub retry_config: RetryConfig,
    /// Log level
    pub log_level: LogLevel,
    /// Directory containing bridge scripts (None = auto-detect)
    pub bridge_scripts_dir: Option<PathBuf>,
}

//impl Default for AiBridgeConfig {
//    fn default() -> Self {
//        Self {
//            venv_path: PathBuf::from("./ai_venv"),
//            gpu_config: GpuConfig::default(),
//            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
//            retry_config: RetryConfig::default(),
//            log_level: LogLevel::Info,
//            bridge_scripts_dir: None,
//        }
//    }
//}

impl Default for AiBridgeConfig {
    fn default() -> Self {
        // 環境変数 SUPERBOOK_VENV があればそれを使用し、なければデフォルトの ./ai_venv を使う
        let venv_path = std::env::var("SUPERBOOK_VENV")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./ai_venv"));

        // 環境変数 SUPERBOOK_BRIDGE_SCRIPTS_DIR からパスを取得
        let bridge_scripts_dir = std::env::var("SUPERBOOK_BRIDGE_SCRIPTS_DIR")
            .map(PathBuf::from)
            .ok();

        Self {
            venv_path,
            gpu_config: GpuConfig::default(),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            retry_config: RetryConfig::default(),
            log_level: LogLevel::Info,
            bridge_scripts_dir,
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
    /// Set Python virtual environment path
    #[must_use]
    pub fn venv_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.venv_path = path.into();
        self
    }

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

    /// Set bridge scripts directory
    #[must_use]
    pub fn bridge_scripts_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.config.bridge_scripts_dir = Some(dir.into());
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
    /// Get the module name for Python
    #[must_use]
    pub fn module_name(&self) -> &str {
        match self {
            AiTool::RealESRGAN => "realesrgan",
            AiTool::YomiToku => "yomitoku",
        }
    }

    /// Get the bridge script filename
    #[must_use]
    pub fn bridge_script_name(&self) -> &str {
        match self {
            AiTool::RealESRGAN => "realesrgan_bridge.py",
            AiTool::YomiToku => "yomitoku_bridge.py",
        }
    }

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

/// Resolve the path to a bridge script for the given AI tool.
///
/// Search order:
/// 1. `config.bridge_scripts_dir` (if specified)
/// 2. `config.venv_path.parent()` (venv parent directory)
/// 3. Current executable's directory / `ai_bridge/`
/// 4. `./ai_bridge/` (current working directory)
///
/// Returns the path if found, or an error with all searched paths.
pub fn resolve_bridge_script(tool: AiTool, config: &AiBridgeConfig) -> Result<PathBuf> {
    let script_name = tool.bridge_script_name();
    let mut searched_paths = Vec::new();

    // --- 追加箇所: 0. 環境変数を最優先でチェック ---
    if let Ok(env_dir) = std::env::var("SUPERBOOK_BRIDGE_SCRIPTS_DIR") {
        let path = PathBuf::from(env_dir).join(script_name);
        if path.exists() {
            return Ok(path);
        }
        searched_paths.push(path);
    }

    // 1. Check explicit bridge_scripts_dir
    if let Some(ref dir) = config.bridge_scripts_dir {
        let path = dir.join(script_name);
        if path.exists() {
            return Ok(path);
        }
        searched_paths.push(path);
    }

    // 2. Check venv parent directory
    if let Some(parent) = config.venv_path.parent() {
        let path = parent.join(script_name);
        if path.exists() {
            return Ok(path);
        }
        searched_paths.push(path);
    }

    // 3. Check executable's directory / ai_bridge/
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let path = exe_dir.join("ai_bridge").join(script_name);
            if path.exists() {
                return Ok(path);
            }
            searched_paths.push(path);
        }
    }

    // 4. Check ./ai_bridge/ (CWD)
    let cwd_path = PathBuf::from("ai_bridge").join(script_name);
    if cwd_path.exists() {
        return Ok(cwd_path);
    }
    searched_paths.push(cwd_path);

    // Not found anywhere
    let paths_str: Vec<String> = searched_paths
        .iter()
        .map(|p| p.display().to_string())
        .collect();
    Err(AiBridgeError::ProcessFailed(format!(
        "Bridge script '{}' for {} not found. Searched paths:\n  - {}",
        script_name,
        tool.display_name(),
        paths_str.join("\n  - ")
    )))
}

/// AI Bridge trait
//pub trait AiBridge {
//    /// Initialize bridge
//    fn new(config: AiBridgeConfig) -> Result<Self>
//    where
//        Self: Sized;
//
//    /// Check if tool is available
//    fn check_tool(&self, tool: AiTool) -> Result<bool>;
//
//    /// Check GPU status
//    fn check_gpu(&self) -> Result<GpuStats>;
//
//    /// Execute task (sync)
//    fn execute(
//        &self,
//        tool: AiTool,
//        input_files: &[PathBuf],
//        output_dir: &Path,
//        tool_options: &dyn std::any::Any,
//    ) -> Result<AiTaskResult>;
//
//    /// Cancel running process
//    fn cancel(&self) -> Result<()>;
//}
#[async_trait]
pub trait AiBridge: Send + Sync { // 🚀 Send + Sync を追加してスレッド間共有を可能に
    /// Initialize bridge
    fn new(config: AiBridgeConfig) -> Result<Self> where Self: Sized;
    
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
}

/// Subprocess-based bridge implementation
pub struct SubprocessBridge {
    config: AiBridgeConfig,
}

impl SubprocessBridge {
    // Create a new subprocess bridge
    ///
    /// Validates that the venv exists and warns if bridge scripts cannot be found.
    #[allow(clippy::redundant_clone)] // Clone needed due to partial move restrictions
    pub fn new(config: AiBridgeConfig) -> Result<Self> {
        // Check if venv exists
        if !config.venv_path.exists() {
            return Err(AiBridgeError::VenvNotFound(config.venv_path.clone()));
        }

        // Pre-check bridge script availability and warn if missing
        for tool in &[AiTool::RealESRGAN, AiTool::YomiToku] {
            if resolve_bridge_script(*tool, &config).is_err() {
                eprintln!(
                    "Warning: Bridge script for {} not found. \
                     {} will not work until '{}' is placed in the ai_bridge/ directory \
                     or SUPERBOOK_BRIDGE_SCRIPTS_DIR is set.",
                    tool.display_name(),
                    tool.display_name(),
                    tool.bridge_script_name(),
                );
            }
        }

        Ok(Self { config })
    }

    /// Get the configuration
    pub fn config(&self) -> &AiBridgeConfig {
        &self.config
    }

    /// Get Python executable path
    fn get_python_path(&self) -> PathBuf {
        if cfg!(windows) {
            self.config.venv_path.join("Scripts").join("python.exe")
        } else {
            self.config.venv_path.join("bin").join("python")
        }
    }

    /// Check if a tool is available
    //pub fn check_tool(&self, tool: AiTool) -> Result<bool> {
    //    let python = self.get_python_path();

    //    if !python.exists() {
    //        return Ok(false);
    //    }

    //    let output = Command::new(&python)
    //        .arg("-c")
    //        .arg(format!("import {}", tool.module_name()))
    //        .output();

    //    match output {
    //        Ok(o) => Ok(o.status.success()),
    //        Err(_) => Ok(false),
    //    }
    //}

    //// Check GPU status
    //pub fn check_gpu(&self) -> Result<GpuStats> {
    //    let output = Command::new("nvidia-smi")
    //        .args(["--query-gpu=memory.used", "--format=csv,noheader,nounits"])
    //        .output()
    //        .map_err(|_| AiBridgeError::GpuNotAvailable)?;

    //    if !output.status.success() {
    //        return Err(AiBridgeError::GpuNotAvailable);
    //    }

    //    let vram_str = String::from_utf8_lossy(&output.stdout);
    //    let vram_mb: u64 = vram_str.trim().parse().unwrap_or(0);

    //    Ok(GpuStats {
    //        peak_vram_mb: vram_mb,
    //        avg_utilization: 0.0,
    //    })
    //}

    //// Execute AI tool
    ///
    /// # Arguments
    /// * `tool` - The AI tool to execute
    /// * `input_files` - Input file paths
    /// * `output_dir` - Output directory for results
    /// * `tool_options` - Tool-specific options (can be downcast to RealEsrganOptions, etc.)
    pub fn execute(
        &self,
        tool: AiTool,
        input_files: &[PathBuf],
        output_dir: &Path,
        tool_options: &dyn std::any::Any,
    ) -> Result<AiTaskResult> {
        let start_time = std::time::Instant::now();
        let python = self.get_python_path();

        // Resolve bridge script path using multi-path fallback
        let bridge_script = resolve_bridge_script(tool, &self.config)?;

        let mut processed = Vec::new();
        let mut failed = Vec::new();

        for input_file in input_files {
            let mut last_error = None;

            // Generate output filename based on input
            let output_filename = format!(
                "{}{}x.png",
                input_file.file_stem().unwrap_or_default().to_string_lossy(),
                "2", // デフォルトは2倍で、必要に応じて tool_options から scale を取得して置き換えることもできます
                // input_file.extension().unwrap_or_default().to_string_lossy() は不要なので削除
            );
            let output_path = output_dir.join(&output_filename);

            for retry in 0..=self.config.retry_config.max_retries {
                let mut cmd = Command::new(&python);
                cmd.arg(&bridge_script);

                match tool {
                    AiTool::RealESRGAN => {
                        cmd.arg("-i").arg(input_file);
                        cmd.arg("-o").arg(&output_path);

                        // Extract options from tool_options if available
                        if let Some(opts) = tool_options.downcast_ref::<crate::RealEsrganOptions>() {
                            cmd.arg("-s").arg(opts.scale.to_string());
                            cmd.arg("-t").arg(opts.tile_size.to_string());
                            if let Some(gpu_id) = opts.gpu_id {
                                cmd.arg("-g").arg(gpu_id.to_string());
                            }
                            if !opts.fp16 {
                                cmd.arg("--fp32");
                            }
                        } else if let Some(tile) = self.config.gpu_config.tile_size {
                            cmd.arg("-t").arg(tile.to_string());
                        }

                        cmd.arg("--json");
                    }
                    AiTool::YomiToku => {
                        cmd.arg(input_file);
                        cmd.arg("--output").arg(output_dir);
                        cmd.arg("--json");
                    }
                }

                cmd.stdout(Stdio::piped());
                cmd.stderr(Stdio::piped());

                match cmd.output() {
                    Ok(output) if output.status.success() => {
                        processed.push(input_file.clone());
                        last_error = None;
                        break;
                    }
                    Ok(output) => {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        last_error = Some(format!("stderr: {}, stdout: {}", stderr, stdout));

                        if stderr.contains("out of memory") || stderr.contains("CUDA error") {
                            return Err(AiBridgeError::OutOfMemory);
                        }
                    }
                    Err(e) => {
                        last_error = Some(e.to_string());
                    }
                }

                // Wait before retry
                if retry < self.config.retry_config.max_retries {
                    let wait_time = if self.config.retry_config.exponential_backoff {
                        self.config.retry_config.retry_interval * 2_u32.pow(retry)
                    } else {
                        self.config.retry_config.retry_interval
                    };
                    std::thread::sleep(wait_time);
                }
            }

            if let Some(error) = last_error {
                failed.push((input_file.clone(), error));
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

    pub fn cancel(&self) -> Result<()> {
        // Placeholder cancel support for subprocess bridge.
        // A real implementation would track running child processes and kill them.
        Ok(())
    }

    //// Execute a raw command with timeout
    ///
    /// This is a lower-level method for executing custom Python scripts
    /// with arbitrary arguments and a configurable timeout.
    pub fn execute_with_timeout(&self, args: &[String], timeout: Duration) -> Result<String> {
        let python = self.get_python_path();

        let mut cmd = Command::new(&python);
        cmd.args(args);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let child = cmd
            .spawn()
            .map_err(|e| AiBridgeError::ProcessFailed(format!("Failed to spawn process: {}", e)))?;

        // Wait for completion and check timeout
        let start = std::time::Instant::now();
        let output = child
            .wait_with_output()
            .map_err(|e| AiBridgeError::ProcessFailed(format!("Process error: {}", e)))?;

        if start.elapsed() > timeout {
            return Err(AiBridgeError::Timeout(timeout));
        }

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("out of memory") || stderr.contains("CUDA error") {
                return Err(AiBridgeError::OutOfMemory);
            }
            return Err(AiBridgeError::ProcessFailed(format!(
                "Process exited with status {}: {}",
                output.status, stderr
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    //// Cancel running process (placeholder)
    //pub fn cancel(&self) -> Result<()> {
    //    // In a full implementation, this would track and kill running processes
    //    Ok(())
    //}
}

#[async_trait]
impl AiBridge for SubprocessBridge {
    // トレイト版の new
    fn new(config: AiBridgeConfig) -> Result<Self> {
        SubprocessBridge::new(config)
    }

    // 🚀 追加
    fn config(&self) -> &AiBridgeConfig {
        &self.config
    }

    // 非同期化された check_tool
    async fn check_tool(&self, tool: AiTool) -> Result<bool> {
        // すでに impl SubprocessBridge にある同名メソッドを呼び出す
        // self.check_tool(tool) や SubprocessBridge::check_tool(self, tool) だと
        // トレイトの非同期メソッドを呼んでしまい無限ループ＆型エラーになります。
        // 固有の同期版ロジックをここで直接実行するのが最も安全です。
        let python = self.get_python_path();
        if !python.exists() {
            return Ok(false);
        }
        let output = Command::new(&python)
            .arg("-c")
            .arg(format!("import {}", tool.module_name()))
            .output();
    
        match output {
            Ok(o) => Ok(o.status.success()),
            _ => Ok(false),
        }
    }

    // 非同期化された check_gpu
    async fn check_gpu(&self) -> Result<GpuStats> {
        // すでに impl SubprocessBridge にある同名メソッドを呼び出す
        let output = Command::new("nvidia-smi")
            .args(["--query-gpu=memory.used", "--format=csv,noheader,nounits"])
            .output()
            .map_err(|_| AiBridgeError::GpuNotAvailable)?;

        if !output.status.success() {
            return Err(AiBridgeError::GpuNotAvailable);
        }

        let vram_str = String::from_utf8_lossy(&output.stdout);
        let vram_mb: u64 = vram_str.trim().parse().unwrap_or(0);
        
        Ok(GpuStats {
            peak_vram_mb: vram_mb,
            avg_utilization: 0.0,
        })
    }

    // 非同期化された execute
    // For subprocess bridge, we execute synchronously
    async fn execute(
        &self,
        tool: AiTool,
        input_files: &[PathBuf],
        output_dir: &Path,
        tool_options: &(dyn std::any::Any + Send + Sync),
    ) -> Result<AiTaskResult> {
        // Call the synchronous version inline
        // This is acceptable because subprocess execution is already blocking anyway
        let start_time = std::time::Instant::now();
        let python = self.get_python_path();

        // Resolve bridge script path using multi-path fallback
        let bridge_script = resolve_bridge_script(tool, &self.config)?;

        let mut processed = Vec::new();
        let mut failed = Vec::new();

        for input_file in input_files {
            let mut last_error = None;

            // Generate output filename based on input
            let output_filename = format!(
                "{}_upscaled.{}",
                input_file.file_stem().unwrap_or_default().to_string_lossy(),
                input_file.extension().unwrap_or_default().to_string_lossy()
            );
            let output_path = output_dir.join(&output_filename);

            for retry in 0..=self.config.retry_config.max_retries {
                let mut cmd = Command::new(&python);
                cmd.arg(&bridge_script);

                match tool {
                    AiTool::RealESRGAN => {
                        cmd.arg("-i").arg(input_file);
                        cmd.arg("-o").arg(&output_path);

                        // Extract options from tool_options if available
                        if let Some(opts) = tool_options.downcast_ref::<crate::RealEsrganOptions>() {
                            cmd.arg("-s").arg(opts.scale.to_string());
                            cmd.arg("-t").arg(opts.tile_size.to_string());
                            if let Some(gpu_id) = opts.gpu_id {
                                cmd.arg("-g").arg(gpu_id.to_string());
                            }
                            if !opts.fp16 {
                                cmd.arg("--fp32");
                            }
                        } else if let Some(tile) = self.config.gpu_config.tile_size {
                            cmd.arg("-t").arg(tile.to_string());
                        }

                        cmd.arg("--json");
                    }
                    AiTool::YomiToku => {
                        cmd.arg(input_file);
                        cmd.arg("--output").arg(output_dir);
                        cmd.arg("--json");
                    }
                }

                cmd.stdout(Stdio::piped());
                cmd.stderr(Stdio::piped());

                match cmd.output() {
                    Ok(output) if output.status.success() => {
                        processed.push(input_file.clone());
                        last_error = None;
                        break;
                    }
                    Ok(output) => {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        last_error = Some(format!("stderr: {}, stdout: {}", stderr, stdout));

                        if stderr.contains("out of memory") || stderr.contains("CUDA error") {
                            return Err(AiBridgeError::OutOfMemory);
                        }
                    }
                    Err(e) => {
                        last_error = Some(e.to_string());
                    }
                }

                // Wait before retry
                if retry < self.config.retry_config.max_retries {
                    let wait_time = if self.config.retry_config.exponential_backoff {
                        self.config.retry_config.retry_interval * 2_u32.pow(retry)
                    } else {
                        self.config.retry_config.retry_interval
                    };
                    std::thread::sleep(wait_time);
                }
            }

            if let Some(error) = last_error {
                failed.push((input_file.clone(), error));
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

    // 非同期化された cancel
    async fn cancel(&self) -> Result<()> {
        SubprocessBridge::cancel(self)
    }
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
        let url = self.service_urls.get(&tool)
            .ok_or_else(|| AiBridgeError::ProcessFailed("Service URL not configured".into()))?;
        
        // ハンドシェイク: 最大3回までリトライ [構築方針]
        for attempt in 1..=3 {
            match self.client
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
                                // RealESRGAN は Torch 1.x を期待 [構築方針]
                                if torch_major_version >= 2 {
                                    eprintln!(
                                        "⚠️  WARNING: {} is running on Torch {}, expected Torch 1.x. \
                                         This may cause compatibility issues.",
                                        tool, torch_version
                                    );
                                    true // 警告だが続行
                                } else {
                                    true
                                }
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
                    eprintln!(
                        "❌ {} API returned error: {}",
                        tool,
                        resp.status()
                    );
                    return Ok(false);
                }
            }
        }

        Ok(false)
    }

    async fn check_gpu(&self) -> Result<GpuStats> {
        // モック実装。本来は各サービスから情報を取得する [4]
        Ok(GpuStats { peak_vram_mb: 0, avg_utilization: 0.0 })
    }

    async fn execute(
        &self,
        tool: AiTool,
        input_files: &[PathBuf],
        output_dir: &Path,
        tool_options: &(dyn std::any::Any + Send + Sync),
    ) -> Result<AiTaskResult> {
        let start_time = Instant::now();
        let url = self.service_urls.get(&tool)
            .ok_or_else(|| AiBridgeError::ProcessFailed("Service URL not configured".into()))?;
        
        let mut processed = Vec::new();
        let mut failed = Vec::new();

        for input_file in input_files {
            let res = match tool {
                AiTool::RealESRGAN => {
                    let opts = tool_options.downcast_ref::<crate::realesrgan::RealEsrganOptions>()
                        .ok_or_else(|| AiBridgeError::ProcessFailed("Invalid options for RealESRGAN".into()))?;
                    
                    let payload = UpscaleRequest {
                        input_path: input_file.to_string_lossy().into(),
                        // 🚀 修正: ファイル名形式を "{元ファイル名}{倍率}x.png" に変更します
                        output_path: output_dir.join(format!(
                            "{}{}x.png",            
                            input_file.file_stem().unwrap().to_string_lossy(),
                            opts.scale // 🚀 scale (2 または 4) をファイル名に含めます
                        )).to_string_lossy().into(),
                        scale: opts.scale,
                        tile: opts.tile_size,
                        model_name: opts.model.model_name().to_string(),
                        fp32: !opts.fp16,
                        gpu_id: opts.gpu_id.unwrap_or(0) as i32,
                    };
                    self.client.post(format!("{}/upscale", url)).json(&payload).send().await
                },
                AiTool::YomiToku => {
                    let opts = tool_options.downcast_ref::<crate::yomitoku::YomiTokuOptions>()
                        .ok_or_else(|| AiBridgeError::ProcessFailed("Invalid options for YomiToku".into()))?;
                    
                    let payload = OcrRequest {
                        input_path: input_file.to_string_lossy().to_string(),
                        gpu_id: opts.gpu_id.unwrap_or(0) as i32,
                        confidence: opts.confidence_threshold,
                        format: "json".into(),
                    };
                    self.client.post(format!("{}/ocr", url)).json(&payload).send().await
                }
            };

            match res {
                Ok(resp) if resp.status().is_success() => processed.push(input_file.clone()),
                Ok(resp) => failed.push((input_file.clone(), format!("HTTP error: {}", resp.status()))),
                Err(e) => failed.push((input_file.clone(), e.to_string())),
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
            std::env::var("REALESRGAN_API_URL").unwrap_or_else(|_| "http://realesrgan-api:8000".into())
        );
        service_urls.insert(
            AiTool::YomiToku, 
            std::env::var("YOMITOKU_API_URL").unwrap_or_else(|_| "http://yomitoku-api:8000".into())
        );

        Ok(Self {
            config,
            client: Client::builder()
                .timeout(Duration::from_secs(3600)) // 1時間のタイムアウト
                .build()
                .map_err(|e| AiBridgeError::ProcessFailed(e.to_string()))?,
            service_urls,
        })
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
