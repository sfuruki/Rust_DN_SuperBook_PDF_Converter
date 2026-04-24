//! superbook-pdf - High-quality PDF converter for scanned books
//!
//! CLI entry point

use clap::Parser;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;
use superbook_pdf::ai_bridge::{AiBridge, AiTool, HttpApiBridge};
use superbook_pdf::{
    build_standard_pipeline_runner,
    exit_codes, should_skip_processing, ApplyConfigArgs,
    CacheDigest, CacheInfoArgs, CleanupStage, Cli, CliOverrides, ColorStage, Commands, Config,
    ConvertArgs, DeskewStage, ListConfigArgs, LoadStage, MarginStage, MarkdownArgs,
    MarkdownMergeStage, MarkdownStage, NormalizeStage, OcrStage, PageNumberStage, SaveStage,
    PageStatus, PdfWriterOptions, PipelineRunner, PipelineRunnerConfig, PipelineTomlConfig,
    PreviewArgs, PrintPdfWriter, ProcessingResult, ProgressCallback, ProgressEvent,
    ProgressTracker, ReprocessArgs, ReprocessOptions, ReprocessState, RetryConfig, RunArgs,
    ProcessingCache, UpscaleStage, ValidationStage, GpuJobQueue, GpuQueueConfig,
};

#[cfg(feature = "web")]
use superbook_pdf::{ServeArgs, ServerConfig, WebServer};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    println!("🚀 Mode: Microservice (HTTP API)");
    let config = superbook_pdf::AiBridgeConfig::default();
    if let Ok(bridge) = HttpApiBridge::new(config) {
        println!("🔍 Initializing AI Service Handshake...");
        if let Ok(true) = bridge.check_tool(AiTool::RealESRGAN).await {
        } else {
            eprintln!("  ⚠️  RealESRGAN API: Connection failed or incompatible version");
        }
        if let Ok(true) = bridge.check_tool(AiTool::YomiToku).await {
        } else {
            eprintln!("  ⚠️  YomiToku API: Connection failed or incompatible version");
        }
    }

    let result = match cli.command {
        Commands::Convert(args) => run_convert(&args).await,
        Commands::Markdown(args) => run_markdown(&args).await,
        Commands::Reprocess(args) => run_reprocess(&args),
        Commands::Info => run_info(),
        Commands::CacheInfo(args) => run_cache_info(&args),
        #[cfg(feature = "web")]
        Commands::Serve(args) => run_serve(&args).await,
        Commands::Run(args) => run_pipeline(&args).await,
        Commands::ListConfig(args) => run_list_config(&args),
        Commands::ApplyConfig(args) => run_apply_config(&args),
        Commands::Preview(args) => run_preview(&args).await,
    };

    std::process::exit(match result {
        Ok(()) => exit_codes::SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            exit_codes::GENERAL_ERROR
        }
    });
}

// ============ Progress Callback Implementation ============

/// Verbose progress callback for CLI output
struct VerboseProgress {
    verbose_level: u32,
}

impl VerboseProgress {
    /// Check if step messages should be shown (level >= 1)
    #[allow(dead_code)]
    fn should_show_steps(&self) -> bool {
        self.verbose_level > 0
    }

    /// Check if progress messages should be shown (level >= 1)
    #[allow(dead_code)]
    fn should_show_progress(&self) -> bool {
        self.verbose_level > 0
    }

    /// Check if debug messages should be shown (level >= 3, i.e., -vvv)
    #[allow(dead_code)]
    fn should_show_debug(&self) -> bool {
        self.verbose_level > 2
    }
}

impl ProgressCallback for VerboseProgress {
    fn on_step_start(&self, step: &str) {
        if self.verbose_level > 0 {
            println!("  {}", step);
        }
    }

    fn on_step_progress(&self, current: usize, total: usize) {
        if self.verbose_level > 0 {
            print!("\r    Progress: {}/{}", current, total);
            std::io::stdout().flush().ok();
        }
    }

    fn on_step_complete(&self, step: &str, message: &str) {
        if self.verbose_level > 0 {
            println!("    {}: {}", step, message);
        }
    }

    fn on_debug(&self, message: &str) {
        if self.should_show_debug() {
            println!("    [DEBUG] {}", message);
        }
    }

    fn on_warning(&self, message: &str) {
        // Warnings always shown (even at verbose level 0)
        eprintln!("    [WARNING] {}", message);
    }
}

// ============ Convert Command ============

async fn run_convert(args: &ConvertArgs) -> Result<(), Box<dyn std::error::Error>> {
    let start_time = Instant::now();

    // 入力パスのバリデーション [3]
    if !args.input.exists() {
        eprintln!("Error: Input path does not exist: {}", args.input.display());
        std::process::exit(exit_codes::INPUT_NOT_FOUND);
    }

    // 処理対象のPDFファイルを収集 [3, 4]
    let pdf_files = collect_pdf_files(&args.input)?;
    if pdf_files.is_empty() {
        eprintln!("Error: No PDF files found in input path");
        std::process::exit(exit_codes::INPUT_NOT_FOUND);
    }

    // 設定ファイルの読み込み [3]
    let file_config = match &args.config {
        Some(config_path) => match Config::load_from_path(config_path) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("Warning: Failed to load config file: {}", e);
                Config::default()
            }
        },
        None => Config::load().unwrap_or_default(),
    };

    // CLI引数による上書き設定の適用 [3, 5]
    let cli_overrides = create_cli_overrides(args);
    let mut pipeline_config = file_config.merge_with_cli(&cli_overrides);

    // -o 未指定時は SUPERBOOK_OUTPUT_DIR 環境変数、なければ ./output
    let output_dir: PathBuf = args.output.clone().unwrap_or_else(|| {
        std::env::var("SUPERBOOK_OUTPUT_DIR")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("./output"))
    });

    // --work-dir 未指定時は SUPERBOOK_WORK_DIR 環境変数
    let work_dir: Option<PathBuf> = args.work_dir.clone().or_else(|| {
        std::env::var("SUPERBOOK_WORK_DIR")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from)
    });
    pipeline_config.override_work_dir = work_dir.clone();

    // ドライラン（実行計画の表示）モード [3, 4]
    if args.dry_run {
        print_execution_plan(args, &pdf_files, &pipeline_config);
        return Ok(());
    }

    // 出力ディレクトリの作成
    std::fs::create_dir_all(&output_dir)?;
    if let Some(wd) = &work_dir {
        std::fs::create_dir_all(wd)?;
    };
    let verbose = args.verbose > 0;
    let options_json = pipeline_config.to_json();

    // 処理結果のカウント用変数
    let mut ok_count = 0usize;
    let mut skip_count = 0usize;
    let mut error_count = 0usize;

    // 非同期ループ内で実行
    for (idx, pdf_path) in pdf_files.iter().enumerate() {
        let output_pdf = output_path_from_input(pdf_path, &output_dir);

        // キャッシュによるスキップ判定 [3]
        if args.skip_existing && !args.force {
            if output_pdf.exists() {
                if verbose {
                    println!(
                        "[{}/{}] Skipping (exists): {}",
                        idx + 1,
                        pdf_files.len(),
                        pdf_path.display()
                    );
                }
                skip_count += 1;
                continue;
            }
        } else if !args.force {
            if let Some(cache) = should_skip_processing(pdf_path, &output_pdf, &options_json, false)
            {
                if verbose {
                    println!(
                        "[{}/{}] Skipping (cached, {} pages): {}",
                        idx + 1,
                        pdf_files.len(),
                        cache.result.page_count,
                        pdf_path.display()
                    );
                }
                skip_count += 1;
                continue;
            }
        }

        if verbose {
            println!(
                "[{}/{}] Processing: {}",
                idx + 1,
                pdf_files.len(),
                pdf_path.display()
            );
        }

        // === 新実行経路: PipelineRunner + Stage trait ===
        let page_start = Instant::now();

        // ページ数取得（pdfinfo 利用; 失敗時はエラー扱い）
        let total_pages = match probe_pdf_page_count(pdf_path) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("Error getting page count for {}: {}", pdf_path.display(), e);
                error_count += 1;
                continue;
            }
        };
        let actual_pages = args.max_pages.map(|m| m.min(total_pages)).unwrap_or(total_pages);

        if verbose {
            println!("  Pages: {}", actual_pages);
        }

        // ファイルごとの作業ディレクトリ
        let pdf_stem = pdf_path.file_stem().unwrap_or_default().to_string_lossy();
        let file_work_base = match &work_dir {
            Some(wd) => wd.join(pdf_stem.as_ref()),
            None => output_dir.join("_work").join(pdf_stem.as_ref()),
        };
        std::fs::create_dir_all(&file_work_base)?;

        let verbose_level = args.verbose;
        let queue_cfg = GpuQueueConfig {
            max_in_flight: 1,
            safety_margin_mb: 3000,
            status_poll_ms: 100,
        };
        let upscale_url = std::env::var("REALESRGAN_API_URL")
            .unwrap_or_else(|_| "http://localhost:8000".to_string());
        let ocr_url = std::env::var("YOMITOKU_API_URL")
            .unwrap_or_else(|_| "http://localhost:8000".to_string());
        let upscale_queue = GpuJobQueue::new("upscale", upscale_url, queue_cfg.clone());
        let ocr_queue = GpuJobQueue::new("ocr", ocr_url, queue_cfg);
        let runner = PipelineRunner::new(PipelineRunnerConfig {
            max_parallel_pages: args.thread_count(),
            cpu_min_parallel_pages: 1,
            cpu_target_load_per_core: 0.9,
            cpu_status_poll_ms: 200,
            work_base_dir: file_work_base.clone(),
            retry: RetryConfig::default(),
        })
        .add_stage(LoadStage::new(pdf_path.clone(), args.dpi))
        .add_stage(DeskewStage::new(args.effective_deskew(), 0.8))
        .add_stage(ColorStage::new(args.effective_color_correction()))
        .add_stage(UpscaleStage::new(
            2,
            "realesrgan-x4plus".to_string(),
            args.effective_upscale(),
            256,
            false,
            upscale_queue,
        ))
        .add_stage(MarginStage::new(args.margin_trim > 0.0))
        .add_stage(NormalizeStage::new(args.effective_internal_resolution()))
        .add_stage(PageNumberStage::new(true))
        .add_stage(OcrStage::new(
            args.ocr,
            "jpn".to_string(),
            0.5,
            "json".to_string(),
            ocr_queue,
        ))
        .add_stage(SaveStage::new(
            output_dir.clone(),
            args.output_height,
            args.jpeg_quality,
        ))
        .add_stage(ValidationStage::new(false, 0))
        .add_stage(CleanupStage::new(false)); // PDF組み立て後に手動削除

        let page_results = runner
            .run_all(
                actual_pages,
                Some(std::sync::Arc::new(move |event: ProgressEvent| {
                    if verbose_level > 0 {
                        println!(
                            "  [{}/{}] Page {} - {}",
                            event.completed, event.total, event.page_id, event.stage
                        );
                    }
                })),
            )
            .await;

        let ok_pages = page_results.iter().filter(|r| r.success).count();
        let fail_pages = page_results.len() - ok_pages;

        if fail_pages > 0 {
            eprintln!(
                "Error: {} pages failed in {}",
                fail_pages,
                pdf_path.display()
            );
            error_count += 1;
            std::fs::remove_dir_all(&file_work_base).ok();
            continue;
        }

        // 処理済みページ画像を順番に収集
        let page_images: Vec<PathBuf> = (0..actual_pages)
            .map(|i| file_work_base.join(format!("{:04}", i)).join("gaozou.webp"))
            .filter(|p| p.exists())
            .collect();

        if page_images.is_empty() {
            eprintln!(
                "Error: No page images found after processing {}",
                pdf_path.display()
            );
            error_count += 1;
            std::fs::remove_dir_all(&file_work_base).ok();
            continue;
        }

        // 最終 PDF を組み立て
        let pdf_opts = PdfWriterOptions::builder()
            .dpi(args.dpi)
            .jpeg_quality(args.jpeg_quality)
            .build();
        match PrintPdfWriter::create_from_images(&page_images, &output_pdf, &pdf_opts) {
            Ok(()) => {
                let elapsed_s = page_start.elapsed().as_secs_f64();
                let output_size = std::fs::metadata(&output_pdf)
                    .map(|m| m.len())
                    .unwrap_or(0);
                ok_count += 1;
                // 処理成功後のキャッシュ保存 [3]
                if let Ok(digest) = CacheDigest::new(pdf_path, &options_json) {
                    let cache_result = ProcessingResult::new(
                        actual_pages,
                        None,
                        false,
                        elapsed_s,
                        output_size,
                    );
                    let cache = ProcessingCache::new(digest, cache_result);
                    let _ = cache.save(&output_pdf);
                }
                if verbose {
                    println!(
                        " Completed: {} pages, {:.2}s, {} bytes",
                        actual_pages, elapsed_s, output_size
                    );
                }
            }
            Err(e) => {
                eprintln!("Error assembling PDF for {}: {}", pdf_path.display(), e);
                error_count += 1;
            }
        }

        // 作業ディレクトリを削除
        std::fs::remove_dir_all(&file_work_base).ok();
    }

    let elapsed = start_time.elapsed();

    // 処理結果のサマリー表示 [3]
    if !args.quiet {
        ProgressTracker::print_summary(pdf_files.len(), ok_count, skip_count, error_count);
        println!("Total time: {:.2}s", elapsed.as_secs_f64());
    }

    if error_count > 0 {
        return Err(format!("{} file(s) failed to process", error_count).into());
    }

    Ok(())
}

// ============ Markdown Command ============

async fn run_markdown(args: &MarkdownArgs) -> Result<(), Box<dyn std::error::Error>> {
    let start_time = Instant::now();
    if !args.input.exists() {
        eprintln!("Error: Input path does not exist: {}", args.input.display());
        std::process::exit(exit_codes::INPUT_NOT_FOUND);
    }

    if !args.input.is_file() {
        eprintln!("Error: Input must be a PDF file: {}", args.input.display());
        std::process::exit(exit_codes::INVALID_ARGS);
    }

    let verbose = args.verbose > 0;
    if verbose {
        println!("Markdown変換開始: {}", args.input.display());
        println!("出力先: {}", args.output.display());
        if args.resume {
            println!("リカバリーモード: 有効");
        }
    }

    std::fs::create_dir_all(&args.output)?;

    let total_pages = probe_pdf_page_count(&args.input)?;
    let page_count = args.max_pages.map(|m| m.min(total_pages)).unwrap_or(total_pages);
    let work_base = args.output.join("_work_markdown");
    std::fs::create_dir_all(&work_base)?;

    let title = args
        .input
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let runner = PipelineRunner::new(PipelineRunnerConfig {
        // markdown_merge を安全に最終ページで実行するため逐次実行
        max_parallel_pages: 1,
        cpu_min_parallel_pages: 1,
        cpu_target_load_per_core: 0.9,
        cpu_status_poll_ms: 200,
        work_base_dir: work_base.clone(),
        retry: RetryConfig::default(),
    });
    let queue_cfg = GpuQueueConfig {
        max_in_flight: 1,
        safety_margin_mb: 3000,
        status_poll_ms: 100,
    };
    let upscale_url = std::env::var("REALESRGAN_API_URL")
        .unwrap_or_else(|_| "http://localhost:8000".to_string());
    let ocr_url = std::env::var("YOMITOKU_API_URL")
        .unwrap_or_else(|_| "http://localhost:8000".to_string());
    let upscale_queue = GpuJobQueue::new("upscale", upscale_url, queue_cfg.clone());
    let ocr_queue = GpuJobQueue::new("ocr", ocr_url, queue_cfg);
    let runner = runner
    .add_stage(LoadStage::new(args.input.clone(), args.dpi))
    .add_stage(DeskewStage::new(args.effective_deskew(), 0.8))
    .add_stage(UpscaleStage::new(
        2,
        "realesrgan-x4plus",
        args.upscale,
        256,
        false,
        upscale_queue,
    ))
    .add_stage(PageNumberStage::new(args.include_page_numbers))
    .add_stage(OcrStage::new(true, "jpn", 0.5, "json", ocr_queue))
    .add_stage(MarkdownStage::new(
        args.output.clone(),
        args.include_page_numbers,
    ))
    .add_stage(MarkdownMergeStage::new(
        args.output.clone(),
        title,
        page_count,
    ))
    .add_stage(CleanupStage::new(false));

    let verbose_level = args.verbose;
    let page_results = runner
        .run_all(
            page_count,
            Some(std::sync::Arc::new(move |event: ProgressEvent| {
                if verbose_level > 0 {
                    println!(
                        "  [{}/{}] Page {} - {}",
                        event.completed, event.total, event.page_id, event.stage
                    );
                }
            })),
        )
        .await;

    let fail_pages = page_results.iter().filter(|r| !r.success).count();
    if fail_pages > 0 {
        return Err(format!("Markdown conversion failed on {} pages", fail_pages).into());
    }

    if !args.quiet {
        let elapsed = start_time.elapsed();
        println!();
        println!("=== Markdown変換完了 ===");
        println!("ページ数: {}", page_count);
        println!("出力:     {}", args.output.display());
        println!("処理時間: {:.2}s", elapsed.as_secs_f64());
    }

    Ok(())
}

// ============ Helper Functions ============

/// Create CLI overrides from ConvertArgs
///
/// Only override config file values when CLI explicitly sets a non-default value.
/// This allows config files to provide defaults that aren't overridden by clap defaults.
fn create_cli_overrides(args: &ConvertArgs) -> CliOverrides {
    let mut overrides = CliOverrides::new();

    // CLI defaults - only override if user explicitly changed these
    const DEFAULT_DPI: u32 = 300;
    const DEFAULT_MARGIN_TRIM: f32 = 0.5;
    const DEFAULT_OUTPUT_HEIGHT: u32 = 3508;
    const DEFAULT_JPEG_QUALITY: u8 = 90;

    // Basic options - only set if they differ from defaults
    if args.dpi != DEFAULT_DPI {
        overrides.dpi = Some(args.dpi);
    }

    // Deskew: override if --no-deskew was used
    if !args.effective_deskew() {
        overrides.deskew = Some(false);
    }

    // Margin trim: override if changed from default
    if (args.margin_trim - DEFAULT_MARGIN_TRIM).abs() > f32::EPSILON {
        overrides.margin_trim = Some(args.margin_trim as f64);
    }

    // Upscale: override if --no-upscale was used
    if !args.effective_upscale() {
        overrides.upscale = Some(false);
    }

    // GPU: override if --no-gpu was used
    if !args.effective_gpu() {
        overrides.gpu = Some(false);
    }

    // OCR: override if explicitly enabled
    if args.ocr {
        overrides.ocr = Some(true);
    }

    // Threads: only set if explicitly provided
    overrides.threads = args.threads;

    // Advanced options - only set if explicitly enabled
    if args.internal_resolution || args.advanced {
        overrides.internal_resolution = Some(true);
    }
    if args.color_correction || args.advanced {
        overrides.color_correction = Some(true);
    }

    // Output height: only set if changed from default
    if args.output_height != DEFAULT_OUTPUT_HEIGHT {
        overrides.output_height = Some(args.output_height);
    }

    // JPEG quality: only set if changed from default
    if args.jpeg_quality != DEFAULT_JPEG_QUALITY {
        overrides.jpeg_quality = Some(args.jpeg_quality);
    }

    // Debug options
    overrides.max_pages = args.max_pages;
    if args.save_debug {
        overrides.save_debug = Some(true);
    }

    overrides
}

/// Collect PDF files from input path (file or directory)
fn collect_pdf_files(input: &PathBuf) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut pdf_files = Vec::new();

    if input.is_file() {
        if input.extension().is_some_and(|ext| ext == "pdf") {
            pdf_files.push(input.clone());
        }
    } else if input.is_dir() {
        for entry in std::fs::read_dir(input)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "pdf") {
                pdf_files.push(path);
            }
        }
        pdf_files.sort();
    }

    Ok(pdf_files)
}

fn output_path_from_input(input: &std::path::Path, output_dir: &std::path::Path) -> PathBuf {
    let pdf_name = input.file_stem().unwrap_or_default().to_string_lossy();
    output_dir.join(format!("{}_superbook.pdf", pdf_name))
}

/// Print execution plan for dry-run mode
fn print_execution_plan(
    args: &ConvertArgs,
    pdf_files: &[PathBuf],
    config: &superbook_pdf::PipelineConfig,
) {
    println!("=== Dry Run - Execution Plan ===");
    println!();
    println!("Input: {}", args.input.display());
    println!(
        "Output: {}",
        args.output
            .as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(from env/default)".to_string())
    );
    println!("Files to process: {}", pdf_files.len());
    println!();
    println!("Pipeline Configuration:");
    println!("  1. Image Extraction (DPI: {})", config.dpi);
    if config.deskew {
        println!("  2. Deskew Correction: ENABLED");
    } else {
        println!("  2. Deskew Correction: DISABLED");
    }
    println!("  3. Margin Trim: {}%", config.margin_trim);
    if config.upscale {
        println!("  4. AI Upscaling (RealESRGAN 2x): ENABLED");
    } else {
        println!("  4. AI Upscaling: DISABLED");
    }
    if config.ocr {
        println!("  5. OCR (YomiToku): ENABLED");
    } else {
        println!("  5. OCR: DISABLED");
    }
    if config.internal_resolution {
        println!("  6. Internal Resolution Normalization (4960x7016): ENABLED");
    }
    if config.color_correction {
        println!("  7. Global Color Correction: ENABLED");
    }
    println!(
        "  8. PDF Generation (output height: {})",
        config.output_height
    );
    println!();
    println!("Processing Options:");
    println!(
        "  Threads: {}",
        config.threads.unwrap_or_else(num_cpus::get)
    );
    if args.chunk_size > 0 {
        println!("  Chunk size: {} pages", args.chunk_size);
    } else {
        println!("  Chunk size: unlimited (all pages at once)");
    }
    println!("  GPU: {}", if config.gpu { "YES" } else { "NO" });
    println!(
        "  Skip existing: {}",
        if args.skip_existing { "YES" } else { "NO" }
    );
    println!(
        "  Force re-process: {}",
        if args.force { "YES" } else { "NO" }
    );
    println!("  Verbose: {}", args.verbose);
    println!();
    println!("Debug Options:");
    if let Some(max) = config.max_pages {
        println!("  Max pages: {}", max);
    } else {
        println!("  Max pages: unlimited");
    }
    println!(
        "  Save debug images: {}",
        if config.save_debug { "YES" } else { "NO" }
    );
    println!();
    println!("Files:");
    for (i, file) in pdf_files.iter().enumerate() {
        println!("  {}. {}", i + 1, file.display());
    }
}

// ============ Info Command ============

fn run_info() -> Result<(), Box<dyn std::error::Error>> {
    println!("superbook-pdf v{}", env!("CARGO_PKG_VERSION"));
    println!();

    // System Information
    println!("System Information:");
    println!("  Platform: {}", std::env::consts::OS);
    println!("  Arch: {}", std::env::consts::ARCH);
    println!("  CPUs: {}", num_cpus::get());

    // Memory info (Linux)
    if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
        if let Some(line) = meminfo.lines().find(|l| l.starts_with("MemTotal:")) {
            if let Some(kb) = line.split_whitespace().nth(1) {
                if let Ok(kb_val) = kb.parse::<u64>() {
                    println!("  Memory: {:.1} GB", kb_val as f64 / 1_048_576.0);
                }
            }
        }
    }

    // External Tools
    println!();
    println!("PDF Extraction Tools:");
    check_tool_with_version("pdftoppm", "Poppler", &["-v"]);
    check_tool_with_version("pdfinfo", "Poppler pdfinfo", &["-v"]);
    check_imagemagick_tool();
    check_imagemagick_identify_tool();
    check_tool_with_version("gs", "Ghostscript", &["--version"]);
    check_tool_with_version("qpdf", "QPDF", &["--version"]);

    println!();
    println!("Runtime Tools:");
    check_python_tool();

    println!();
    println!("Page Number OCR (Optional):");
    check_tool_with_version("tesseract", "Tesseract", &["--version"]);

    // AI Services (HTTP microservices)
    println!();
    println!("AI Services:");
    let realesrgan_url = std::env::var("REALESRGAN_API_URL")
        .unwrap_or_else(|_| "http://realesrgan-api:8000".to_string());
    let yomitoku_url = std::env::var("YOMITOKU_API_URL")
        .unwrap_or_else(|_| "http://yomitoku-api:8000".to_string());
    let realesrgan_info = check_ai_service("RealESRGAN", &realesrgan_url);
    let yomitoku_info = check_ai_service("YomiToku", &yomitoku_url);

    // GPU Status (via nvidia-smi if available)
    println!();
    println!("GPU Status:");
    if let Ok(output) = std::process::Command::new("nvidia-smi")
        .arg("--query-gpu=name,memory.total,driver_version")
        .arg("--format=csv,noheader")
        .output()
    {
        if output.status.success() {
            let gpu_info = String::from_utf8_lossy(&output.stdout);
            for line in gpu_info.trim().lines() {
                println!("  NVIDIA: {}", line.trim());
            }
        } else {
            println!("  NVIDIA GPU: Not detected");
        }
    } else {
        println!("  NVIDIA GPU: nvidia-smi not found (using AI service metadata)");
        if let Some(info) = realesrgan_info.as_ref().or(yomitoku_info.as_ref()) {
            if let Some(device) = &info.device {
                println!("  Device (from {}): {}", info.name, device);
            }
            if let Some(vram) = info.vram_total_gb {
                println!("  VRAM (from {}): {:.2} GB", info.name, vram);
            }
            if let Some(cuda) = info.cuda_available {
                println!(
                    "  CUDA (from {}): {}",
                    info.name,
                    if cuda { "✅" } else { "❌" }
                );
            }
        }
    }

    // Config File Locations
    println!();
    println!("Config File Locations:");
    println!("  Local: ./superbook.toml");
    if let Some(config_dir) = dirs::config_dir() {
        println!(
            "  User:  {}",
            config_dir.join("superbook-pdf/config.toml").display()
        );
    }

    Ok(())
}

struct AiServiceInfo {
    name: String,
    cuda_available: Option<bool>,
    device: Option<String>,
    vram_total_gb: Option<f64>,
}

fn check_ai_service(name: &str, url: &str) -> Option<AiServiceInfo> {
    let version_url = format!("{}/version", url.trim_end_matches('/'));
    match ureq::get(&version_url)
        .timeout(std::time::Duration::from_secs(5))
        .call()
    {
        Ok(response) => {
            if let Ok(body) = response.into_string() {
                // Parse key fields from JSON body
                let service_version = extract_json_str(&body, "service_version");
                let torch = extract_json_str(&body, "torch_version");
                let cuda = extract_json_bool(&body, "cuda_available");
                let device = extract_json_str(&body, "device");
                let vram_total_gb = extract_json_f64(&body, "vram_total_gb");
                let python_version = extract_json_str(&body, "python_version");
                println!("  {}: Available (url={})", name, url);
                if let Some(v) = service_version {
                    println!("    Version: {}", v);
                }
                if let Some(p) = python_version.as_ref() {
                    println!("    Python: {}", p);
                }
                if let Some(t) = torch {
                    println!("    Torch: {}", t);
                }
                if let Some(c) = cuda {
                    println!("    CUDA: {}", if c { "✅" } else { "❌" });
                }
                if let Some(ref d) = device {
                    println!("    Device: {}", d);
                }
                if let Some(v) = vram_total_gb {
                    println!("    VRAM: {:.2} GB", v);
                }
                return Some(AiServiceInfo {
                    name: name.to_string(),
                    cuda_available: cuda,
                    device,
                    vram_total_gb,
                });
            } else {
                println!("  {}: Available (url={})", name, url);
            }
        }
        Err(e) => {
            println!("  {}: Unavailable (url={}, error={})", name, url, e);
        }
    }
    None
}

fn extract_json_str(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\":", key);
    let pos = json.find(&pattern)?;
    let rest = json[pos + pattern.len()..].trim_start();
    if let Some(inner) = rest.strip_prefix('"') {
        let end = inner.find('"')?;
        Some(inner[..end].to_string())
    } else {
        None
    }
}

fn extract_json_bool(json: &str, key: &str) -> Option<bool> {
    let pattern = format!("\"{}\":", key);
    let pos = json.find(&pattern)?;
    let rest = json[pos + pattern.len()..].trim_start();
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn extract_json_f64(json: &str, key: &str) -> Option<f64> {
    let pattern = format!("\"{}\":", key);
    let pos = json.find(&pattern)?;
    let rest = json[pos + pattern.len()..].trim_start();
    let end = rest
        .find(|c: char| c == ',' || c == '}' || c.is_whitespace())
        .unwrap_or(rest.len());
    rest[..end].trim().parse::<f64>().ok()
}

fn check_imagemagick_tool() {
    // Debian/Ubuntu images often provide ImageMagick as `convert` without `magick`.
    if which::which("magick").is_ok() {
        check_tool_with_version("magick", "ImageMagick", &["--version"]);
    } else {
        check_tool_with_version("convert", "ImageMagick", &["--version"]);
    }
}

fn check_imagemagick_identify_tool() {
    if which::which("identify").is_ok() {
        check_tool_with_version("identify", "ImageMagick Identify", &["-version"]);
    } else if which::which("magick").is_ok() {
        check_tool_with_version("magick", "ImageMagick Identify", &["identify", "-version"]);
    } else {
        println!("  ImageMagick Identify: Not found");
    }
}

fn check_python_tool() {
    if which::which("python3").is_ok() {
        check_tool_with_version("python3", "Python (Core)", &["--version"]);
    } else if which::which("python").is_ok() {
        check_tool_with_version("python", "Python (Core)", &["--version"]);
    } else {
        println!(
            "  Python (Core): Not found (expected: Python is provided by AI service containers)"
        );
    }
}

fn check_tool_with_version(cmd: &str, name: &str, version_args: &[&str]) {
    match which::which(cmd) {
        Ok(path) => {
            // Try to get version
            if let Ok(output) = std::process::Command::new(&path)
                .args(version_args)
                .output()
            {
                // Some tools print version to stderr (e.g. pdftoppm -v), so check both.
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let version_line = stdout
                    .lines()
                    .chain(stderr.lines())
                    .map(str::trim)
                    .find(|line| !line.is_empty())
                    .unwrap_or("");

                if !version_line.is_empty() && version_line.len() < 120 {
                    println!("  {}: {} ({})", name, version_line, path.display());
                } else {
                    println!("  {}: {} (found)", name, path.display());
                }
            } else {
                println!("  {}: {} (found)", name, path.display());
            }
        }
        Err(_) => println!("  {}: Not found", name),
    }
}

// ============ Cache Info Command ============

fn run_cache_info(args: &CacheInfoArgs) -> Result<(), Box<dyn std::error::Error>> {
    use chrono::{DateTime, Local, TimeZone};

    let output_path = &args.output_pdf;

    if !output_path.exists() {
        return Err(format!("Output file not found: {}", output_path.display()).into());
    }

    match ProcessingCache::load(output_path) {
        Ok(cache) => {
            println!("=== Cache Information ===");
            println!();
            println!("Output file: {}", output_path.display());
            println!(
                "Cache file:  {}",
                ProcessingCache::cache_path(output_path).display()
            );
            println!();
            println!("Cache Version: {}", cache.version);
            let processed_dt: DateTime<Local> = Local
                .timestamp_opt(cache.processed_at as i64, 0)
                .single()
                .unwrap_or_else(Local::now);
            println!(
                "Processed at:  {}",
                processed_dt.format("%Y-%m-%d %H:%M:%S")
            );
            println!();
            println!("Source Digest:");
            println!("  Modified: {}", cache.digest.source_modified);
            println!("  Size:     {} bytes", cache.digest.source_size);
            println!("  Options:  {}", cache.digest.options_hash);
            println!();
            println!("Processing Result:");
            println!("  Page count:  {}", cache.result.page_count);
            println!(
                "  Page shift:  {}",
                cache
                    .result
                    .page_number_shift
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "none".to_string())
            );
            println!(
                "  Vertical:    {}",
                if cache.result.is_vertical {
                    "yes"
                } else {
                    "no"
                }
            );
            println!("  Elapsed:     {:.2}s", cache.result.elapsed_seconds);
            println!(
                "  Output size: {} bytes ({:.2} MB)",
                cache.result.output_size,
                cache.result.output_size as f64 / 1_048_576.0
            );
        }
        Err(e) => {
            println!("No cache found for: {}", output_path.display());
            println!(
                "Cache file would be: {}",
                ProcessingCache::cache_path(output_path).display()
            );
            println!();
            println!("Reason: {}", e);
        }
    }

    Ok(())
}

// ============ Reprocess Command ============

fn run_reprocess(args: &ReprocessArgs) -> Result<(), Box<dyn std::error::Error>> {
    let start_time = Instant::now();
    let verbose = args.verbose > 0;

    // Determine if input is a state file or PDF
    let state_path = if args.is_state_file() {
        args.input.clone()
    } else {
        // For PDF input, look for state file in output directory
        let output_dir = args.output.clone().unwrap_or_else(|| {
            args.input
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join("output")
        });
        output_dir.join(".superbook-state.json")
    };

    // Load or create state
    let mut state = if state_path.exists() {
        ReprocessState::load(&state_path)?
    } else {
        if args.is_state_file() {
            return Err(format!("State file not found: {}", state_path.display()).into());
        }
        // No existing state - need to run initial processing first
        return Err("No processing state found. Please run 'convert' command first to create initial state.".into());
    };

    // Status-only mode
    if args.status {
        print_reprocess_status(&state);
        return Ok(());
    }

    // Get failed pages to reprocess
    let failed_pages = if !args.page_indices().is_empty() {
        args.page_indices()
    } else {
        state.failed_pages()
    };

    if failed_pages.is_empty() {
        println!("No failed pages to reprocess.");
        println!("Completion: {:.1}%", state.completion_percent());
        return Ok(());
    }

    if verbose {
        println!("Reprocessing {} failed page(s)...", failed_pages.len());
        println!("Pages: {:?}", failed_pages);
    }

    // Create reprocess options
    let options = ReprocessOptions {
        max_retries: args.max_retries,
        page_indices: args.page_indices(),
        force: args.force,
        keep_intermediates: args.keep_intermediates,
    };

    // Track results
    let success_count = 0usize;
    let mut still_failed = 0usize;

    // Process each failed page
    for &page_idx in &failed_pages {
        if page_idx >= state.pages.len() {
            eprintln!(
                "Warning: Page index {} out of range (total: {})",
                page_idx,
                state.pages.len()
            );
            continue;
        }

        // Check retry count
        if let PageStatus::Failed { retry_count, .. } = &state.pages[page_idx] {
            if *retry_count >= options.max_retries && !args.force {
                if verbose {
                    println!(
                        "  Page {}: Skipped (max retries {} exceeded)",
                        page_idx, options.max_retries
                    );
                }
                still_failed += 1;
                continue;
            }
        }

        if verbose {
            println!("  Processing page {}...", page_idx);
        }

        // Note: Actual reprocessing would require pipeline integration
        // For now, we increment retry count and leave as failed
        // This is a placeholder for full pipeline integration
        if let PageStatus::Failed { error, retry_count } = &state.pages[page_idx] {
            state.pages[page_idx] = PageStatus::Failed {
                error: error.clone(),
                retry_count: retry_count + 1,
            };
            still_failed += 1;

            // In a full implementation, we would:
            // 1. Load the pipeline with same config
            // 2. Re-extract the specific page
            // 3. Run through deskew, margin, upscale, etc.
            // 4. Update state to Success or Failed
        }
    }

    // Update state timestamps
    state.updated_at = chrono::Utc::now().to_rfc3339();

    // Save state
    state.save(&state_path)?;

    let elapsed = start_time.elapsed();

    // Print summary
    if !args.quiet {
        println!();
        println!("=== Reprocess Summary ===");
        println!("Total pages:      {}", state.pages.len());
        println!("Reprocessed:      {}", failed_pages.len());
        println!("Now successful:   {}", success_count);
        println!("Still failing:    {}", still_failed);
        println!("Completion:       {:.1}%", state.completion_percent());
        println!("Time elapsed:     {:.2}s", elapsed.as_secs_f64());

        if state.is_complete() {
            println!();
            println!("All pages processed successfully!");
        } else {
            let remaining_failed = state.failed_pages();
            if !remaining_failed.is_empty() {
                println!();
                println!("Remaining failed pages: {:?}", remaining_failed);
            }
        }
    }

    Ok(())
}

fn print_reprocess_status(state: &ReprocessState) {
    println!("=== Reprocess Status ===");
    println!();
    println!("Source PDF:   {}", state.source_pdf.display());
    println!("Output dir:   {}", state.output_dir.display());
    println!("Config hash:  {}", state.config_hash);
    println!("Created:      {}", state.created_at);
    println!("Updated:      {}", state.updated_at);
    println!();
    println!("Pages: {} total", state.pages.len());

    let success_count = state.success_pages().len();
    let failed_pages = state.failed_pages();
    let pending_count = state
        .pages
        .iter()
        .filter(|p| matches!(p, PageStatus::Pending))
        .count();

    println!("  Success: {}", success_count);
    println!("  Failed:  {}", failed_pages.len());
    println!("  Pending: {}", pending_count);
    println!();
    println!("Completion: {:.1}%", state.completion_percent());

    if !failed_pages.is_empty() {
        println!();
        println!("Failed pages:");
        for idx in &failed_pages {
            if let PageStatus::Failed { error, retry_count } = &state.pages[*idx] {
                println!("  Page {}: {} (retries: {})", idx, error, retry_count);
            }
        }
    }
}

// ============ Serve Command (Web Server) ============

#[cfg(feature = "web")]
async fn run_serve(args: &ServeArgs) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = ServerConfig::default()
        .with_port(args.port)
        .with_bind(&args.bind)
        .with_upload_limit(args.upload_limit * 1024 * 1024);

    // WorkerPool の並列数は pipeline.toml の concurrency.job_parallel を正とする。
    // 設定不在時は PipelineTomlConfig のデフォルト値にフォールバックする。
    let toml_config = PipelineTomlConfig::load().unwrap_or_default();
    config.workers = toml_config.resolved_job_parallel();

    // Configure CORS
    if args.no_cors {
        config = config.with_cors_disabled();
    } else if !args.cors_origins.is_empty() {
        config = config.with_cors_origins(args.cors_origins.clone());
    }
    // Default: permissive CORS is already set in ServerConfig::default()

    // Already inside a tokio runtime thanks to #[tokio::main]
    let server = WebServer::with_config(config);
    server.run().await.map_err(|e| e.to_string())?;

    Ok(())
}

// ============ Unit Tests ============

#[cfg(test)]
mod tests {
    use super::VerboseProgress;
    use superbook_pdf::{ProgressCallback, SilentProgress};

    // TC-CLI-OUTPUT-001: DEBUG messages should only appear at verbose level 3 (-vvv)
    #[test]
    fn test_debug_messages_require_level_3() {
        // Level 0: no debug
        let handler_0 = VerboseProgress::new(0);
        assert!(!handler_0.should_show_debug());

        // Level 1 (-v): no debug
        let handler_1 = VerboseProgress::new(1);
        assert!(!handler_1.should_show_debug());

        // Level 2 (-vv): no debug
        let handler_2 = VerboseProgress::new(2);
        assert!(!handler_2.should_show_debug());

        // Level 3 (-vvv): should show debug
        let handler_3 = VerboseProgress::new(3);
        assert!(handler_3.should_show_debug());
    }

    // TC-CLI-OUTPUT-002: Verbose level thresholds
    #[test]
    fn test_verbose_level_thresholds() {
        // Level 0: no output
        let handler_0 = VerboseProgress::new(0);
        assert!(!handler_0.should_show_steps());
        assert!(!handler_0.should_show_progress());
        assert!(!handler_0.should_show_debug());

        // Level 1 (-v): basic progress
        let handler_1 = VerboseProgress::new(1);
        assert!(handler_1.should_show_steps());
        assert!(handler_1.should_show_progress());
        assert!(!handler_1.should_show_debug());

        // Level 2 (-vv): detailed info
        let handler_2 = VerboseProgress::new(2);
        assert!(handler_2.should_show_steps());
        assert!(handler_2.should_show_progress());
        assert!(!handler_2.should_show_debug());

        // Level 3 (-vvv): debug info
        let handler_3 = VerboseProgress::new(3);
        assert!(handler_3.should_show_steps());
        assert!(handler_3.should_show_progress());
        assert!(handler_3.should_show_debug());
    }

    // TC-CLI-OUTPUT-003: on_warning always prints regardless of verbose level
    #[test]
    fn test_on_warning_always_visible() {
        // on_warning should work at all verbose levels (even 0)
        // We verify it doesn't panic and the impl doesn't gate on verbose_level
        let handler_0 = VerboseProgress::new(0);
        handler_0.on_warning("test warning at level 0");

        let handler_1 = VerboseProgress::new(1);
        handler_1.on_warning("test warning at level 1");

        let handler_3 = VerboseProgress::new(3);
        handler_3.on_warning("test warning at level 3");
    }

    // TC-CLI-OUTPUT-004: SilentProgress on_warning is a no-op
    #[test]
    fn test_silent_progress_on_warning() {
        let silent = SilentProgress;
        // Should not panic or produce output
        silent.on_warning("test warning");
        silent.on_debug("test debug");
    }

    // TC-CLI-OUTPUT-005: Default ProgressCallback on_warning implementation
    #[test]
    fn test_default_progress_callback_on_warning() {
        struct MinimalProgress;
        impl ProgressCallback for MinimalProgress {
            fn on_step_start(&self, _step: &str) {}
            fn on_step_progress(&self, _current: usize, _total: usize) {}
            fn on_step_complete(&self, _step: &str, _message: &str) {}
            fn on_debug(&self, _message: &str) {}
        }
        // Default on_warning should print to stderr — verify it doesn't panic
        let progress = MinimalProgress;
        progress.on_warning("test default warning");
    }
}

// ============ New Pipeline Commands (Stage-based architecture) ============

/// `run` command handler: Build PipelineRunner and process PDF
async fn run_pipeline(args: &RunArgs) -> Result<(), Box<dyn std::error::Error>> {
    // Load pipeline.toml config
    let toml_config = match &args.config {
        Some(path) => PipelineTomlConfig::load_from_path(path)?,
        None => PipelineTomlConfig::load().unwrap_or_default(),
    };

    if args.dry_run {
        println!("=== Dry Run ===");
        println!("Input: {}", args.input.display());
        println!("Config: {:?}", args.config);
        println!("{}", toml_config.to_toml()?);
        return Ok(());
    }

    if !args.input.exists() {
        return Err(format!("Input not found: {}", args.input.display()).into());
    }

    let output_dir = args.output.clone().unwrap_or_else(|| PathBuf::from("./output"));
    std::fs::create_dir_all(&output_dir)?;

    let work_base = args.work_dir.clone().unwrap_or_else(|| PathBuf::from("./data/work"));
    std::fs::create_dir_all(&work_base)?;

    // Get page count via pdftoppm probe
    let page_count = probe_pdf_page_count(&args.input)?;
    if args.verbose > 0 {
        println!("PDF pages: {}", page_count);
    }

    let max_parallel = if args.parallel > 0 {
        args.parallel
    } else {
        toml_config.resolved_page_parallel()
    };

    let runner = build_standard_pipeline_runner(
        PipelineRunnerConfig {
        max_parallel_pages: max_parallel,
        cpu_min_parallel_pages: toml_config.resolved_cpu_dynamic_min_parallel(),
        cpu_target_load_per_core: toml_config.resolved_cpu_target_load_per_core(),
        cpu_status_poll_ms: toml_config.resolved_cpu_status_poll_ms(),
        work_base_dir: work_base.clone(),
        retry: RetryConfig {
            max_attempts: toml_config.retry.max_attempts,
            backoff_ms: toml_config.retry.backoff_ms,
        },
        },
        args.input.clone(),
        output_dir.clone(),
        &toml_config,
    );

    let verbose = args.verbose;
    let results = runner
        .run_all(
            page_count,
            Some(std::sync::Arc::new(move |event| {
                if verbose > 0 {
                    println!("[Progress] {:?}", event);
                }
            })),
        )
        .await;

    let ok = results.iter().filter(|r| r.success).count();
    let fail = results.len() - ok;
    println!("Completed: {}/{} pages ({} failed)", ok, results.len(), fail);

    if fail > 0 {
        return Err(format!("{} pages failed", fail).into());
    }

    if toml_config.cleanup.enable {
        if let Err(e) = std::fs::remove_dir_all(&work_base) {
            eprintln!("Warning: failed to cleanup work directory {}: {}", work_base.display(), e);
        }
    }

    Ok(())
}

/// `list-config` command handler: Show pipeline.toml settings
fn run_list_config(args: &ListConfigArgs) -> Result<(), Box<dyn std::error::Error>> {
    let config = match &args.config {
        Some(path) => PipelineTomlConfig::load_from_path(path)?,
        None => PipelineTomlConfig::load().unwrap_or_default(),
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&config)?);
    } else {
        println!("{}", config.to_toml()?);
    }
    Ok(())
}

/// `apply-config` command handler: Copy a preset config to pipeline.toml
fn run_apply_config(args: &ApplyConfigArgs) -> Result<(), Box<dyn std::error::Error>> {
    if !args.preset.exists() {
        return Err(format!("Preset not found: {}", args.preset.display()).into());
    }
    if args.output.exists() && !args.force {
        return Err(format!(
            "Output already exists: {}. Use --force to overwrite.",
            args.output.display()
        ).into());
    }
    std::fs::copy(&args.preset, &args.output)?;
    println!("Applied: {} -> {}", args.preset.display(), args.output.display());
    Ok(())
}

/// `preview` command handler: Process a single page and show result
async fn run_preview(args: &PreviewArgs) -> Result<(), Box<dyn std::error::Error>> {
    if !args.input.exists() {
        return Err(format!("Input not found: {}", args.input.display()).into());
    }

    let toml_config = match &args.config {
        Some(path) => PipelineTomlConfig::load_from_path(path)?,
        None => PipelineTomlConfig::load().unwrap_or_default(),
    };

    let output_dir = args.output.clone().unwrap_or_else(|| PathBuf::from("./preview_out"));
    std::fs::create_dir_all(&output_dir)?;

    let work_base = PathBuf::from("./data/work_preview");
    std::fs::create_dir_all(&work_base)?;

    println!("Previewing page {} of {}", args.page, args.input.display());

    let queue_cfg = GpuQueueConfig {
        max_in_flight: toml_config.resolved_gpu_stage_parallel(),
        safety_margin_mb: toml_config.resolved_gpu_safety_margin_mb(),
        status_poll_ms: toml_config.resolved_gpu_status_poll_ms(),
    };
    let upscale_url = std::env::var("REALESRGAN_API_URL")
        .unwrap_or_else(|_| "http://localhost:8000".to_string());
    let upscale_queue = GpuJobQueue::new("upscale", upscale_url, queue_cfg);

    let runner = PipelineRunner::new(PipelineRunnerConfig {
        max_parallel_pages: 1,
        cpu_min_parallel_pages: 1,
        cpu_target_load_per_core: toml_config.resolved_cpu_target_load_per_core(),
        cpu_status_poll_ms: toml_config.resolved_cpu_status_poll_ms(),
        work_base_dir: work_base,
        retry: RetryConfig {
            max_attempts: toml_config.retry.max_attempts,
            backoff_ms: toml_config.retry.backoff_ms,
        },
    })
    .add_stage(LoadStage::new(args.input.clone(), toml_config.load.dpi))
    .add_stage(DeskewStage::new(
        toml_config.correct.enable,
        toml_config.correct.deskew_strength,
    ))
    .add_stage(ColorStage::new(toml_config.correct.color_correction))
    .add_stage(UpscaleStage::new(
        toml_config.upscale.scale,
        toml_config.upscale.model.clone(),
        toml_config.upscale.enable,
        toml_config.upscale.tile,
        toml_config.upscale.fp32,
        upscale_queue,
    ))
    .add_stage(MarginStage::new(toml_config.ocr_pre.margin_trim))
    .add_stage(NormalizeStage::new(toml_config.ocr_pre.normalize_resolution))
    .add_stage(PageNumberStage::new(false))
    .add_stage(SaveStage::new(
        output_dir.clone(),
        toml_config.save.output_height,
        toml_config.save.jpeg_quality,
    ))
    .add_stage(CleanupStage::new(false));

    // Run just the target page (0-indexed)
    let page_idx = args.page.saturating_sub(1);
    let results = runner.run_all(page_idx + 1, None).await;

    if let Some(last) = results.last() {
        if last.success {
            println!("Preview saved to: {}", output_dir.display());
        } else {
            return Err(format!("Preview failed: {:?}", last.error).into());
        }
    }
    Ok(())
}

/// Probe PDF page count using pdftoppm
fn probe_pdf_page_count(pdf_path: &PathBuf) -> Result<usize, Box<dyn std::error::Error>> {
    let output = std::process::Command::new("pdfinfo")
        .arg(pdf_path)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                if line.starts_with("Pages:") {
                    if let Some(n) = line.split_whitespace().nth(1) {
                        return n.parse::<usize>().map_err(|e| e.into());
                    }
                }
            }
            Err("Could not parse page count from pdfinfo".into())
        }
        _ => Err("pdfinfo not available. Install poppler-utils.".into()),
    }
}