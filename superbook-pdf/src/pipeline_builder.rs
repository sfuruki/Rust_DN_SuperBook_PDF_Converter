use crate::config::PipelineTomlConfig;
use crate::gpu_queue::{GpuJobQueue, GpuQueueConfig};
use crate::{
    CleanupStage, ColorStage, DeskewStage, LoadStage, MarginStage, NormalizeStage, OcrStage,
    PageNumberStage, PipelineRunner, PipelineRunnerConfig, SaveStage, UpscaleStage,
    ValidationStage,
};
use std::path::PathBuf;

/// Build the standard page-processing pipeline shared by CLI and Web execution.
pub fn build_standard_pipeline_runner(
    runner_config: PipelineRunnerConfig,
    input_path: PathBuf,
    output_dir: PathBuf,
    config: &PipelineTomlConfig,
) -> PipelineRunner {
    let queue_cfg = GpuQueueConfig {
        max_in_flight: config.resolved_gpu_stage_parallel(),
        safety_margin_mb: config.resolved_gpu_safety_margin_mb(),
        status_poll_ms: config.resolved_gpu_status_poll_ms(),
    };
    let upscale_url = std::env::var("REALESRGAN_API_URL")
        .unwrap_or_else(|_| "http://localhost:8000".to_string());
    let ocr_url = std::env::var("YOMITOKU_API_URL")
        .unwrap_or_else(|_| "http://localhost:8000".to_string());
    let upscale_queue = GpuJobQueue::new("upscale", upscale_url, queue_cfg.clone());
    let ocr_queue = GpuJobQueue::new("ocr", ocr_url, queue_cfg);

    PipelineRunner::new(runner_config)
        .add_stage(LoadStage::new(input_path, config.load.dpi))
        .add_stage(DeskewStage::new(
            config.correct.enable,
            config.correct.deskew_strength,
        ))
        .add_stage(ColorStage::new(config.correct.color_correction))
        .add_stage(UpscaleStage::new(
            config.upscale.scale,
            config.upscale.model.clone(),
            config.upscale.enable,
            config.upscale.tile,
            config.upscale.fp32,
            upscale_queue,
        ))
        .add_stage(MarginStage::new(config.ocr_pre.margin_trim))
        .add_stage(NormalizeStage::new(config.ocr_pre.normalize_resolution))
        .add_stage(PageNumberStage::new(false))
        .add_stage(OcrStage::new(
            config.ocr.enable,
            config.ocr.language.clone(),
            config.ocr.confidence,
            config.ocr.format.clone(),
            ocr_queue,
        ))
        .add_stage(SaveStage::new(
            output_dir,
            config.save.output_height,
            config.save.jpeg_quality,
        ))
        .add_stage(ValidationStage::new(
            config.validation.enable,
            config.validation.min_chars,
        ))
        // ページ単位 cleanup は失敗時に「最後のページだけ残る」状態を生むため無効化し、
        // 全ページ成功後にのみまとめて削除する。
        .add_stage(CleanupStage::new(false))
}