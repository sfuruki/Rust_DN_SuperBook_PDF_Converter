//! Background worker for processing PDF conversion jobs
//!
//! Handles the actual PDF conversion in a background task.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::config::PipelineTomlConfig;
use super::job::{JobQueue, JobStatus, Progress};
use super::metrics::MetricsCollector;
use super::websocket::WsBroadcaster;
use crate::{
    build_standard_pipeline_runner, PdfWriterOptions, PipelineRunnerConfig, PrintPdfWriter,
    ProgressEvent, RetryConfig,
};

/// Worker message types
#[derive(Debug)]
pub enum WorkerMessage {
    /// Process a job with the given ID and input file path
    Process {
        job_id: Uuid,
        input_path: PathBuf,
        effective_config: PipelineTomlConfig,
    },
    /// Shutdown the worker
    Shutdown,
}

fn stage_to_step(stage: &str) -> (u32, &'static str) {
    match stage {
        "load" => (2, "Load"),
        "deskew" => (3, "Deskew"),
        "color" => (4, "Color"),
        "upscale" => (5, "Upscale"),
        "margin" => (6, "Margin"),
        "normalize" => (7, "Normalize"),
        "page_number" => (8, "PageNumber"),
        "ocr" => (9, "OCR"),
        "save" => (10, "Save"),
        "validation" => (11, "Validation"),
        "cleanup" => (12, "Cleanup"),
        "done" => (13, "Finalize"),
        _ => (1, "Starting"),
    }
}

struct ProgressDispatch {
    current_step: u32,
    total_steps: u32,
    step_name: String,
    percent: u8,
}

fn output_name_from_input_filename(input_filename: &str) -> String {
    let stem = std::path::Path::new(input_filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("output");
    format!("{}_superbook.pdf", stem)
}

fn preferred_output_path(
    output_dir: &std::path::Path,
    input_filename: &str,
    job_id: Uuid,
) -> PathBuf {
    let preferred = output_dir.join(output_name_from_input_filename(input_filename));
    if !preferred.exists() {
        return preferred;
    }

    let stem = std::path::Path::new(input_filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("output");
    let job_id_str = job_id.to_string();
    let short_id = &job_id_str[..8];
    output_dir.join(format!("{}_{}_superbook.pdf", stem, short_id))
}

/// Background worker for job processing
pub struct JobWorker {
    queue: JobQueue,
    receiver: mpsc::Receiver<WorkerMessage>,
    work_dir: PathBuf,
    output_dir: PathBuf,
    broadcaster: Arc<WsBroadcaster>,
    metrics: Arc<MetricsCollector>,
}

impl JobWorker {
    /// Create a new worker
    pub fn new(
        queue: JobQueue,
        receiver: mpsc::Receiver<WorkerMessage>,
        work_dir: PathBuf,
        output_dir: PathBuf,
        broadcaster: Arc<WsBroadcaster>,
        metrics: Arc<MetricsCollector>,
    ) -> Self {
        Self {
            queue,
            receiver,
            work_dir,
            output_dir,
            broadcaster,
            metrics,
        }
    }

    /// Run the worker loop
    pub async fn run(mut self) {
        while let Some(msg) = self.receiver.recv().await {
            match msg {
                WorkerMessage::Process {
                    job_id,
                    input_path,
                    effective_config,
                } => {
                    self.process_job(job_id, input_path, effective_config)
                        .await;
                }
                WorkerMessage::Shutdown => {
                    break;
                }
            }
        }
    }

    /// Process a single job with actual pipeline
    pub async fn process_job(
        &self,
        job_id: Uuid,
        input_path: PathBuf,
        config: PipelineTomlConfig,
    ) {
        let input_filename = self
            .queue
            .get(job_id)
            .map(|job| job.input_filename)
            .unwrap_or_else(|| {
                input_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("upload.pdf")
                    .to_string()
            });

        const TOTAL_STEPS: u32 = 13;

        // Check if job was cancelled before moving it to processing.
        if let Some(job) = self.queue.get(job_id) {
            if job.status == JobStatus::Cancelled {
                return;
            }
        }

        // Mark job as processing
        self.queue.update(job_id, |job| {
            job.start();
            job.update_progress(Progress::new(1, TOTAL_STEPS, "Starting"));
        });
        self.metrics.record_job_started();
        let queue_wait_ms = self
            .queue
            .get(job_id)
            .map(|job| (chrono::Utc::now() - job.created_at).num_milliseconds().max(0) as u64)
            .unwrap_or(0);
        self.metrics.record_job_queue_wait_ms(queue_wait_ms);

        // Broadcast status change via WebSocket
        self.broadcaster
            .broadcast_status_change(job_id, JobStatus::Queued, JobStatus::Processing)
            .await;
        self.broadcaster
            .broadcast_progress(job_id, 1, TOTAL_STEPS, "Starting")
            .await;

        // Use a per-job work directory for intermediate files.
        let processing_dir = self.work_dir.join(format!("job_{}", job_id.simple()));
        if let Err(e) = std::fs::create_dir_all(&processing_dir) {
            let error_msg = format!("Failed to create working directory: {}", e);
            self.queue.update(job_id, |job| {
                job.fail(error_msg.clone());
            });
            self.metrics.record_job_failed_with_timing(0, 0);
            self.broadcaster.broadcast_error(job_id, &error_msg).await;
            return;
        }

        // Keep output_dir for final artifacts only.
        let final_output_dir = self.output_dir.clone();
        if let Err(e) = std::fs::create_dir_all(&final_output_dir) {
            let error_msg = format!("Failed to create output directory: {}", e);
            self.queue.update(job_id, |job| {
                job.fail(error_msg.clone());
            });
            self.metrics.record_job_failed_with_timing(0, 0);
            self.broadcaster.broadcast_error(job_id, &error_msg).await;
            return;
        }

        let page_count = match crate::LopdfReader::new(&input_path) {
            Ok(reader) => reader.info.page_count,
            Err(e) => {
                let error_msg = format!("Failed to read input PDF: {}", e);
                self.queue.update(job_id, |job| {
                    job.fail(error_msg.clone());
                });
                self.metrics.record_job_failed_with_timing(0, 0);
                self.broadcaster.broadcast_error(job_id, &error_msg).await;
                return;
            }
        };

        let start = std::time::Instant::now();
        let queue = self.queue.clone();
        let broadcaster = self.broadcaster.clone();
        let done_pages = Arc::new(AtomicUsize::new(0));
        let done_pages_cb = done_pages.clone();
        let (progress_tx, mut progress_rx) = mpsc::unbounded_channel::<ProgressDispatch>();
        let progress_tx_cb = progress_tx.clone();
        let progress_queue = queue.clone();
        let progress_broadcaster = broadcaster.clone();
        let progress_task = tokio::spawn(async move {
            while let Some(msg) = progress_rx.recv().await {
                let progress = Progress {
                    current_step: msg.current_step,
                    total_steps: msg.total_steps,
                    step_name: msg.step_name.clone(),
                    percent: msg.percent,
                };
                progress_queue.update(job_id, |job| {
                    job.update_progress(progress.clone());
                });
                progress_broadcaster
                    .broadcast_progress_precise(
                        job_id,
                        msg.current_step,
                        msg.total_steps,
                        &msg.step_name,
                        msg.percent,
                    )
                    .await;
            }
        });

        let runner = build_standard_pipeline_runner(
            PipelineRunnerConfig {
                max_parallel_pages: config.resolved_page_parallel(),
                cpu_min_parallel_pages: config.resolved_cpu_dynamic_min_parallel(),
                cpu_target_load_per_core: config.resolved_cpu_target_load_per_core(),
                cpu_status_poll_ms: config.resolved_cpu_status_poll_ms(),
                work_base_dir: processing_dir.clone(),
                retry: RetryConfig {
                    max_attempts: config.retry.max_attempts,
                    backoff_ms: config.retry.backoff_ms,
                },
            },
            input_path.clone(),
            final_output_dir.clone(),
            &config,
        );

        let page_results = runner
            .run_all(
                page_count,
                Some(Arc::new(move |event: ProgressEvent| {
                    let (current_step, step_name) = stage_to_step(&event.stage);
                    let completed_pages = if event.stage == "done" {
                        done_pages_cb.fetch_add(1, Ordering::Relaxed) + 1
                    } else {
                        done_pages_cb.load(Ordering::Relaxed)
                    };

                    let fraction = if page_count == 0 {
                        0.0
                    } else {
                        completed_pages as f32 / page_count as f32
                    };
                    let overall = ((current_step.saturating_sub(1)) as f32 + fraction)
                        / TOTAL_STEPS as f32;
                    let percent = (overall.clamp(0.0, 1.0) * 100.0) as u8;
                    let _ = progress_tx_cb.send(ProgressDispatch {
                        current_step,
                        total_steps: TOTAL_STEPS,
                        step_name: step_name.to_string(),
                        percent,
                    });
                })),
            )
            .await;
        drop(progress_tx);
        let _ = progress_task.await;

        let elapsed_ms = start.elapsed().as_millis() as u64;
        let gpu_stage_ms = if config.upscale.enable || config.ocr.enable {
            elapsed_ms
        } else {
            0
        };

        let ok_pages = page_results.iter().filter(|r| r.success).count();
        if ok_pages != page_count {
            let first_error = page_results
                .iter()
                .find(|r| !r.success)
                .and_then(|r| r.error.clone())
                .unwrap_or_else(|| "Unknown page error".to_string());
            let error_msg = format!("PipelineRunner error: {}", first_error);
            self.queue.update(job_id, |job| {
                job.fail(error_msg.clone());
            });
            self.metrics
                .record_job_failed_with_timing(elapsed_ms, gpu_stage_ms);
            self.broadcaster.broadcast_error(job_id, &error_msg).await;
            std::fs::remove_dir_all(&processing_dir).ok();
            return;
        }

        let page_images: Vec<PathBuf> = (0..page_count)
            .map(|i| processing_dir.join(format!("{:04}", i)).join("gaozou.webp"))
            .collect();

        if page_images.iter().any(|p| !p.exists()) {
            let error_msg = "PipelineRunner completed but output page image is missing".to_string();
            self.queue.update(job_id, |job| {
                job.fail(error_msg.clone());
            });
            self.metrics
                .record_job_failed_with_timing(elapsed_ms, gpu_stage_ms);
            self.broadcaster.broadcast_error(job_id, &error_msg).await;
            std::fs::remove_dir_all(&processing_dir).ok();
            return;
        }

        let final_output_path = preferred_output_path(&final_output_dir, &input_filename, job_id);
        let pdf_opts = PdfWriterOptions::builder()
            .dpi(config.load.dpi)
            .jpeg_quality(config.save.jpeg_quality)
            .build();

        if let Err(e) = PrintPdfWriter::create_from_images(&page_images, &final_output_path, &pdf_opts)
        {
            let error_msg = format!("Failed to create PDF: {}", e);
            self.queue.update(job_id, |job| {
                job.fail(error_msg.clone());
            });
            self.metrics
                .record_job_failed_with_timing(elapsed_ms, gpu_stage_ms);
            self.broadcaster.broadcast_error(job_id, &error_msg).await;
            std::fs::remove_dir_all(&processing_dir).ok();
            return;
        }

        let elapsed = start.elapsed().as_secs_f64();
        self.metrics
            .record_job_completed_with_gpu(elapsed_ms, page_count as u64, gpu_stage_ms);
        self.queue.update(job_id, |job| {
            job.complete(final_output_path);
        });
        self.broadcaster
            .broadcast_completed(job_id, elapsed, page_count)
            .await;

        if config.cleanup.enable {
            std::fs::remove_dir_all(&processing_dir).ok();
        }
    }
}

#[cfg(test)]
mod naming_tests {
    use super::*;

    #[test]
    fn test_output_name_from_input_filename() {
        assert_eq!(
            output_name_from_input_filename("sample-book.pdf"),
            "sample-book_superbook.pdf"
        );
        assert_eq!(
            output_name_from_input_filename("noext"),
            "noext_superbook.pdf"
        );
    }
}

/// Worker pool for managing multiple workers
pub struct WorkerPool {
    sender: mpsc::Sender<WorkerMessage>,
    work_dir: PathBuf,
    output_dir: PathBuf,
    worker_count: usize,
}

impl WorkerPool {
    /// Create a new worker pool
    pub fn new(
        queue: JobQueue,
        work_dir: PathBuf,
        output_dir: PathBuf,
        worker_count: usize,
        broadcaster: Arc<WsBroadcaster>,
        metrics: Arc<MetricsCollector>,
    ) -> Self {
        let (sender, receiver) = mpsc::channel::<WorkerMessage>(100);

        // Spawn workers
        let receiver = Arc::new(tokio::sync::Mutex::new(receiver));

        for _ in 0..worker_count {
            let queue = queue.clone();
            let work_dir = work_dir.clone();
            let output_dir = output_dir.clone();
            let receiver = receiver.clone();
            let broadcaster = broadcaster.clone();
            let metrics = metrics.clone();

            tokio::spawn(async move {
                loop {
                    let msg = {
                        let mut rx = receiver.lock().await;
                        rx.recv().await
                    };

                    match msg {
                        Some(WorkerMessage::Process {
                            job_id,
                            input_path,
                            effective_config,
                        }) => {
                            // Create a temporary worker for this job
                            let (_, dummy_rx) = mpsc::channel(1);
                            let worker = JobWorker::new(
                                queue.clone(),
                                dummy_rx,
                                work_dir.clone(),
                                output_dir.clone(),
                                broadcaster.clone(),
                                metrics.clone(),
                            );
                            worker
                                .process_job(job_id, input_path, effective_config)
                                .await;
                        }
                        Some(WorkerMessage::Shutdown) | None => {
                            break;
                        }
                    }
                }
            });
        }

        Self {
            sender,
            work_dir,
            output_dir,
            worker_count,
        }
    }

    /// Submit a job for processing
    pub async fn submit(
        &self,
        job_id: Uuid,
        input_path: PathBuf,
        effective_config: PipelineTomlConfig,
    ) -> Result<(), String> {
        self.sender
            .send(WorkerMessage::Process {
                job_id,
                input_path,
                effective_config,
            })
            .await
            .map_err(|e| format!("Failed to submit job: {}", e))
    }

    /// Get the work directory
    pub fn work_dir(&self) -> &PathBuf {
        &self.work_dir
    }

    /// Get the output directory
    pub fn output_dir(&self) -> &PathBuf {
        &self.output_dir
    }

    /// Shutdown all workers
    pub async fn shutdown(&self) {
        // Send shutdown message (workers will exit after current job)
        let _ = self.sender.send(WorkerMessage::Shutdown).await;
    }

    /// Get the number of workers
    pub fn worker_count(&self) -> usize {
        self.worker_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_server::job::Job;

    #[tokio::test]
    async fn test_worker_message_debug() {
        let msg = WorkerMessage::Process {
            job_id: Uuid::new_v4(),
            input_path: PathBuf::from("/test.pdf"),
            effective_config: PipelineTomlConfig::default(),
        };
        let debug = format!("{:?}", msg);
        assert!(debug.contains("Process"));
    }

    #[tokio::test]
    async fn test_worker_pool_creation() {
        let queue = JobQueue::new();
        let work_dir = std::env::temp_dir();
        let output_dir = work_dir.join("output");
        let broadcaster = Arc::new(WsBroadcaster::new());
        let metrics = Arc::new(MetricsCollector::new());
        let _pool = WorkerPool::new(queue, work_dir, output_dir, 2, broadcaster, metrics);
        // Pool created successfully
    }

    #[tokio::test]
    async fn test_page_parallel_used_directly() {
        let mut config = PipelineTomlConfig::default();
        config.concurrency.page_parallel = 3;
        assert_eq!(config.resolved_page_parallel(), 3);

        config.concurrency.page_parallel = 0;
        let expected_cpu = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .max(1);
        assert_eq!(config.resolved_page_parallel(), expected_cpu);
    }

    #[tokio::test]
    async fn test_job_processing_with_invalid_pdf() {
        let queue = JobQueue::new();
        let work_dir = std::env::temp_dir().join("superbook_test_worker");
        let output_dir = work_dir.join("output");
        std::fs::create_dir_all(&work_dir).ok();

        let broadcaster = Arc::new(WsBroadcaster::new());
        let metrics = Arc::new(MetricsCollector::new());
        let pool = WorkerPool::new(
            queue.clone(),
            work_dir.clone(),
            output_dir,
            1,
            broadcaster,
            metrics,
        );

        // Create a job
        let job = Job::new("test.pdf", crate::api_server::job::ConvertOptions::default());
        let job_id = job.id;
        queue.submit(job);

        // Submit for processing with invalid PDF
        let input_path = work_dir.join("invalid.pdf");
        std::fs::write(&input_path, b"not a valid pdf").ok();

        pool.submit(job_id, input_path, PipelineTomlConfig::default())
            .await
            .unwrap();

        // Wait for processing
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        // Job should fail because input is not a valid PDF
        let job = queue.get(job_id).unwrap();
        assert!(
            job.status == JobStatus::Failed || job.status == JobStatus::Processing,
            "Job should be failed or still processing, got {:?}",
            job.status
        );

        // Cleanup
        std::fs::remove_dir_all(&work_dir).ok();
    }
}
