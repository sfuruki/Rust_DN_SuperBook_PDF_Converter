# 08-ai-bridge.spec.md - AI Tools Bridge Specification

## Overview

外部AIツール（Python: RealESRGAN, YomiToku等）との通信を管理するモジュール。
subprocess/PyO3を使用してPythonプロセスを制御。

---

## Responsibilities

1. Python仮想環境の管理
2. AIツールプロセスの起動・監視
3. GPUリソースの管理
4. エラーハンドリングとリトライ
5. 進捗の取得とレポート

---

## Data Structures

```rust
use std::path::{Path, PathBuf};
use std::time::Duration;

/// AIブリッジ設定
#[derive(Debug, Clone)]
pub struct AiBridgeConfig {
    /// Python仮想環境パス
    pub venv_path: PathBuf,
    /// GPU使用設定
    pub gpu_config: GpuConfig,
    /// タイムアウト設定
    pub timeout: Duration,
    /// リトライ設定
    pub retry_config: RetryConfig,
    /// ログレベル
    pub log_level: LogLevel,
}

#[derive(Debug, Clone)]
pub struct GpuConfig {
    /// GPU使用を有効化
    pub enabled: bool,
    /// 使用するGPU ID（Noneで自動選択）
    pub device_id: Option<u32>,
    /// 最大VRAM使用量（MB）
    pub max_vram_mb: Option<u64>,
    /// タイルサイズ（メモリ節約用）
    pub tile_size: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// 最大リトライ回数
    pub max_retries: u32,
    /// リトライ間隔
    pub retry_interval: Duration,
    /// 指数バックオフを使用
    pub exponential_backoff: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum LogLevel {
    #[default]
    Info,
    Debug,
    Warn,
    Error,
}

/// プロセス状態
#[derive(Debug, Clone)]
pub enum ProcessStatus {
    /// 準備中
    Preparing,
    /// 実行中
    Running { progress: f32 },
    /// 完了
    Completed { duration: Duration },
    /// エラー
    Failed { error: String, retries: u32 },
    /// タイムアウト
    TimedOut,
    /// キャンセル
    Cancelled,
}

/// AIタスク結果
#[derive(Debug)]
pub struct AiTaskResult {
    /// 処理成功したファイル
    pub processed_files: Vec<PathBuf>,
    /// スキップされたファイル
    pub skipped_files: Vec<(PathBuf, String)>,
    /// エラーが発生したファイル
    pub failed_files: Vec<(PathBuf, String)>,
    /// 実行時間
    pub duration: Duration,
    /// GPU使用統計
    pub gpu_stats: Option<GpuStats>,
}

#[derive(Debug, Clone)]
pub struct GpuStats {
    /// ピークVRAM使用量（MB）
    pub peak_vram_mb: u64,
    /// 平均GPU使用率
    pub avg_utilization: f32,
}

/// AIツールの種類
#[derive(Debug, Clone, Copy)]
pub enum AiTool {
    RealESRGAN,
    YomiToku,
}

/// エラー型
#[derive(Debug, thiserror::Error)]
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
```

---

## Public API

```rust
/// AiBridge トレイト
pub trait AiBridge {
    /// ブリッジを初期化
    fn new(config: AiBridgeConfig) -> Result<Self> where Self: Sized;

    /// ツールが利用可能か確認
    fn check_tool(&self, tool: AiTool) -> Result<bool>;

    /// GPU状態を確認
    fn check_gpu(&self) -> Result<GpuStats>;

    /// タスクを実行（同期）
    fn execute(
        &self,
        tool: AiTool,
        input_files: &[PathBuf],
        output_dir: &Path,
        tool_options: &dyn std::any::Any,
    ) -> Result<AiTaskResult>;

    /// タスクを実行（非同期、進捗コールバック付き）
    fn execute_async(
        &self,
        tool: AiTool,
        input_files: &[PathBuf],
        output_dir: &Path,
        tool_options: &dyn std::any::Any,
        progress_callback: Box<dyn Fn(ProcessStatus) + Send>,
    ) -> Result<AiTaskResult>;

    /// 実行中のプロセスをキャンセル
    fn cancel(&self) -> Result<()>;
}

/// subprocess実装
pub struct SubprocessBridge {
    config: AiBridgeConfig,
    // 内部状態
}

impl Default for AiBridgeConfig {
    fn default() -> Self {
        Self {
            venv_path: PathBuf::from("./ai_venv"),
            gpu_config: GpuConfig {
                enabled: true,
                device_id: None,
                max_vram_mb: None,
                tile_size: Some(400),
            },
            timeout: Duration::from_secs(3600), // 1時間
            retry_config: RetryConfig {
                max_retries: 3,
                retry_interval: Duration::from_secs(5),
                exponential_backoff: true,
            },
            log_level: LogLevel::Info,
        }
    }
}
```

---

## Test Cases

### TC-AIB-001: ブリッジ初期化

```rust
#[test]
fn test_bridge_initialization() {
    let config = AiBridgeConfig {
        venv_path: PathBuf::from("tests/fixtures/test_venv"),
        ..Default::default()
    };

    let bridge = SubprocessBridge::new(config).unwrap();
    assert!(bridge.check_tool(AiTool::RealESRGAN).is_ok());
}
```

### TC-AIB-002: 仮想環境なしエラー

```rust
#[test]
fn test_missing_venv_error() {
    let config = AiBridgeConfig {
        venv_path: PathBuf::from("/nonexistent/venv"),
        ..Default::default()
    };

    let result = SubprocessBridge::new(config);
    assert!(matches!(result, Err(AiBridgeError::VenvNotFound(_))));
}
```

### TC-AIB-003: ツール確認

```rust
#[test]
fn test_check_tool() {
    let bridge = create_test_bridge();

    let realesrgan_available = bridge.check_tool(AiTool::RealESRGAN).unwrap();
    let yomitoku_available = bridge.check_tool(AiTool::YomiToku).unwrap();

    // テスト環境ではツールがインストールされている前提
    assert!(realesrgan_available);
    assert!(yomitoku_available);
}
```

### TC-AIB-004: GPU状態確認

```rust
#[test]
fn test_check_gpu() {
    let bridge = create_test_bridge();

    let gpu_stats = bridge.check_gpu();

    if let Ok(stats) = gpu_stats {
        assert!(stats.peak_vram_mb > 0);
    } else {
        // GPUなし環境でも正常にエラーを返す
        assert!(matches!(gpu_stats, Err(AiBridgeError::GpuNotAvailable)));
    }
}
```

### TC-AIB-005: タスク実行（成功）

```rust
#[test]
fn test_execute_success() {
    let bridge = create_test_bridge();
    let temp_dir = tempfile::tempdir().unwrap();

    let input_files = vec![PathBuf::from("tests/fixtures/test_image.png")];
    let options = RealEsrganOptions::default();

    let result = bridge.execute(
        AiTool::RealESRGAN,
        &input_files,
        temp_dir.path(),
        &options,
    ).unwrap();

    assert_eq!(result.processed_files.len(), 1);
    assert!(result.failed_files.is_empty());
}
```

### TC-AIB-006: タスク実行（バッチ）

```rust
#[test]
fn test_execute_batch() {
    let bridge = create_test_bridge();
    let temp_dir = tempfile::tempdir().unwrap();

    let input_files: Vec<_> = (1..=5)
        .map(|i| PathBuf::from(format!("tests/fixtures/image_{}.png", i)))
        .collect();

    let options = RealEsrganOptions::default();

    let result = bridge.execute(
        AiTool::RealESRGAN,
        &input_files,
        temp_dir.path(),
        &options,
    ).unwrap();

    assert_eq!(result.processed_files.len(), 5);
}
```

### TC-AIB-007: タイムアウト

```rust
#[test]
fn test_timeout() {
    let bridge = SubprocessBridge::new(AiBridgeConfig {
        timeout: Duration::from_millis(1), // 即タイムアウト
        ..Default::default()
    }).unwrap();

    let temp_dir = tempfile::tempdir().unwrap();
    let input_files = vec![PathBuf::from("tests/fixtures/large_image.png")];

    let result = bridge.execute(
        AiTool::RealESRGAN,
        &input_files,
        temp_dir.path(),
        &RealEsrganOptions::default(),
    );

    assert!(matches!(result, Err(AiBridgeError::Timeout(_))));
}
```

### TC-AIB-008: リトライ動作

```rust
#[test]
fn test_retry_behavior() {
    // 最初2回失敗、3回目で成功するモックを使用
    let bridge = create_test_bridge_with_mock();
    let temp_dir = tempfile::tempdir().unwrap();

    let result = bridge.execute(
        AiTool::RealESRGAN,
        &[PathBuf::from("tests/fixtures/image.png")],
        temp_dir.path(),
        &RealEsrganOptions::default(),
    ).unwrap();

    assert_eq!(result.processed_files.len(), 1);
}
```

### TC-AIB-009: 非同期実行と進捗

```rust
#[test]
fn test_async_execution_with_progress() {
    let bridge = create_test_bridge();
    let temp_dir = tempfile::tempdir().unwrap();

    let progress_updates = Arc::new(Mutex::new(Vec::new()));
    let progress_clone = progress_updates.clone();

    let result = bridge.execute_async(
        AiTool::RealESRGAN,
        &[PathBuf::from("tests/fixtures/image.png")],
        temp_dir.path(),
        &RealEsrganOptions::default(),
        Box::new(move |status| {
            progress_clone.lock().unwrap().push(status);
        }),
    ).unwrap();

    let updates = progress_updates.lock().unwrap();
    assert!(!updates.is_empty());

    // 最後のステータスは Completed
    assert!(matches!(updates.last(), Some(ProcessStatus::Completed { .. })));
}
```

### TC-AIB-010: キャンセル

```rust
#[test]
fn test_cancel() {
    let bridge = create_test_bridge();
    let temp_dir = tempfile::tempdir().unwrap();

    // 長時間タスクを開始
    let bridge_clone = bridge.clone();
    let handle = std::thread::spawn(move || {
        bridge_clone.execute(
            AiTool::RealESRGAN,
            &[PathBuf::from("tests/fixtures/large_image.png")],
            temp_dir.path(),
            &RealEsrganOptions::default(),
        )
    });

    std::thread::sleep(Duration::from_millis(100));

    // キャンセル
    bridge.cancel().unwrap();

    let result = handle.join().unwrap();
    // キャンセルまたはタイムアウト
    assert!(result.is_err());
}
```

---

## Implementation Notes

### subprocess を使用した実装

```rust
use std::process::{Command, Stdio, Child};
use std::io::{BufReader, BufRead};

impl SubprocessBridge {
    pub fn new(config: AiBridgeConfig) -> Result<Self> {
        if !config.venv_path.exists() {
            return Err(AiBridgeError::VenvNotFound(config.venv_path.clone()));
        }

        Ok(Self { config })
    }

    fn get_python_path(&self) -> PathBuf {
        if cfg!(windows) {
            self.config.venv_path.join("Scripts").join("python.exe")
        } else {
            self.config.venv_path.join("bin").join("python")
        }
    }

    pub fn execute(
        &self,
        tool: AiTool,
        input_files: &[PathBuf],
        output_dir: &Path,
        tool_options: &dyn std::any::Any,
    ) -> Result<AiTaskResult> {
        let start_time = std::time::Instant::now();
        let python = self.get_python_path();

        let mut processed = Vec::new();
        let mut failed = Vec::new();

        for (retry_count, input_file) in input_files.iter().enumerate() {
            let mut last_error = None;

            for retry in 0..=self.config.retry_config.max_retries {
                let mut cmd = Command::new(&python);

                match tool {
                    AiTool::RealESRGAN => {
                        cmd.arg("-m").arg("realesrgan");
                        // オプション追加
                        if let Some(opts) = tool_options.downcast_ref::<RealEsrganOptions>() {
                            cmd.arg("-s").arg(opts.scale.to_string());
                            if let Some(tile) = self.config.gpu_config.tile_size {
                                cmd.arg("-t").arg(tile.to_string());
                            }
                        }
                        cmd.arg("-i").arg(input_file);
                        cmd.arg("-o").arg(output_dir);
                    }
                    AiTool::YomiToku => {
                        cmd.arg("-m").arg("yomitoku");
                        cmd.arg(input_file);
                        cmd.arg("--output").arg(output_dir);
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
                        last_error = Some(stderr.to_string());

                        if stderr.contains("out of memory") || stderr.contains("CUDA error") {
                            return Err(AiBridgeError::OutOfMemory);
                        }
                    }
                    Err(e) => {
                        last_error = Some(e.to_string());
                    }
                }

                // リトライ間隔
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
            gpu_stats: None, // TODO: GPU統計取得
        })
    }

    pub fn check_gpu(&self) -> Result<GpuStats> {
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
            avg_utilization: 0.0, // TODO
        })
    }
}
```

---

## Acceptance Criteria

- [ ] ブリッジが正常に初期化される
- [ ] 仮想環境なしで適切なエラーを返す
- [ ] ツールの利用可否を確認できる
- [ ] GPU状態を取得できる
- [ ] タスクが正常に実行される
- [ ] バッチ処理が正しく動作する
- [ ] タイムアウトが機能する
- [ ] リトライが正しく動作する
- [ ] 進捗コールバックが呼ばれる
- [ ] キャンセルが機能する

---

## Dependencies

```toml
[dependencies]
thiserror = "2"

[dev-dependencies]
tempfile = "3"
```
