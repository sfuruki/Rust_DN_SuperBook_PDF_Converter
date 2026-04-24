//! Shared pipeline types and utilities.
//!
//! Legacy monolithic processing has been removed. This module now keeps only
//! configuration, errors, and generic helpers shared across CLI/Web pipelines.

use crate::cli::ConvertArgs;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use thiserror::Error;

/// Estimated memory per image at 300 DPI (A4 size, RGBA)
const ESTIMATED_IMAGE_MEMORY_MB: usize = 100;

/// Minimum chunk size for parallel processing
const MIN_CHUNK_SIZE: usize = 4;

/// Default memory limit if not specified (4GB)
const DEFAULT_MEMORY_LIMIT_MB: usize = 4096;

/// Calculate optimal chunk size based on memory constraints.
pub fn calculate_optimal_chunk_size(
    total_items: usize,
    max_memory_mb: usize,
    threads: usize,
) -> usize {
    let memory_limit = if max_memory_mb == 0 {
        get_available_memory_mb().unwrap_or(DEFAULT_MEMORY_LIMIT_MB)
    } else {
        max_memory_mb
    };

    let usable_memory = memory_limit / 2;
    let max_concurrent = threads.max(num_cpus::get());
    let per_thread_capacity = usable_memory / ESTIMATED_IMAGE_MEMORY_MB;
    let concurrent_capacity = per_thread_capacity.min(max_concurrent);
    let chunk_size = concurrent_capacity.max(MIN_CHUNK_SIZE);

    chunk_size.min(total_items).max(1)
}

#[cfg(target_os = "linux")]
fn get_available_memory_mb() -> Option<usize> {
    use std::fs;

    let meminfo = fs::read_to_string("/proc/meminfo").ok()?;
    for line in meminfo.lines() {
        if line.starts_with("MemAvailable:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let kb: usize = parts[1].parse().ok()?;
                return Some(kb / 1024);
            }
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn get_available_memory_mb() -> Option<usize> {
    Some(DEFAULT_MEMORY_LIMIT_MB)
}

/// Process items in chunks for memory-controlled parallel execution.
pub fn process_in_chunks<T, R, F, P>(
    items: &[T],
    chunk_size: usize,
    processor: F,
    progress: Option<&P>,
) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
    P: Fn(usize, usize) + Sync,
{
    let total = items.len();
    if total == 0 {
        return vec![];
    }

    let effective_chunk_size = if chunk_size == 0 { total } else { chunk_size };
    let completed = AtomicUsize::new(0);
    let mut indexed_results: Vec<(usize, R)> = Vec::with_capacity(total);

    for chunk_start in (0..total).step_by(effective_chunk_size) {
        let chunk_end = (chunk_start + effective_chunk_size).min(total);
        let chunk: Vec<(usize, &T)> = (chunk_start..chunk_end).map(|i| (i, &items[i])).collect();

        let chunk_results: Vec<(usize, R)> = chunk
            .par_iter()
            .map(|(idx, item)| {
                let result = processor(item);
                let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                if let Some(cb) = progress {
                    cb(done, total);
                }
                (*idx, result)
            })
            .collect();

        indexed_results.extend(chunk_results);
    }

    indexed_results.sort_by_key(|(idx, _)| *idx);
    indexed_results.into_iter().map(|(_, r)| r).collect()
}

/// Progress callback shared by non-stage pipelines (e.g. markdown).
pub trait ProgressCallback: Send + Sync {
    fn on_step_start(&self, step: &str);
    fn on_step_progress(&self, current: usize, total: usize);
    fn on_step_complete(&self, step: &str, message: &str);
    fn on_debug(&self, message: &str);
    fn on_warning(&self, message: &str) {
        eprintln!("Warning: {}", message);
    }
}

/// No-op progress callback.
pub struct SilentProgress;

impl ProgressCallback for SilentProgress {
    fn on_step_start(&self, _step: &str) {}
    fn on_step_progress(&self, _current: usize, _total: usize) {}
    fn on_step_complete(&self, _step: &str, _message: &str) {}
    fn on_debug(&self, _message: &str) {}
    fn on_warning(&self, _message: &str) {}
}

/// Shared pipeline error.
#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("Input file not found: {0}")]
    InputNotFound(PathBuf),

    #[error("Output directory not writable: {0}")]
    OutputNotWritable(PathBuf),

    #[error("PDF extraction failed: {0}")]
    ExtractionFailed(String),

    #[error("Image processing failed: {0}")]
    ImageProcessingFailed(String),

    #[error("PDF generation failed: {0}")]
    PdfGenerationFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Shared processing configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub dpi: u32,
    pub deskew: bool,
    pub margin_trim: f64,
    pub upscale: bool,
    pub gpu: bool,
    pub internal_resolution: bool,
    pub color_correction: bool,
    pub output_height: u32,
    pub ocr: bool,
    pub max_pages: Option<usize>,
    pub save_debug: bool,
    pub jpeg_quality: u8,
    pub threads: Option<usize>,
    #[serde(default)]
    pub max_memory_mb: usize,
    #[serde(default)]
    pub chunk_size: usize,
    #[serde(default = "default_true")]
    pub assume_japanese_book: bool,
    #[serde(skip)]
    pub override_work_dir: Option<PathBuf>,
}

fn default_true() -> bool {
    true
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            dpi: 300,
            deskew: true,
            margin_trim: 0.7,
            upscale: true,
            gpu: true,
            internal_resolution: false,
            color_correction: true,
            output_height: 3508,
            ocr: false,
            max_pages: None,
            save_debug: false,
            jpeg_quality: 90,
            threads: None,
            max_memory_mb: 0,
            chunk_size: 0,
            assume_japanese_book: true,
            override_work_dir: None,
        }
    }
}

impl PipelineConfig {
    pub fn from_convert_args(args: &ConvertArgs) -> Self {
        let advanced = args.advanced;
        Self {
            dpi: args.dpi,
            deskew: args.effective_deskew(),
            margin_trim: args.margin_trim as f64,
            upscale: args.effective_upscale(),
            gpu: args.effective_gpu(),
            internal_resolution: args.internal_resolution || advanced,
            color_correction: args.color_correction || advanced,
            output_height: args.output_height,
            ocr: args.ocr,
            max_pages: args.max_pages,
            save_debug: args.save_debug,
            jpeg_quality: args.jpeg_quality,
            threads: args.threads,
            max_memory_mb: 0,
            chunk_size: 0,
            assume_japanese_book: true,
            override_work_dir: args.work_dir.clone(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn with_dpi(mut self, dpi: u32) -> Self {
        self.dpi = dpi;
        self
    }

    pub fn with_deskew(mut self, enabled: bool) -> Self {
        self.deskew = enabled;
        self
    }

    pub fn with_margin_trim(mut self, percent: f64) -> Self {
        self.margin_trim = percent;
        self
    }

    pub fn with_upscale(mut self, enabled: bool) -> Self {
        self.upscale = enabled;
        self
    }

    pub fn with_gpu(mut self, enabled: bool) -> Self {
        self.gpu = enabled;
        self
    }

    pub fn with_ocr(mut self, enabled: bool) -> Self {
        self.ocr = enabled;
        self
    }

    pub fn with_max_pages(mut self, max: Option<usize>) -> Self {
        self.max_pages = max;
        self
    }

    pub fn with_advanced(mut self) -> Self {
        self.internal_resolution = true;
        self.color_correction = true;
        self
    }
}
