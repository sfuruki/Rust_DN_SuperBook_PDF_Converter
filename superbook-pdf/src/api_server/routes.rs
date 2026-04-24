//! REST API routes for the web server
//!
//! Provides endpoints for PDF conversion, job management, and health checks.

use axum::{
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{sse::Event, sse::KeepAlive, IntoResponse, Json, Sse},
    routing::{delete, get, post},
    Router,
};
use chrono::Utc;
use futures::stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use super::auth::{
    extract_api_key, AuthConfig, AuthManager, AuthResult, AuthStatusResponse, Scope,
};
use super::batch::{BatchJob, BatchProgress, BatchQueue, Priority};
use super::job::{ConvertOptions, Job, JobQueue, JobStatus};
use super::metrics::{MetricsCollector, StatsResponse, SystemMetrics};
use super::persistence::{
    HistoryQuery, HistoryResponse, JobStore, JsonJobStore, PersistenceConfig, RetryResponse,
};
use super::rate_limit::{
    RateLimitConfig, RateLimitError, RateLimitResult, RateLimitStatus, RateLimiter,
};
use super::websocket::{ws_job_handler, WsBroadcaster, WsMessage};
use super::worker::WorkerPool;

use std::net::IpAddr;
use std::path::{Path as StdPath, PathBuf};
use std::{fs::OpenOptions, io::Write};
use crate::config::PipelineTomlConfig;

/// Application state shared across handlers
pub struct AppState {
    pub queue: JobQueue,
    pub batch_queue: BatchQueue,
    pub version: String,
    pub worker_pool: WorkerPool,
    pub broadcaster: Arc<WsBroadcaster>,
    pub metrics: Arc<MetricsCollector>,
    pub rate_limiter: Arc<RateLimiter>,
    pub auth_manager: Arc<AuthManager>,
    pub job_store: Option<Arc<dyn JobStore>>,
    #[allow(dead_code)]
    pub persistence_config: PersistenceConfig,
}

impl AppState {
    pub fn new(work_dir: PathBuf, worker_count: usize) -> Self {
        Self::new_with_config(
            work_dir,
            worker_count,
            RateLimitConfig::default(),
            AuthConfig::default(),
        )
    }

    /// Create AppState with custom rate limit config (convenience method)
    #[allow(dead_code)]
    pub fn new_with_rate_limit(
        work_dir: PathBuf,
        worker_count: usize,
        rate_limit_config: RateLimitConfig,
    ) -> Self {
        Self::new_with_config(
            work_dir,
            worker_count,
            rate_limit_config,
            AuthConfig::default(),
        )
    }

    pub fn new_with_config(
        work_dir: PathBuf,
        worker_count: usize,
        rate_limit_config: RateLimitConfig,
        auth_config: AuthConfig,
    ) -> Self {
        Self::new_with_persistence(
            work_dir,
            worker_count,
            rate_limit_config,
            auth_config,
            PersistenceConfig::default(),
        )
    }

    /// Create AppState with full configuration including persistence
    pub fn new_with_persistence(
        work_dir: PathBuf,
        worker_count: usize,
        rate_limit_config: RateLimitConfig,
        auth_config: AuthConfig,
        persistence_config: PersistenceConfig,
    ) -> Self {
        let queue = JobQueue::new();
        let batch_queue = BatchQueue::new(queue.clone());
        let output_dir = match std::env::var("SUPERBOOK_OUTPUT_DIR") {
            Ok(path) if !path.trim().is_empty() => PathBuf::from(path),
            _ => work_dir.join("output"),
        };
        std::fs::create_dir_all(&output_dir).ok();

        let broadcaster = Arc::new(WsBroadcaster::new());
        let metrics = Arc::new(MetricsCollector::new());
        let rate_limiter = Arc::new(RateLimiter::new(rate_limit_config));
        let auth_manager = Arc::new(AuthManager::new(auth_config));
        let worker_pool = WorkerPool::new(
            queue.clone(),
            work_dir.clone(),
            output_dir.clone(),
            worker_count,
            broadcaster.clone(),
            metrics.clone(),
        );

        // Initialize job store if persistence is enabled
        let job_store: Option<Arc<dyn JobStore>> = if persistence_config.enabled {
            let store_path = persistence_config.storage_path.join("jobs.json");
            match JsonJobStore::new(store_path) {
                Ok(store) => Some(Arc::new(store)),
                Err(e) => {
                    eprintln!("Warning: Failed to initialize job store: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Self {
            queue,
            batch_queue,
            version: env!("CARGO_PKG_VERSION").to_string(),
            worker_pool,
            broadcaster,
            metrics,
            rate_limiter,
            auth_manager,
            job_store,
            persistence_config,
        }
    }
}

fn work_dir_for_job(base_work_dir: &StdPath, job_id: Uuid, filename: &str) -> PathBuf {
    let stem = StdPath::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("upload");
    base_work_dir.join(format!("work_{}_{}", job_id, stem))
}

fn input_path_for_job(base_work_dir: &StdPath, job_id: Uuid, filename: &str) -> PathBuf {
    let job_work_dir = work_dir_for_job(base_work_dir, job_id, filename);
    job_work_dir
        .join("input")
        .join(format!("{}_{}", job_id, filename))
}

/// Build the API router
pub fn api_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/convert", post(upload_and_start_job))
        .route("/jobs/{id}", get(get_job))
        .route("/jobs/{id}", delete(cancel_job))
        .route("/jobs/{id}/download", get(download_result))
        .route("/jobs/{id}/retry", post(retry_job))
        .route("/jobs/history", get(get_job_history))
        .route("/batch", post(create_batch))
        .route("/batch/{id}", get(get_batch))
        .route("/batch/{id}", delete(cancel_batch))
        .route("/batch/{id}/jobs", get(get_batch_jobs))
        .route("/health", get(health_check))
        .route("/metrics", get(get_metrics))
        .route("/stats", get(get_stats))
        .route("/rate-limit/status", get(get_rate_limit_status))
        .route("/auth/status", get(get_auth_status))
        .route("/config/schema", get(get_config_schema))
        .route("/config/current", get(get_config_current))
        .route("/config/list", get(get_config_list))
        .route("/config/save", post(post_config_save))
        .route("/config/validate", post(post_config_validate))
        .route("/config/history", get(get_config_history))
        .route("/config/restore", post(post_config_restore))
        .route("/config/delete", post(post_config_delete))
        .route("/config/load", post(post_config_load).get(get_config_load))
        .route("/progress/stream", get(get_progress_stream))
}

/// Build the WebSocket router
pub fn ws_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/jobs/{id}", get(ws_handler))
        .route("/batch/{id}", get(ws_batch_handler))
}

/// WebSocket handler wrapper that extracts broadcaster from AppState
async fn ws_handler(
    ws: axum::extract::ws::WebSocketUpgrade,
    Path(job_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws_job_handler(
        ws,
        Path(job_id),
        axum::extract::State(state.broadcaster.clone()),
    )
    .await
}

/// WebSocket handler for batch progress updates
async fn ws_batch_handler(
    ws: axum::extract::ws::WebSocketUpgrade,
    Path(batch_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // Reuse the same WebSocket handler - batch and job use the same broadcaster
    ws_job_handler(
        ws,
        Path(batch_id),
        axum::extract::State(state.broadcaster.clone()),
    )
    .await
}

/// AI service version information
#[derive(Debug, Clone, Serialize)]
pub struct AiServiceVersion {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub torch_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cuda_available: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
}

/// AI service versions
#[derive(Debug, Clone, Serialize)]
pub struct AiVersions {
    pub realesrgan: AiServiceVersion,
    pub yomitoku: AiServiceVersion,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub tools: ToolStatus,
    pub ai_services: AiVersions,
}

#[derive(Debug, Serialize)]
pub struct ToolStatus {
    pub poppler: bool,
    pub tesseract: bool,
    pub realesrgan: bool,
    pub yomitoku: bool,
}

/// Fetch AI service version via HTTP GET {base_url}/version with a short timeout.
async fn fetch_ai_version(base_url: &str) -> AiServiceVersion {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            return AiServiceVersion {
                available: false,
                service_version: None,
                torch_version: None,
                cuda_available: None,
                device: None,
            }
        }
    };

    match client.get(format!("{}/version", base_url)).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(v) => AiServiceVersion {
                available: true,
                service_version: v
                    .get("service_version")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string()),
                torch_version: v
                    .get("torch_version")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string()),
                cuda_available: v.get("cuda_available").and_then(|s| s.as_bool()),
                device: v
                    .get("device")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string()),
            },
            Err(_) => AiServiceVersion {
                available: true,
                service_version: None,
                torch_version: None,
                cuda_available: None,
                device: None,
            },
        },
        _ => AiServiceVersion {
            available: false,
            service_version: None,
            torch_version: None,
            cuda_available: None,
            device: None,
        },
    }
}

/// Health check endpoint
async fn health_check(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let realesrgan_url =
        std::env::var("REALESRGAN_API_URL").unwrap_or_else(|_| "http://realesrgan-api:8000".into());
    let yomitoku_url =
        std::env::var("YOMITOKU_API_URL").unwrap_or_else(|_| "http://yomitoku-api:8000".into());

    let (realesrgan_ver, yomitoku_ver) = tokio::join!(
        fetch_ai_version(&realesrgan_url),
        fetch_ai_version(&yomitoku_url)
    );

    let tools = ToolStatus {
        poppler: which::which("pdftoppm").is_ok(),
        tesseract: which::which("tesseract").is_ok(),
        realesrgan: realesrgan_ver.available,
        yomitoku: yomitoku_ver.available,
    };

    Json(HealthResponse {
        status: "healthy".to_string(),
        version: state.version.clone(),
        tools,
        ai_services: AiVersions {
            realesrgan: realesrgan_ver,
            yomitoku: yomitoku_ver,
        },
    })
}

/// Check if a Python module is available in the current runtime image.
#[allow(dead_code)]
fn check_python_module(module: &str) -> bool {
    // Check if python3 exists
    let python = which::which("python3").or_else(|_| which::which("python"));
    if python.is_err() {
        return false;
    }

    // Check if module is importable
    let import_cmd = format!("import {}", module);
    let output = std::process::Command::new(python.unwrap())
        .args(["-c", &import_cmd])
        .output();

    matches!(output, Ok(o) if o.status.success())
}

/// Get metrics in Prometheus format
async fn get_metrics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let queued = state.queue.pending_count() as u64;
    let ws_connections = state.broadcaster.channel_count().await;
    let worker_count = state.worker_pool.worker_count();

    let body = state
        .metrics
        .format_prometheus(queued, ws_connections, worker_count);

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body,
    )
}

/// Get statistics in JSON format
async fn get_stats(State(state): State<Arc<AppState>>) -> Json<StatsResponse> {
    let queued = state.queue.pending_count() as u64;
    let ws_connections = state.broadcaster.channel_count().await;
    let worker_count = state.worker_pool.worker_count();

    let response = StatsResponse {
        server: state.metrics.get_server_info(),
        jobs: state.metrics.get_job_statistics(queued),
        batches: state.metrics.get_batch_statistics(),
        system: SystemMetrics {
            memory_used_mb: get_memory_usage_mb(),
            worker_count,
            websocket_connections: ws_connections,
        },
    };

    Json(response)
}

/// Get current process memory usage in MB
fn get_memory_usage_mb() -> u64 {
    // Try to read from /proc/self/statm on Linux
    if let Ok(content) = std::fs::read_to_string("/proc/self/statm") {
        if let Some(rss) = content.split_whitespace().nth(1) {
            if let Ok(pages) = rss.parse::<u64>() {
                // Each page is typically 4KB
                return pages * 4 / 1024;
            }
        }
    }
    0
}

/// Get rate limit status
async fn get_rate_limit_status(
    State(state): State<Arc<AppState>>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Json<RateLimitStatus> {
    let ip = addr.ip();

    // Check current status for this IP
    let (remaining, reset_at) = match state.rate_limiter.check(ip) {
        RateLimitResult::Allowed {
            remaining,
            reset_at,
        } => (remaining, reset_at),
        RateLimitResult::Limited { retry_after } => {
            use std::time::{SystemTime, UNIX_EPOCH};
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            (0, now + retry_after)
        }
    };

    Json(RateLimitStatus {
        enabled: state.rate_limiter.is_enabled(),
        requests_per_minute: state.rate_limiter.requests_per_minute(),
        burst_size: state.rate_limiter.burst_size(),
        your_remaining: remaining,
        reset_at,
    })
}

/// Rate limit response type alias (used by middleware integration)
#[allow(dead_code)]
type RateLimitResponse = (
    StatusCode,
    [(header::HeaderName, String); 4],
    Json<RateLimitError>,
);

/// Check rate limit for a request. Returns None if allowed, or an error response if limited.
/// This function is designed to be used in middleware for rate limiting all API endpoints.
#[allow(dead_code)]
pub fn check_rate_limit(rate_limiter: &RateLimiter, ip: IpAddr) -> Option<RateLimitResponse> {
    match rate_limiter.check(ip) {
        RateLimitResult::Allowed {
            remaining,
            reset_at,
        } => {
            // Request allowed - headers will be added by middleware
            let _ = (remaining, reset_at);
            None
        }
        RateLimitResult::Limited { retry_after } => {
            // Request limited
            let error = RateLimitError::new(retry_after);
            Some((
                StatusCode::TOO_MANY_REQUESTS,
                [
                    (
                        header::HeaderName::from_static("x-ratelimit-limit"),
                        "0".to_string(),
                    ),
                    (
                        header::HeaderName::from_static("x-ratelimit-remaining"),
                        "0".to_string(),
                    ),
                    (
                        header::HeaderName::from_static("x-ratelimit-reset"),
                        "0".to_string(),
                    ),
                    (
                        header::HeaderName::from_static("retry-after"),
                        retry_after.to_string(),
                    ),
                ],
                Json(error),
            ))
        }
    }
}

/// Get authentication status
async fn get_auth_status(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<AuthStatusResponse> {
    // If auth is disabled, return disabled status
    if !state.auth_manager.is_enabled() {
        return Json(AuthStatusResponse::disabled());
    }

    // Extract API key from headers
    let authorization = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let x_api_key = headers.get("x-api-key").and_then(|v| v.to_str().ok());

    let api_key = extract_api_key(authorization, x_api_key);

    match api_key {
        Some(key) => match state.auth_manager.validate(&key) {
            AuthResult::Authenticated { key_name, scopes } => {
                Json(AuthStatusResponse::authenticated(key_name, scopes))
            }
            AuthResult::Disabled => Json(AuthStatusResponse::disabled()),
            AuthResult::Expired => Json(AuthStatusResponse::unauthenticated(true)),
            AuthResult::InvalidKey => Json(AuthStatusResponse::unauthenticated(true)),
            AuthResult::Missing => Json(AuthStatusResponse::unauthenticated(true)),
        },
        None => Json(AuthStatusResponse::unauthenticated(true)),
    }
}

#[derive(Debug, Deserialize)]
struct ConfigSaveRequest {
    #[serde(default)]
    config: Option<Value>,
    #[serde(default)]
    toml: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigLoadRequest {
    path: String,
}

#[derive(Debug, Deserialize)]
struct ConfigLoadQuery {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ConfigValidateRequest {
    #[serde(default)]
    config: Option<Value>,
    #[serde(default)]
    toml: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigDeleteRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ConfigHistoryQuery {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ConfigRestoreRequest {
    name: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct JobStatusQuery {
    job_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct JobStartResponse {
    pub job_id: Uuid,
    pub status: String,
    pub created_at: String,
    pub total_pages: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_name: Option<String>,
    pub effective_config_version: String,
}

fn preset_dir() -> PathBuf {
    PathBuf::from("config-presets")
}

fn history_dir() -> PathBuf {
    preset_dir().join(".history")
}

fn sanitize_config_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }
    let sanitized: String = trimmed
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if sanitized.is_empty() {
        None
    } else {
        Some(sanitized)
    }
}

fn config_path_by_name(name: &str) -> Result<PathBuf, String> {
    let normalized = name.trim();
    if normalized.eq_ignore_ascii_case("current") || normalized.eq_ignore_ascii_case("pipeline") {
        return Ok(PathBuf::from("pipeline.toml"));
    }

    let safe = sanitize_config_name(normalized)
        .ok_or_else(|| "Invalid config name: only [A-Za-z0-9_-] allowed".to_string())?;
    Ok(preset_dir().join(format!("{}.toml", safe)))
}

fn trim_history_versions(name: &str, keep: usize) -> Result<(), String> {
    let safe = match sanitize_config_name(name) {
        Some(v) => v,
        None => return Ok(()),
    };

    let mut versions: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(history_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if file_name.starts_with(&format!("{}--", safe)) {
                    versions.push(path);
                }
            }
        }
    }

    versions.sort();
    versions.reverse();

    for old in versions.into_iter().skip(keep) {
        let _ = std::fs::remove_file(old);
    }

    Ok(())
}

fn create_history_snapshot(name: &str, path: &StdPath, new_content: &str) -> Result<Option<String>, String> {
    let safe = match sanitize_config_name(name) {
        Some(v) => v,
        None => return Ok(None),
    };

    if !path.exists() {
        return Ok(None);
    }

    if let Ok(current_content) = std::fs::read_to_string(path) {
        if current_content == new_content {
            return Ok(None);
        }
    }

    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();

    let dir = history_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let snapshot_path = dir.join(format!("{}--{}.toml", safe, epoch));
    std::fs::copy(path, &snapshot_path).map_err(|e| e.to_string())?;
    Ok(Some(snapshot_path.display().to_string()))
}

fn ws_message_to_event(msg: WsMessage) -> (String, String) {
    let updated_at = Utc::now().to_rfc3339();
    let data = match msg {
        WsMessage::Progress {
            job_id,
            step_name,
            ..
        } => serde_json::json!({
            "job_id": job_id,
            "page": 0,
            "status": "running",
            "stage": step_name,
            "updated_at": updated_at,
        }),
        WsMessage::StatusChange {
            job_id,
            new_status,
            ..
        } => {
            let status = match new_status {
                JobStatus::Queued => "pending",
                JobStatus::Processing => "running",
                JobStatus::Completed => "done",
                JobStatus::Failed | JobStatus::Cancelled => "error",
            };
            serde_json::json!({
                "job_id": job_id,
                "page": 0,
                "status": status,
                "stage": "status",
                "updated_at": updated_at,
            })
        }
        WsMessage::Completed { job_id, .. } => serde_json::json!({
            "job_id": job_id,
            "page": 0,
            "status": "done",
            "stage": "save",
            "updated_at": updated_at,
        }),
        WsMessage::Error { job_id, .. } => serde_json::json!({
            "job_id": job_id,
            "page": 0,
            "status": "error",
            "stage": "error",
            "updated_at": updated_at,
        }),
        WsMessage::PagePreview {
            job_id,
            page_number,
            stage,
            ..
        } => serde_json::json!({
            "job_id": job_id,
            "page": page_number,
            "status": "running",
            "stage": stage,
            "updated_at": updated_at,
        }),
        WsMessage::Log { job_id, .. } => serde_json::json!({
            "job_id": job_id,
            "page": 0,
            "status": "running",
            "stage": "log",
            "updated_at": updated_at,
        }),
        WsMessage::BatchProgress { batch_id, .. } | WsMessage::BatchCompleted { batch_id, .. } => {
            serde_json::json!({
                "job_id": batch_id,
                "page": 0,
                "status": "running",
                "stage": "batch",
                "updated_at": updated_at,
            })
        }
        WsMessage::ServerShutdown { .. } => serde_json::json!({
            "job_id": "server",
            "page": 0,
            "status": "error",
            "stage": "shutdown",
            "updated_at": updated_at,
        }),
    };

    ("page_update".to_string(), data.to_string())
}

fn parse_config_request(
    config: Option<Value>,
    toml: Option<String>,
) -> Result<PipelineTomlConfig, String> {
    if let Some(cfg) = config {
        reject_legacy_parallel_keys_json(&cfg)?;
        return serde_json::from_value::<PipelineTomlConfig>(cfg)
            .map_err(|e| format!("Invalid config JSON: {}", e));
    }

    if let Some(text) = toml {
        return PipelineTomlConfig::from_toml(&text).map_err(|e| e.to_string());
    }

    Err("Missing config payload: provide `config`".to_string())
}

fn merge_json_values(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Object(base_map), Value::Object(overlay_map)) => {
            for (k, v) in overlay_map {
                merge_json_values(base_map.entry(k).or_insert(Value::Null), v);
            }
        }
        (slot, value) => {
            *slot = value;
        }
    }
}

fn reject_legacy_parallel_keys_json(value: &Value) -> Result<(), String> {
    let Some(concurrency) = value.get("concurrency").and_then(|v| v.as_object()) else {
        return Ok(());
    };

    if concurrency.contains_key("max_parallel_pages") {
        return Err(
            "Unsupported legacy key 'concurrency.max_parallel_pages'. Use 'concurrency.page_parallel'."
                .to_string(),
        );
    }

    if concurrency.contains_key("max_parallel_pages_cpu") {
        return Err(
            "Unsupported legacy key 'concurrency.max_parallel_pages_cpu'. Use 'concurrency.page_parallel'."
                .to_string(),
        );
    }

    if concurrency.contains_key("max_parallel_pages_gpu") {
        return Err(
            "Unsupported legacy key 'concurrency.max_parallel_pages_gpu'. Use 'concurrency.gpu_stage_parallel'."
                .to_string(),
        );
    }

    Ok(())
}

fn config_fingerprint(config: &PipelineTomlConfig) -> String {
    let text = config.to_toml().unwrap_or_default();
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in text.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("cfg-{:016x}", hash)
}

fn map_config_to_convert_options(config: &PipelineTomlConfig) -> ConvertOptions {
    ConvertOptions {
        dpi: config.load.dpi,
        deskew: config.correct.enable,
        upscale: config.upscale.enable,
        ocr: config.ocr.enable,
        advanced: config.correct.color_correction
            || config.ocr_pre.margin_trim
            || config.ocr_pre.normalize_resolution,
    }
}

fn convert_options_to_pipeline_toml_config(options: &ConvertOptions) -> PipelineTomlConfig {
    let mut config = PipelineTomlConfig::default();
    config.load.dpi = options.dpi;
    config.correct.enable = options.deskew;
    config.correct.color_correction = options.advanced;
    config.upscale.enable = options.upscale;
    config.ocr.enable = options.ocr;
    config.ocr_pre.margin_trim = options.advanced;
    config.ocr_pre.normalize_resolution = options.advanced;
    config
}

fn job_effective_config(job: &Job) -> PipelineTomlConfig {
    job.effective_config
        .clone()
        .unwrap_or_else(|| convert_options_to_pipeline_toml_config(&job.options))
}

fn resolve_effective_config(
    config_name: Option<&str>,
    inline_config: Option<Value>,
) -> Result<(PipelineTomlConfig, Option<String>, String), String> {
    let mut merged = serde_json::to_value(PipelineTomlConfig::load().unwrap_or_default())
        .map_err(|e| e.to_string())?;

    let mut normalized_name: Option<String> = None;

    if let Some(name) = config_name {
        let trimmed = name.trim();
        let preset = if trimmed.eq_ignore_ascii_case("pipeline")
            || trimmed.eq_ignore_ascii_case("current")
        {
            // pipeline/current はファイル未配置でも既定値で解決できる。
            PipelineTomlConfig::load().unwrap_or_default()
        } else {
            let path = config_path_by_name(trimmed)?;
            if !path.exists() {
                return Err(format!("Config not found: {}", path.display()));
            }
            PipelineTomlConfig::load_from_path(&path).map_err(|e| e.to_string())?
        };
        let preset_json = serde_json::to_value(preset).map_err(|e| e.to_string())?;
        merge_json_values(&mut merged, preset_json);
        normalized_name = Some(trimmed.to_string());
    }

    if let Some(inline) = inline_config {
        reject_legacy_parallel_keys_json(&inline)?;
        merge_json_values(&mut merged, inline);
    }

    let effective: PipelineTomlConfig =
        serde_json::from_value(merged).map_err(|e| format!("Invalid inline config: {}", e))?;
    let version = config_fingerprint(&effective);

    Ok((effective, normalized_name, version))
}

fn probe_pdf_total_pages(pdf_path: &StdPath) -> usize {
    let output = std::process::Command::new("pdfinfo").arg(pdf_path).output();
    if let Ok(out) = output {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                if line.starts_with("Pages:") {
                    if let Some(n) = line.split_whitespace().nth(1) {
                        if let Ok(v) = n.parse::<usize>() {
                            return v;
                        }
                    }
                }
            }
        }
    }
    0
}

fn authorize_scope(
    state: &AppState,
    headers: &HeaderMap,
    required_scope: Scope,
) -> Result<Option<String>, AppError> {
    if !state.auth_manager.is_enabled() {
        return Ok(None);
    }

    let authorization = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let x_api_key = headers.get("x-api-key").and_then(|v| v.to_str().ok());

    let api_key = extract_api_key(authorization, x_api_key)
        .ok_or_else(|| AppError::Unauthorized("API key required".to_string()))?;

    match state.auth_manager.validate(&api_key) {
        AuthResult::Authenticated { key_name, scopes } => {
            let ok = scopes.iter().any(|s| s.includes(required_scope));
            if ok {
                Ok(Some(key_name))
            } else {
                Err(AppError::Forbidden("Insufficient scope".to_string()))
            }
        }
        AuthResult::Disabled => Ok(None),
        AuthResult::Expired => Err(AppError::Unauthorized("API key expired".to_string())),
        AuthResult::InvalidKey => Err(AppError::Unauthorized("Invalid API key".to_string())),
        AuthResult::Missing => Err(AppError::Unauthorized("API key missing".to_string())),
    }
}

fn extract_single_page_pdf(source_pdf: &StdPath, page: usize, output_pdf: &StdPath) -> Result<(), AppError> {
    if page == 0 {
        return Err(AppError::BadRequest("page must be >= 1".to_string()));
    }

    if let Some(parent) = output_pdf.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Internal(format!("Failed to create page extract directory: {}", e)))?;
    }

    let status = std::process::Command::new("pdfseparate")
        .arg("-f")
        .arg(page.to_string())
        .arg("-l")
        .arg(page.to_string())
        .arg(source_pdf)
        .arg(output_pdf)
        .status()
        .map_err(|e| AppError::Internal(format!("Failed to run pdfseparate: {}", e)))?;

    if !status.success() {
        return Err(AppError::Internal("pdfseparate failed to extract requested page".to_string()));
    }

    if !output_pdf.exists() {
        return Err(AppError::Internal("extracted page PDF was not created".to_string()));
    }

    Ok(())
}

fn write_audit_log(
    state: &AppState,
    user_id: Option<&str>,
    action: &str,
    details: serde_json::Value,
) {
    let path = std::env::var("SUPERBOOK_AUDIT_LOG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| state.worker_pool.work_dir().join("audit.log"));

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let event = serde_json::json!({
        "timestamp": Utc::now().to_rfc3339(),
        "user_id": user_id.unwrap_or("anonymous"),
        "action": action,
        "details": details,
    });

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{}", event);
    }
}

async fn get_config_schema() -> impl IntoResponse {
    Json(serde_json::json!({
        "schema_version": "1.0.0",
        "fields": [
            {
                "path": "load.dpi",
                "type": "number",
                "min": 72,
                "max": 1200,
                "default": 300,
                "ui": "input",
                "label": "DPI",
                "description": "PDF extraction DPI"
            },
            {
                "path": "correct.enable",
                "type": "boolean",
                "default": true,
                "ui": "toggle",
                "label": "補正有効",
                "description": "Deskew stage enable"
            },
            {
                "path": "correct.deskew_strength",
                "type": "number",
                "min": 0.0,
                "max": 1.0,
                "default": 0.8,
                "ui": "slider",
                "label": "傾き補正強度",
                "description": "0.0=off, 1.0=max"
            },
            {
                "path": "upscale.scale",
                "type": "number",
                "enum": [1,2,4],
                "default": 2,
                "ui": "select",
                "label": "超解像倍率",
                "description": "RealESRGAN upscale factor"
            },
            {
                "path": "upscale.model",
                "type": "string",
                "enum": ["realesrgan-x2plus", "realesrgan-x4plus"],
                "default": "realesrgan-x4plus",
                "ui": "select",
                "label": "超解像モデル",
                "description": "RealESRGAN model name"
            },
            {
                "path": "ocr.enable",
                "type": "boolean",
                "default": false,
                "ui": "toggle",
                "label": "OCR有効",
                "description": "Enable OCR stage"
            },
            {
                "path": "ocr.language",
                "type": "string",
                "enum": ["jpn", "ja", "ja-jp", "japanese"],
                "default": "jpn",
                "ui": "select",
                "label": "OCR言語",
                "description": "OCR language"
            },
            {
                "path": "retry.max_attempts",
                "type": "number",
                "min": 1,
                "max": 10,
                "default": 3,
                "ui": "input",
                "label": "最大リトライ回数",
                "description": "Retry count per stage"
            },
            {
                "path": "retry.backoff_ms",
                "type": "number",
                "min": 0,
                "max": 60000,
                "default": 500,
                "ui": "input",
                "label": "バックオフ(ms)",
                "description": "Initial retry backoff"
            },
            {
                "path": "concurrency.page_parallel",
                "type": "number",
                "min": 0,
                "max": 256,
                "default": 0,
                "ui": "input",
                "label": "ページ並列",
                "description": "Per-job page parallelism (0 = fully dynamic auto)"
            },
            {
                "path": "concurrency.job_parallel",
                "type": "number",
                "min": 0,
                "max": 256,
                "default": 0,
                "ui": "input",
                "label": "同時ジョブ並列",
                "description": "Concurrent job count (0 = fully dynamic auto)"
            },
            {
                "path": "concurrency.gpu_stage_parallel",
                "type": "number",
                "min": 0,
                "max": 256,
                "default": 0,
                "ui": "input",
                "label": "GPUステージ並列",
                "description": "Concurrent GPU stage invocations (0 = fully dynamic auto)"
            },
            {
                "path": "concurrency.cpu_dynamic_min_parallel",
                "type": "number",
                "min": 0,
                "max": 256,
                "default": 0,
                "ui": "input",
                "label": "CPU最小並列",
                "description": "Minimum per-job parallelism when CPU dynamic control throttles (0 = auto)"
            },
            {
                "path": "concurrency.cpu_target_load_per_core",
                "type": "number",
                "min": 0.1,
                "max": 4.0,
                "default": 0.9,
                "ui": "input",
                "label": "CPU目標負荷/コア",
                "description": "Target 1m load average per CPU core"
            },
            {
                "path": "concurrency.cpu_status_poll_ms",
                "type": "number",
                "min": 20,
                "max": 10000,
                "default": 200,
                "ui": "input",
                "label": "CPU再評価間隔(ms)",
                "description": "Polling interval for CPU dynamic parallelism"
            }
        ]
    }))
}

async fn get_config_current() -> impl IntoResponse {
    let config = PipelineTomlConfig::load().unwrap_or_default();
    let version = config_fingerprint(&config);
    (StatusCode::OK, Json(serde_json::json!({ "name": "pipeline", "config": config, "version": version }))).into_response()
}

async fn get_config_list() -> impl IntoResponse {
    let mut names = vec!["pipeline".to_string()];
    let dir = preset_dir();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }
    }
    names.sort();
    names.dedup();
    Json(serde_json::json!({ "configs": names }))
}

async fn post_config_save(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ConfigSaveRequest>,
) -> impl IntoResponse {
    let auth_user = match authorize_scope(&state, &headers, Scope::Write) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };

    match parse_config_request(req.config, req.toml) {
        Ok(config) => {
            let req_name = req.name.clone();

            let path = match req_name.as_deref() {
                Some(name) => match config_path_by_name(name) {
                    Ok(path) => path,
                    Err(e) => {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(serde_json::json!({ "ok": false, "error": e })),
                        )
                            .into_response();
                    }
                },
                None => PathBuf::from("pipeline.toml"),
            };

            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
                        )
                            .into_response();
                    }
                }
            }

            let content = match config.to_toml() {
                Ok(v) => v,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
                    )
                        .into_response();
                }
            };

            let history_snapshot = if let Some(name) = req_name.as_deref() {
                if !name.eq_ignore_ascii_case("pipeline") {
                    match create_history_snapshot(name, &path, &content) {
                        Ok(v) => v,
                        Err(e) => {
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({ "ok": false, "error": e })),
                            )
                                .into_response();
                        }
                    }
                } else {
                    None
                }
            } else {
                None
            };

            let response = match std::fs::write(&path, content) {
                Ok(_) => {
                    if let Some(name) = req_name.as_deref() {
                        let _ = trim_history_versions(name, 20);
                    }

                    write_audit_log(
                        &state,
                        auth_user.as_deref(),
                        "config/save",
                        serde_json::json!({
                            "name": req_name,
                            "path": path.display().to_string(),
                            "version": config_fingerprint(&config),
                        }),
                    );

                    (
                        StatusCode::OK,
                        Json(serde_json::json!({
                            "ok": true,
                            "name": req_name,
                            "config": config,
                            "version": config_fingerprint(&config),
                            "path": path.display().to_string(),
                            "history_snapshot": history_snapshot,
                        })),
                    )
                        .into_response()
                }
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
                )
                    .into_response(),
            };

            response
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn post_config_restore(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ConfigRestoreRequest>,
) -> impl IntoResponse {
    let auth_user = match authorize_scope(&state, &headers, Scope::Write) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };

    let safe = match sanitize_config_name(&req.name) {
        Some(v) => v,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "error": "Invalid config name" })),
            )
                .into_response();
        }
    };

    let version_file = req.version.trim();
    if !version_file.starts_with(&format!("{}--", safe)) || !version_file.ends_with(".toml") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "ok": false, "error": "Invalid version" })),
        )
            .into_response();
    }

    let source = history_dir().join(version_file);
    if !source.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "ok": false, "error": "Version not found" })),
        )
            .into_response();
    }

    let target = match config_path_by_name(&req.name) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "error": e })),
            )
                .into_response();
        }
    };

    if let Some(parent) = target.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match std::fs::copy(&source, &target) {
        Ok(_) => match PipelineTomlConfig::load_from_path(&target) {
            Ok(config) => {
                write_audit_log(
                    &state,
                    auth_user.as_deref(),
                    "config/restore",
                    serde_json::json!({
                        "name": req.name,
                        "restored_version": req.version,
                        "version": config_fingerprint(&config),
                    }),
                );

                (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "ok": true,
                        "name": req.name,
                        "restored_version": req.version,
                        "config": config,
                        "version": config_fingerprint(&config),
                    })),
                )
                    .into_response()
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
            )
                .into_response(),
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn post_config_validate(Json(req): Json<ConfigValidateRequest>) -> impl IntoResponse {
    match parse_config_request(req.config, req.toml) {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({ "ok": true, "message": "Valid pipeline config" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn get_config_history(Query(query): Query<ConfigHistoryQuery>) -> impl IntoResponse {
    let safe = match sanitize_config_name(&query.name) {
        Some(v) => v,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "error": "Invalid config name" })),
            )
                .into_response();
        }
    };

    let mut versions: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(history_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with(&format!("{}--", safe)) {
                    versions.push(name.to_string());
                }
            }
        }
    }
    versions.sort();
    versions.reverse();

    (
        StatusCode::OK,
        Json(serde_json::json!({ "ok": true, "name": query.name, "versions": versions })),
    )
        .into_response()
}

async fn post_config_delete(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ConfigDeleteRequest>,
) -> impl IntoResponse {
    let auth_user = match authorize_scope(&state, &headers, Scope::Admin) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };

    let trimmed = req.name.trim();
    if trimmed.eq_ignore_ascii_case("pipeline") || trimmed.eq_ignore_ascii_case("current") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "ok": false, "error": "Cannot delete pipeline config" })),
        )
            .into_response();
    }

    let path = match config_path_by_name(trimmed) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "error": e })),
            )
                .into_response();
        }
    };

    if !path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "ok": false, "error": "Config not found" })),
        )
            .into_response();
    }

    match std::fs::remove_file(&path) {
        Ok(_) => {
            write_audit_log(
                &state,
                auth_user.as_deref(),
                "config/delete",
                serde_json::json!({
                    "name": trimmed,
                    "path": path.display().to_string(),
                }),
            );

            (
                StatusCode::OK,
                Json(serde_json::json!({ "ok": true, "deleted": trimmed })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn get_config_load(Query(query): Query<ConfigLoadQuery>) -> impl IntoResponse {
    let path = match config_path_by_name(&query.name) {
        Ok(path) => path,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "error": e })),
            )
                .into_response();
        }
    };

    match PipelineTomlConfig::load_from_path(&path) {
        Ok(config) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "ok": true,
                "name": query.name,
                "config": config,
                "version": config_fingerprint(&config),
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn get_progress_stream(
    State(state): State<Arc<AppState>>,
    Query(query): Query<JobStatusQuery>,
) -> impl IntoResponse {
    let receiver = state.broadcaster.subscribe(query.job_id).await;
    let stream = stream::unfold(receiver, |mut rx| async move {
        match rx.recv().await {
            Ok(msg) => {
                let (event_name, data) = ws_message_to_event(msg);
                let event = Event::default().event(event_name).data(data);
                Some((Ok::<Event, Infallible>(event), rx))
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                let event = Event::default().event("warning").data(
                    serde_json::json!({
                        "message": "SSE stream lagged",
                        "skipped": skipped
                    })
                    .to_string(),
                );
                Some((Ok::<Event, Infallible>(event), rx))
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => None,
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)).text("keepalive"))
}

async fn upload_and_start_job(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<JobStartResponse>), AppError> {
    let auth_user = authorize_scope(&state, &headers, Scope::Write)?;

    let mut filename = String::new();
    let mut file_data: Option<Vec<u8>> = None;
    let mut config_name: Option<String> = None;
    let mut inline_config: Option<Value> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                filename = field.file_name().unwrap_or("upload.pdf").to_string();
                let bytes = field.bytes().await.map_err(|e| {
                    AppError::BadRequest(format!("Failed to read uploaded file data: {}", e))
                })?;
                file_data = Some(bytes.to_vec());
            }
            "config_name" => {
                if let Ok(text) = field.text().await {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        config_name = Some(trimmed.to_string());
                    }
                }
            }
            "inline_config" => {
                if let Ok(text) = field.text().await {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        inline_config = Some(
                            serde_json::from_str::<Value>(trimmed).map_err(|e| {
                                AppError::BadRequest(format!("Invalid inline_config JSON: {}", e))
                            })?,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    if filename.is_empty() {
        return Err(AppError::BadRequest("No file uploaded".to_string()));
    }

    let file_data = file_data.ok_or_else(|| AppError::BadRequest("No file data".to_string()))?;

    let (effective_config, normalized_name, effective_version) = resolve_effective_config(
        config_name.as_deref(),
        inline_config,
    )
    .map_err(AppError::BadRequest)?;

    let options = map_config_to_convert_options(&effective_config);

    let job = Job::new(&filename, options.clone()).with_effective_config(effective_config.clone());
    let job_id = job.id;
    let created_at = job.created_at.to_rfc3339();

    let input_path = input_path_for_job(state.worker_pool.work_dir(), job_id, &filename);
    if let Some(parent) = input_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Internal(format!("Failed to create upload directory: {}", e)))?;
    }
    std::fs::write(&input_path, &file_data)
        .map_err(|e| AppError::Internal(format!("Failed to save uploaded file: {}", e)))?;

    let total_pages = probe_pdf_total_pages(&input_path);

    state.queue.submit(job);

    if let Err(e) = state
        .worker_pool
        .submit(job_id, input_path, effective_config)
        .await
    {
        state.queue.update(job_id, |job| {
            job.fail(format!("Failed to start processing: {}", e));
        });
    }

    write_audit_log(
        &state,
        auth_user.as_deref(),
        "job/start",
        serde_json::json!({
            "job_id": job_id,
            "config_name": normalized_name,
            "effective_config_version": effective_version,
            "input_filename": filename,
        }),
    );

    Ok((
        StatusCode::ACCEPTED,
        Json(JobStartResponse {
            job_id,
            status: "pending".to_string(),
            created_at,
            total_pages,
            config_name: normalized_name,
            effective_config_version: effective_version,
        }),
    ))
}

async fn get_job_status_query(
    State(state): State<Arc<AppState>>,
    Query(query): Query<JobStatusQuery>,
) -> Result<Json<Job>, AppError> {
    state
        .queue
        .get(query.job_id)
        .map(Json)
        .ok_or(AppError::NotFound(format!("Job {} not found", query.job_id)))
}

async fn post_config_load(Json(req): Json<ConfigLoadRequest>) -> impl IntoResponse {
    let requested_path = req.path.clone();
    let path = PathBuf::from(req.path);
    match PipelineTomlConfig::load_from_path(&path) {
        Ok(config) => (
            StatusCode::OK,
            Json(serde_json::json!({ "ok": true, "name": requested_path, "config": config, "version": config_fingerprint(&config) })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// Get job history
async fn get_job_history(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<HistoryQuery>,
) -> Result<Json<HistoryResponse>, AppError> {
    // Get jobs from job store if persistence is enabled
    let all_jobs = if let Some(store) = &state.job_store {
        store
            .list()
            .map_err(|e| AppError::Internal(format!("Failed to get job history: {}", e)))?
    } else {
        // Fall back to in-memory queue
        state.queue.list()
    };

    // Filter by status if provided
    let filtered: Vec<Job> = if let Some(status) = &query.status {
        all_jobs
            .into_iter()
            .filter(|j| &j.status == status)
            .collect()
    } else {
        all_jobs
    };

    let total = filtered.len();

    // Apply pagination
    let jobs: Vec<Job> = filtered
        .into_iter()
        .skip(query.offset)
        .take(query.limit)
        .collect();

    Ok(Json(HistoryResponse {
        jobs,
        total,
        limit: query.limit,
        offset: query.offset,
    }))
}

/// Retry a failed job
async fn retry_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<RetryResponse>, AppError> {
    // Get the job
    let job = state
        .queue
        .get(id)
        .ok_or_else(|| AppError::NotFound(format!("Job {} not found", id)))?;

    // Check if job can be retried (only failed jobs)
    if job.status != JobStatus::Failed {
        return Ok(Json(RetryResponse::error(
            id,
            format!("Cannot retry job with status: {}", job.status),
        )));
    }

    // Create a new job based on the failed one
    let retry_config = job_effective_config(&job);
    let new_job = Job::new(&job.input_filename, job.options.clone())
        .with_effective_config(retry_config.clone());
    let new_job_id = new_job.id;

    // Submit the new job
    state.queue.submit(new_job);

    // Try to find the original input file and resubmit
    let input_path = input_path_for_job(state.worker_pool.work_dir(), id, &job.input_filename);
    if input_path.exists() {
        // Copy to new job path
        let new_input_path = input_path_for_job(
            state.worker_pool.work_dir(),
            new_job_id,
            &job.input_filename,
        );
        if let Some(parent) = new_input_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        if std::fs::copy(&input_path, &new_input_path).is_ok() {
            if let Err(e) = state
                .worker_pool
                .submit(new_job_id, new_input_path, retry_config)
                .await
            {
                state.queue.update(new_job_id, |job| {
                    job.fail(format!("Failed to start processing: {}", e));
                });
            }
        }
    }

    Ok(Json(RetryResponse::success(new_job_id)))
}

/// Get job status
async fn get_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Job>, AppError> {
    state
        .queue
        .get(id)
        .map(Json)
        .ok_or(AppError::NotFound(format!("Job {} not found", id)))
}

/// Cancel a job
async fn cancel_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Job>, AppError> {
    state
        .queue
        .cancel(id)
        .map(Json)
        .ok_or(AppError::NotFound(format!("Job {} not found", id)))
}

/// Download result response struct
#[derive(Debug)]
pub struct PdfDownload {
    data: Vec<u8>,
    filename: String,
}

impl IntoResponse for PdfDownload {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::OK,
            [
                ("Content-Type", "application/pdf"),
                (
                    "Content-Disposition",
                    format!("attachment; filename=\"{}\"", self.filename).as_str(),
                ),
            ],
            self.data,
        )
            .into_response()
    }
}

/// Download conversion result
async fn download_result(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let job = state
        .queue
        .get(id)
        .ok_or(AppError::NotFound(format!("Job {} not found", id)))?;

    match job.status {
        super::job::JobStatus::Completed => {
            if let Some(path) = &job.output_path {
                let data = std::fs::read(path).map_err(|e| {
                    AppError::Internal(format!("Failed to read output file: {}", e))
                })?;

                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("output.pdf")
                    .to_string();

                Ok(PdfDownload { data, filename })
            } else {
                Err(AppError::Internal("Output file not found".to_string()))
            }
        }
        super::job::JobStatus::Queued | super::job::JobStatus::Processing => Err(
            AppError::Conflict(format!("Job {} is still {}", id, job.status)),
        ),
        super::job::JobStatus::Failed => Err(AppError::Conflict(format!(
            "Job {} failed: {}",
            id,
            job.error.as_deref().unwrap_or("Unknown error")
        ))),
        super::job::JobStatus::Cancelled => {
            Err(AppError::Conflict(format!("Job {} was cancelled", id)))
        }
    }
}

// ========== Batch API Handlers ==========

/// Batch creation request
#[derive(Debug, serde::Deserialize)]
pub struct BatchRequest {
    #[serde(default)]
    pub options: ConvertOptions,
    #[serde(default)]
    pub priority: Priority,
}

#[derive(Debug, serde::Deserialize)]
pub struct BatchConfigRequest {
    pub config_name: Option<String>,
    pub inline_config: Option<Value>,
    #[serde(default)]
    pub priority: Priority,
}

/// Batch creation response
#[derive(Debug, Serialize)]
pub struct BatchResponse {
    pub batch_id: Uuid,
    pub status: String,
    pub job_count: usize,
    pub created_at: String,
}

/// Batch status response
#[derive(Debug, Serialize)]
pub struct BatchStatusResponse {
    pub batch_id: Uuid,
    pub status: String,
    pub progress: BatchProgress,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

/// Batch jobs response
#[derive(Debug, Serialize)]
pub struct BatchJobsResponse {
    pub batch_id: Uuid,
    pub jobs: Vec<BatchJobInfo>,
}

/// Individual job info in batch
#[derive(Debug, Serialize)]
pub struct BatchJobInfo {
    pub job_id: Uuid,
    pub filename: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<JobProgressInfo>,
}

/// Job progress info
#[derive(Debug, Serialize)]
pub struct JobProgressInfo {
    pub percent: u8,
    pub step_name: String,
}

/// Batch cancel response
#[derive(Debug, Serialize)]
pub struct BatchCancelResponse {
    pub batch_id: Uuid,
    pub status: String,
    pub cancelled_jobs: usize,
    pub completed_jobs: usize,
}

/// Create a new batch job
async fn create_batch(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<BatchResponse>), AppError> {
    let mut filenames: Vec<String> = Vec::new();
    let mut file_data_list: Vec<(String, Vec<u8>)> = Vec::new();
    let mut effective_config = PipelineTomlConfig::default();
    let mut options = map_config_to_convert_options(&effective_config);
    let mut priority = Priority::default();

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "files[]" | "files" => {
                let filename = field.file_name().unwrap_or("upload.pdf").to_string();
                match field.bytes().await {
                    Ok(data) => {
                        file_data_list.push((filename.clone(), data.to_vec()));
                        filenames.push(filename);
                    }
                    Err(e) => {
                        return Err(AppError::BadRequest(format!(
                            "Failed to read uploaded file data: {}",
                            e
                        )));
                    }
                }
            }
            "options" => {
                if let Ok(text) = field.text().await {
                    if let Ok(parsed) = serde_json::from_str::<BatchConfigRequest>(&text) {
                        let (resolved, _, _) = resolve_effective_config(
                            parsed.config_name.as_deref(),
                            parsed.inline_config,
                        )
                        .map_err(AppError::BadRequest)?;
                        effective_config = resolved;
                        options = map_config_to_convert_options(&effective_config);
                        priority = parsed.priority;
                    } else if let Ok(parsed) = serde_json::from_str::<BatchRequest>(&text) {
                        effective_config = convert_options_to_pipeline_toml_config(&parsed.options);
                        options = parsed.options;
                        priority = parsed.priority;
                    } else if let Ok(parsed) = serde_json::from_str::<ConvertOptions>(&text) {
                        effective_config = convert_options_to_pipeline_toml_config(&parsed);
                        options = parsed;
                    }
                }
            }
            _ => {}
        }
    }

    if filenames.is_empty() {
        return Err(AppError::BadRequest("No files uploaded".to_string()));
    }

    // Create batch job
    let mut batch = BatchJob::new(options.clone(), priority);
    let batch_id = batch.id;
    let created_at = batch.created_at.to_rfc3339();

    // Create individual jobs and save files
    for (filename, data) in file_data_list {
        let job = Job::new(&filename, options.clone())
            .with_effective_config(effective_config.clone());
        let job_id = job.id;

        // Save uploaded file
        let input_path = input_path_for_job(state.worker_pool.work_dir(), job_id, &filename);
        if let Some(parent) = input_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::Internal(format!("Failed to create upload directory: {}", e))
            })?;
        }
        std::fs::write(&input_path, &data)
            .map_err(|e| AppError::Internal(format!("Failed to save uploaded file: {}", e)))?;

        // Submit job
        state.queue.submit(job);
        batch.add_job(job_id);

        // Start processing
        if let Err(e) = state
            .worker_pool
            .submit(job_id, input_path, effective_config.clone())
            .await
        {
            state.queue.update(job_id, |job| {
                job.fail(format!("Failed to start processing: {}", e));
            });
        }
    }

    let job_count = batch.job_count();
    batch.start();

    // Submit batch to queue
    state.batch_queue.submit(batch).await;

    Ok((
        StatusCode::ACCEPTED,
        Json(BatchResponse {
            batch_id,
            status: "processing".to_string(),
            job_count,
            created_at,
        }),
    ))
}

/// Get batch status
async fn get_batch(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<BatchStatusResponse>, AppError> {
    let batch = state
        .batch_queue
        .get(id)
        .await
        .ok_or_else(|| AppError::NotFound(format!("Batch {} not found", id)))?;

    let progress = state
        .batch_queue
        .get_progress(id)
        .await
        .unwrap_or_else(|| BatchProgress::new(0));

    Ok(Json(BatchStatusResponse {
        batch_id: batch.id,
        status: batch.status.to_string(),
        progress,
        created_at: batch.created_at.to_rfc3339(),
        started_at: batch.started_at.map(|t| t.to_rfc3339()),
        completed_at: batch.completed_at.map(|t| t.to_rfc3339()),
    }))
}

/// Get batch jobs list
async fn get_batch_jobs(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<BatchJobsResponse>, AppError> {
    let batch = state
        .batch_queue
        .get(id)
        .await
        .ok_or_else(|| AppError::NotFound(format!("Batch {} not found", id)))?;

    let mut jobs = Vec::new();
    for job_id in &batch.job_ids {
        if let Some(job) = state.queue.get(*job_id) {
            let download_url = if job.status == JobStatus::Completed {
                Some(format!("/api/jobs/{}/download", job_id))
            } else {
                None
            };

            let progress = if job.status == JobStatus::Processing {
                job.progress.as_ref().map(|p| JobProgressInfo {
                    percent: p.percent,
                    step_name: p.step_name.clone(),
                })
            } else {
                None
            };

            jobs.push(BatchJobInfo {
                job_id: *job_id,
                filename: job.input_filename.clone(),
                status: job.status.to_string(),
                download_url,
                progress,
            });
        }
    }

    Ok(Json(BatchJobsResponse {
        batch_id: batch.id,
        jobs,
    }))
}

/// Cancel a batch
async fn cancel_batch(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<BatchCancelResponse>, AppError> {
    let result = state
        .batch_queue
        .cancel(id)
        .await
        .ok_or_else(|| AppError::NotFound(format!("Batch {} not found", id)))?;

    Ok(Json(BatchCancelResponse {
        batch_id: id,
        status: "cancelled".to_string(),
        cancelled_jobs: result.0,
        completed_jobs: result.1,
    }))
}

/// API error type
#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    Internal(String),
    /// Rate limit exceeded (used by rate limiting middleware)
    #[allow(dead_code)]
    TooManyRequests {
        retry_after: u64,
    },
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        #[derive(Serialize)]
        struct ErrorResponse {
            error_code: String,
            message: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            details: Option<serde_json::Value>,
            #[serde(skip_serializing_if = "Option::is_none")]
            retry_after: Option<u64>,
        }

        let (status, code, message, details, retry_after) = match &self {
            AppError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                "VALIDATION_FAILED".to_string(),
                msg.clone(),
                None,
                None,
            ),
            AppError::Unauthorized(msg) => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED".to_string(),
                msg.clone(),
                None,
                None,
            ),
            AppError::Forbidden(msg) => (
                StatusCode::FORBIDDEN,
                "FORBIDDEN".to_string(),
                msg.clone(),
                None,
                None,
            ),
            AppError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                "NOT_FOUND".to_string(),
                msg.clone(),
                None,
                None,
            ),
            AppError::Conflict(msg) => (
                StatusCode::CONFLICT,
                "CONFLICT".to_string(),
                msg.clone(),
                None,
                None,
            ),
            AppError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR".to_string(),
                msg.clone(),
                None,
                None,
            ),
            AppError::TooManyRequests { retry_after } => (
                StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMIT_EXCEEDED".to_string(),
                "Rate limit exceeded".to_string(),
                None,
                Some(*retry_after),
            ),
        };

        let mut response = (
            status,
            Json(ErrorResponse {
                error_code: code,
                message,
                details,
                retry_after,
            }),
        )
            .into_response();

        if let AppError::TooManyRequests { retry_after } = self {
            if let Ok(value) = header::HeaderValue::from_str(&retry_after.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reject_legacy_parallel_keys_json_max_parallel_pages() {
        let json = serde_json::json!({
            "concurrency": {
                "max_parallel_pages": 2
            }
        });

        let err = reject_legacy_parallel_keys_json(&json).unwrap_err();
        assert!(err.contains("Unsupported legacy key"));
    }

    #[test]
    fn test_parse_config_request_rejects_legacy_parallel_keys_json() {
        let json = serde_json::json!({
            "concurrency": {
                "max_parallel_pages_gpu": 3
            }
        });

        let err = parse_config_request(Some(json), None).unwrap_err();
        assert!(err.contains("Unsupported legacy key"));
    }

    #[tokio::test]
    async fn test_app_state_new() {
        let work_dir = std::env::temp_dir().join("superbook_test_routes");
        let state = AppState::new(work_dir.clone(), 1);
        assert!(!state.version.is_empty());
        std::fs::remove_dir_all(&work_dir).ok();
    }

    #[test]
    fn test_tool_status_serialize() {
        let status = ToolStatus {
            poppler: true,
            tesseract: false,
            realesrgan: false,
            yomitoku: false,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"poppler\":true"));
        assert!(json.contains("\"tesseract\":false"));
        assert!(json.contains("\"yomitoku\":false"));
    }

    #[test]
    fn test_health_response_serialize() {
        let response = HealthResponse {
            status: "healthy".to_string(),
            version: "0.4.0".to_string(),
            tools: ToolStatus {
                poppler: true,
                tesseract: false,
                realesrgan: false,
                yomitoku: false,
            },
            ai_services: AiVersions {
                realesrgan: AiServiceVersion {
                    available: false,
                    service_version: None,
                    torch_version: None,
                    cuda_available: None,
                    device: None,
                },
                yomitoku: AiServiceVersion {
                    available: false,
                    service_version: None,
                    torch_version: None,
                    cuda_available: None,
                    device: None,
                },
            },
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"status\":\"healthy\""));
        assert!(json.contains("\"version\":\"0.4.0\""));
    }

    #[test]
    fn test_upload_response_serialize() {
        let id = Uuid::new_v4();
        let response = UploadResponse {
            job_id: id,
            status: "queued".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains(&id.to_string()));
        assert!(json.contains("\"status\":\"queued\""));
    }

    #[test]
    fn test_check_python_module_nonexistent() {
        // Should return false for nonexistent module
        let result = check_python_module("nonexistent_module_xyz_12345");
        assert!(!result);
    }

    #[test]
    fn test_check_python_module_builtin() {
        // If python is available, should return true for built-in modules
        if which::which("python3").is_ok() || which::which("python").is_ok() {
            let result = check_python_module("sys");
            assert!(result);
        }
    }

    // TC-BATCH-API-001: Batch response serialization
    #[test]
    fn test_batch_response_serialize() {
        let id = Uuid::new_v4();
        let response = BatchResponse {
            batch_id: id,
            status: "processing".to_string(),
            job_count: 5,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains(&id.to_string()));
        assert!(json.contains("\"status\":\"processing\""));
        assert!(json.contains("\"job_count\":5"));
    }

    // TC-BATCH-API-002: Batch status response serialization
    #[test]
    fn test_batch_status_response_serialize() {
        let id = Uuid::new_v4();
        let response = BatchStatusResponse {
            batch_id: id,
            status: "processing".to_string(),
            progress: BatchProgress {
                completed: 3,
                processing: 1,
                pending: 1,
                failed: 0,
                total: 5,
            },
            created_at: "2024-01-01T00:00:00Z".to_string(),
            started_at: Some("2024-01-01T00:00:10Z".to_string()),
            completed_at: None,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"completed\":3"));
        assert!(json.contains("\"total\":5"));
        assert!(json.contains("\"started_at\""));
        assert!(!json.contains("\"completed_at\""));
    }

    // TC-BATCH-API-003: Batch jobs response serialization
    #[test]
    fn test_batch_jobs_response_serialize() {
        let batch_id = Uuid::new_v4();
        let job_id = Uuid::new_v4();
        let response = BatchJobsResponse {
            batch_id,
            jobs: vec![BatchJobInfo {
                job_id,
                filename: "test.pdf".to_string(),
                status: "completed".to_string(),
                download_url: Some("/api/jobs/123/download".to_string()),
                progress: None,
            }],
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains(&job_id.to_string()));
        assert!(json.contains("\"filename\":\"test.pdf\""));
        assert!(json.contains("\"download_url\""));
    }

    // TC-BATCH-API-004: Batch cancel response serialization
    #[test]
    fn test_batch_cancel_response_serialize() {
        let id = Uuid::new_v4();
        let response = BatchCancelResponse {
            batch_id: id,
            status: "cancelled".to_string(),
            cancelled_jobs: 2,
            completed_jobs: 3,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"status\":\"cancelled\""));
        assert!(json.contains("\"cancelled_jobs\":2"));
        assert!(json.contains("\"completed_jobs\":3"));
    }

    // TC-BATCH-API-005: Job progress info serialization
    #[test]
    fn test_job_progress_info_serialize() {
        let info = JobProgressInfo {
            percent: 45,
            step_name: "Deskew".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"percent\":45"));
        assert!(json.contains("\"step_name\":\"Deskew\""));
    }

    // TC-BATCH-API-006: Batch request deserialization
    #[test]
    fn test_batch_request_deserialize() {
        let json = r#"{"options":{"dpi":300},"priority":"high"}"#;
        let request: BatchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.options.dpi, 300);
        assert_eq!(request.priority, Priority::High);
    }

    // TC-BATCH-API-007: Batch request with defaults
    #[test]
    fn test_batch_request_defaults() {
        let json = r#"{}"#;
        let request: BatchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.priority, Priority::Normal);
    }

    #[test]
    fn test_parse_convert_options_direct_object() {
        let json = r#"{"dpi":300,"deskew":true,"upscale":false,"ocr":true,"advanced":false}"#;
        let options = parse_convert_options(json).unwrap();
        assert_eq!(options.dpi, 300);
        assert!(options.deskew);
        assert!(!options.upscale);
        assert!(options.ocr);
        assert!(!options.advanced);
    }

    #[test]
    fn test_parse_convert_options_batch_wrapped_object() {
        let json = r#"{"options":{"dpi":300,"deskew":true,"upscale":false,"ocr":true,"advanced":false},"priority":"normal"}"#;
        let options = parse_convert_options(json).unwrap();
        assert_eq!(options.dpi, 300);
        assert!(options.deskew);
        assert!(!options.upscale);
        assert!(options.ocr);
        assert!(!options.advanced);
    }

    #[test]
    fn test_parse_convert_options_double_encoded_json_string() {
        let json = r#"\"{\\\"dpi\\\":300,\\\"deskew\\\":true,\\\"upscale\\\":false,\\\"ocr\\\":true,\\\"advanced\\\":false}\""#;
        let options = parse_convert_options(json).unwrap();
        assert_eq!(options.dpi, 300);
        assert!(options.deskew);
        assert!(!options.upscale);
        assert!(options.ocr);
        assert!(!options.advanced);
    }

    #[test]
    fn test_parse_convert_options_relaxed_object_literal() {
        let json = "{dpi:300,deskew:true,upscale:false,ocr:true,advanced:false}";
        let options = parse_convert_options(json).unwrap();
        assert_eq!(options.dpi, 300);
        assert!(options.deskew);
        assert!(!options.upscale);
        assert!(options.ocr);
        assert!(!options.advanced);
    }

    // TC-BATCH-API-008: AppState includes batch queue
    #[tokio::test]
    async fn test_app_state_has_batch_queue() {
        let work_dir = std::env::temp_dir().join("superbook_test_batch_state");
        let state = AppState::new(work_dir.clone(), 1);
        assert_eq!(state.batch_queue.active_count().await, 0);
        std::fs::remove_dir_all(&work_dir).ok();
    }

    // TC-RATE-001: AppState includes rate limiter
    #[tokio::test]
    async fn test_app_state_has_rate_limiter() {
        let work_dir = std::env::temp_dir().join("superbook_test_rate_state");
        let state = AppState::new(work_dir.clone(), 1);
        assert!(state.rate_limiter.is_enabled());
        std::fs::remove_dir_all(&work_dir).ok();
    }

    // TC-RATE-002: AppState with custom rate limit config
    #[tokio::test]
    async fn test_app_state_with_rate_limit() {
        let work_dir = std::env::temp_dir().join("superbook_test_rate_custom");
        let config = RateLimitConfig {
            requests_per_minute: 120,
            burst_size: 20,
            enabled: true,
            ..Default::default()
        };
        let state = AppState::new_with_rate_limit(work_dir.clone(), 1, config);
        assert_eq!(state.rate_limiter.requests_per_minute(), 120);
        assert_eq!(state.rate_limiter.burst_size(), 20);
        std::fs::remove_dir_all(&work_dir).ok();
    }

    // TC-RATE-003: Rate limit status serialization
    #[test]
    fn test_rate_limit_status_serialize() {
        let status = RateLimitStatus {
            enabled: true,
            requests_per_minute: 60,
            burst_size: 10,
            your_remaining: 55,
            reset_at: 1704067200,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"enabled\":true"));
        assert!(json.contains("\"requests_per_minute\":60"));
        assert!(json.contains("\"your_remaining\":55"));
    }

    // TC-RATE-004: Rate limit error serialization
    #[test]
    fn test_rate_limit_error_serialize() {
        let error = RateLimitError::new(60);
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("\"error\":\"rate_limit_exceeded\""));
        assert!(json.contains("\"retry_after\":60"));
    }

    // TC-RATE-005: Check rate limit allowed
    #[test]
    fn test_check_rate_limit_allowed() {
        let config = RateLimitConfig::default();
        let limiter = RateLimiter::new(config);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        let result = check_rate_limit(&limiter, ip);
        assert!(result.is_none()); // Request allowed
    }

    // TC-RATE-006: Check rate limit exceeded
    #[test]
    fn test_check_rate_limit_exceeded() {
        let config = RateLimitConfig {
            burst_size: 1,
            requests_per_minute: 1,
            ..Default::default()
        };
        let limiter = RateLimiter::new(config);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        // First request allowed
        let _ = check_rate_limit(&limiter, ip);

        // Second request limited
        let result = check_rate_limit(&limiter, ip);
        assert!(result.is_some());

        let (status, _headers, _body) = result.unwrap();
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    }

    // TC-RATE-007: AppError TooManyRequests
    #[test]
    fn test_app_error_too_many_requests() {
        let error = AppError::TooManyRequests { retry_after: 60 };
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    // TC-RATE-008: Disabled rate limiter always allows
    #[test]
    fn test_rate_limit_disabled() {
        let config = RateLimitConfig {
            enabled: false,
            burst_size: 1,
            ..Default::default()
        };
        let limiter = RateLimiter::new(config);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        // All requests allowed when disabled
        for _ in 0..100 {
            let result = check_rate_limit(&limiter, ip);
            assert!(result.is_none());
        }
    }

    // TC-AUTH-001: AppState includes auth manager
    #[tokio::test]
    async fn test_app_state_has_auth_manager() {
        let work_dir = std::env::temp_dir().join("superbook_test_auth_state");
        let state = AppState::new(work_dir.clone(), 1);
        assert!(!state.auth_manager.is_enabled()); // Default is disabled
        std::fs::remove_dir_all(&work_dir).ok();
    }

    // TC-AUTH-002: AppState with custom auth config
    #[tokio::test]
    async fn test_app_state_with_auth_config() {
        use crate::api_server::auth::ApiKey;

        let work_dir = std::env::temp_dir().join("superbook_test_auth_custom");
        let keys = vec![ApiKey::new("test-key", "Test")];
        let auth_config = AuthConfig::enabled_with_keys(keys);
        let state =
            AppState::new_with_config(work_dir.clone(), 1, RateLimitConfig::default(), auth_config);
        assert!(state.auth_manager.is_enabled());
        assert_eq!(state.auth_manager.key_count(), 1);
        std::fs::remove_dir_all(&work_dir).ok();
    }

    // TC-AUTH-003: Auth status response serialization
    #[test]
    fn test_auth_status_response_serialize() {
        use crate::api_server::auth::Scope;
        let response = AuthStatusResponse::authenticated(
            "my-key".to_string(),
            vec![Scope::Read, Scope::Write],
        );
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"authenticated\":true"));
        assert!(json.contains("\"key_name\":\"my-key\""));
    }
}
