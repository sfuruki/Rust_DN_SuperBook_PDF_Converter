//! PipelineRunner
//!
//! 構築方針に従い、以下を実現する：
//! - ステージを Vec に積むだけで処理順序が決まる
//! - JoinSet で page_parallel 個を常時稼働させる有界実行
//! - 全ページ一括 spawn を廃止し、巨大 PDF でのメモリ増大を抑制
//! - リトライ（指数バックオフ）を共通化
//! - CLI からも WebUI からも呼び出せる API として実装

use crate::stage::{PageContext, Stage, StageError};
use crate::cpu_queue::{CpuDynamicLimiter, CpuQueueConfig};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::task::JoinSet;

// ============================================================
// RunnerError
// ============================================================

/// PipelineRunner エラー
#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("Stage error: {0}")]
    Stage(#[from] StageError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Page {page_id} failed after {attempts} retries: {message}")]
    MaxRetriesExceeded {
        page_id: usize,
        attempts: u32,
        message: String,
    },
}

// ============================================================
// RetryConfig（指数バックオフ）
// ============================================================

/// リトライ設定
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// 最大リトライ回数（0 = リトライなし）
    pub max_attempts: u32,
    /// 初回バックオフ（ms）
    pub backoff_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            backoff_ms: 500,
        }
    }
}

impl RetryConfig {
    /// 指数バックオフの待機時間を計算する
    pub fn wait_duration(&self, attempt: u32) -> Duration {
        // 2^attempt * backoff_ms（上限 30秒）
        let ms = self.backoff_ms * (1u64 << attempt.min(6));
        Duration::from_millis(ms.min(30_000))
    }
}

// ============================================================
// PipelineRunnerConfig
// ============================================================

/// PipelineRunner の実行設定
#[derive(Debug, Clone)]
pub struct PipelineRunnerConfig {
    /// 最大並列ページ数（JoinSet の同時実行上限）
    pub max_parallel_pages: usize,
    /// CPU 動的並列の最小値
    pub cpu_min_parallel_pages: usize,
    /// CPU 負荷/コアの目標値
    pub cpu_target_load_per_core: f64,
    /// CPU 負荷再評価の間隔（ms）
    pub cpu_status_poll_ms: u64,
    /// 作業ディレクトリの基底パス
    pub work_base_dir: PathBuf,
    /// リトライ設定
    pub retry: RetryConfig,
}

impl Default for PipelineRunnerConfig {
    fn default() -> Self {
        Self {
            max_parallel_pages: 4,
            cpu_min_parallel_pages: 1,
            cpu_target_load_per_core: 0.9,
            cpu_status_poll_ms: 200,
            work_base_dir: PathBuf::from("/data/work"),
            retry: RetryConfig::default(),
        }
    }
}

// ============================================================
// ProgressEvent（進捗通知）
// ============================================================

/// ページ処理の進捗イベント
#[derive(Debug, Clone)]
pub struct ProgressEvent {
    /// ページID
    pub page_id: usize,
    /// 現在のステージ名
    pub stage: String,
    /// 完了済みページ数
    pub completed: usize,
    /// 全ページ数
    pub total: usize,
}

/// 進捗コールバック型
pub type ProgressCallback = Arc<dyn Fn(ProgressEvent) + Send + Sync>;

// ============================================================
// PageResult
// ============================================================

/// 1ページの処理結果
#[derive(Debug)]
pub struct PageResult {
    pub page_id: usize,
    pub success: bool,
    pub error: Option<String>,
    pub skipped_stages: Vec<String>,
}

// ============================================================
// PipelineRunner
// ============================================================

/// パイプライン実行エンジン
///
/// CLI と WebUI の両方から呼び出せる共通 API。
/// stages を Vec に積み、ページ単位で tokio::spawn して並列処理する。
pub struct PipelineRunner {
    stages: Vec<Arc<dyn Stage>>,
    config: PipelineRunnerConfig,
}

impl PipelineRunner {
    /// 新しい PipelineRunner を作成する
    pub fn new(config: PipelineRunnerConfig) -> Self {
        Self {
            stages: Vec::new(),
            config,
        }
    }

    /// ステージを追加する（処理順序 = 追加順）
    pub fn add_stage(mut self, stage: impl Stage + 'static) -> Self {
        self.stages.push(Arc::new(stage));
        self
    }

    /// 有効なステージ数を返す
    pub fn stage_count(&self) -> usize {
        self.stages.iter().filter(|s| s.is_enabled()).count()
    }

    /// 全ページを有界並列処理する
    ///
    /// `max_parallel_pages` 個のスロットを JoinSet で制御し、常時その数だけページを処理する。
    /// 全ページ一括 spawn を廃止したため、巨大 PDF でもメモリ使用量を抑制できる。
    ///
    /// # Arguments
    /// * `page_count` - 処理するページ数
    /// * `progress_cb` - 進捗コールバック（None = 無し）
    ///
    /// # Returns
    /// 各ページの処理結果（page_id 昇順でソート済み）
    pub async fn run_all(
        &self,
        page_count: usize,
        progress_cb: Option<ProgressCallback>,
    ) -> Vec<PageResult> {
        let stages = Arc::new(self.stages.clone());
        let config = Arc::new(self.config.clone());
        let progress_cb = progress_cb.map(Arc::new);
        let completed = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let mut join_set: JoinSet<PageResult> = JoinSet::new();
        let mut results = Vec::with_capacity(page_count);
        let mut submitted = 0usize;
        let mut cpu_limiter = CpuDynamicLimiter::new(
            self.config.max_parallel_pages,
            CpuQueueConfig {
                min_in_flight: self.config.cpu_min_parallel_pages,
                target_load_per_core: self.config.cpu_target_load_per_core,
                status_poll_ms: self.config.cpu_status_poll_ms,
            },
        );

        loop {
            let allowed_parallel = cpu_limiter.current_limit();
            // 空きスロット分だけ次のページを投入する
            while submitted < page_count && join_set.len() < allowed_parallel {
                submitted += 1;
                let page_id = submitted;
                let stages = stages.clone();
                let config = config.clone();
                let progress_cb = progress_cb.clone();
                let completed = completed.clone();

                join_set.spawn(async move {
                    run_page_with_retry(
                        page_id,
                        page_count,
                        &stages,
                        &config,
                        progress_cb.as_deref(),
                        &completed,
                    )
                    .await
                });
            }

            match join_set.join_next().await {
                None => break,
                Some(Ok(result)) => results.push(result),
                Some(Err(e)) => results.push(PageResult {
                    page_id: 0,
                    success: false,
                    error: Some(format!("Task panic: {}", e)),
                    skipped_stages: vec![],
                }),
            }
        }

        // ページIDで並び直し
        results.sort_by_key(|r| r.page_id);
        results
    }
}

// ============================================================
// 内部実装
// ============================================================

/// 1ページをリトライ付きで処理する
async fn run_page_with_retry(
    page_id: usize,
    total_pages: usize,
    stages: &[Arc<dyn Stage>],
    config: &PipelineRunnerConfig,
    progress_cb: Option<&Arc<dyn Fn(ProgressEvent) + Send + Sync>>,
    completed: &std::sync::atomic::AtomicUsize,
) -> PageResult {
    let max_attempts = config.retry.max_attempts.max(1);
    let mut last_error = String::new();
    let mut skipped_stages = Vec::new();

    for attempt in 0..max_attempts {
        if attempt > 0 {
            let wait = config.retry.wait_duration(attempt - 1);
            tokio::time::sleep(wait).await;
        }

        match run_page(
            page_id,
            stages,
            config,
            progress_cb,
            &mut skipped_stages,
        )
        .await
        {
            Ok(()) => {
                let done = completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if let Some(cb) = progress_cb {
                    cb(ProgressEvent {
                        page_id,
                        stage: "done".to_string(),
                        completed: done,
                        total: total_pages,
                    });
                }
                return PageResult {
                    page_id,
                    success: true,
                    error: None,
                    skipped_stages,
                };
            }
            Err(e) => {
                last_error = e.to_string();
                if attempt + 1 < max_attempts {
                    eprintln!(
                        "[runner] Page {} attempt {}/{} failed: {}. Retrying...",
                        page_id,
                        attempt + 1,
                        max_attempts,
                        last_error
                    );
                }
            }
        }
    }

    PageResult {
        page_id,
        success: false,
        error: Some(last_error),
        skipped_stages,
    }
}

/// 1ページのステージ全てを実行する
async fn run_page(
    page_id: usize,
    stages: &[Arc<dyn Stage>],
    config: &PipelineRunnerConfig,
    progress_cb: Option<&Arc<dyn Fn(ProgressEvent) + Send + Sync>>,
    skipped_stages: &mut Vec<String>,
) -> Result<(), StageError> {
    let mut ctx = PageContext::new(page_id, &config.work_base_dir);
    ctx.ensure_work_dir().map_err(|e| StageError::Io {
        stage: "runner",
        source: e,
    })?;

    for stage in stages.iter() {
        if !stage.is_enabled() {
            skipped_stages.push(format!("{} (disabled)", stage.name()));
            continue;
        }

        if let Some(cb) = progress_cb {
            cb(ProgressEvent {
                page_id,
                stage: stage.name().to_string(),
                completed: 0,
                total: 0,
            });
        }

        match stage.run(&mut ctx).await {
            Ok(()) => {}
            Err(StageError::Skipped { stage: s, reason }) => {
                skipped_stages.push(format!("{}: {}", s, reason));
                // スキップは続行
            }
            Err(e) => return Err(e),
        }
    }

    Ok(())
}
