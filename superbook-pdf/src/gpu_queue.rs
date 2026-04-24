use crate::stage::StageError;
use serde::Deserialize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

#[derive(Debug, Clone)]
pub struct GpuQueueConfig {
    pub max_in_flight: usize,
    pub safety_margin_mb: u64,
    pub status_poll_ms: u64,
}

impl GpuQueueConfig {
    pub fn normalized(&self) -> Self {
        Self {
            max_in_flight: self.max_in_flight.max(1),
            safety_margin_mb: self.safety_margin_mb.max(1),
            status_poll_ms: self.status_poll_ms.max(20),
        }
    }
}

#[derive(Debug)]
pub struct GpuJobQueue {
    stage_name: &'static str,
    status_url: String,
    max_in_flight: usize,
    safety_margin_mb: f64,
    poll_interval: Duration,
    in_flight: AtomicUsize,
    notify: Notify,
    client: reqwest::Client,
}

#[derive(Debug)]
pub struct GpuQueuePermit {
    queue: Arc<GpuJobQueue>,
}

impl Drop for GpuQueuePermit {
    fn drop(&mut self) {
        self.queue.in_flight.fetch_sub(1, Ordering::SeqCst);
        self.queue.notify.notify_one();
    }
}

#[derive(Debug, Deserialize)]
struct GpuStatusResponse {
    #[serde(default)]
    active_requests: Option<usize>,
    #[serde(default)]
    active_inference: Option<usize>,
    #[serde(default)]
    upsampler_pool_size: Option<usize>,
    #[serde(default)]
    gpu_memory_free: Option<f64>,
    #[serde(default)]
    gpu_memory_total: Option<f64>,
    #[serde(default)]
    gpu_memory_used: Option<f64>,
    #[serde(default)]
    measured_inference_mb: Option<f64>,
    #[serde(default)]
    gpu: Option<GpuInfo>,
}

#[derive(Debug, Deserialize)]
struct GpuInfo {
    #[serde(default)]
    memory_allocated_mb: Option<f64>,
    #[serde(default)]
    memory_reserved_mb: Option<f64>,
    #[serde(default)]
    memory_total_mb: Option<f64>,
}

impl GpuJobQueue {
    pub fn new(
        stage_name: &'static str,
        base_url: impl Into<String>,
        config: GpuQueueConfig,
    ) -> Arc<Self> {
        let cfg = config.normalized();
        let status_url = format!("{}/status", base_url.into().trim_end_matches('/'));
        Arc::new(Self {
            stage_name,
            status_url,
            max_in_flight: cfg.max_in_flight,
            safety_margin_mb: cfg.safety_margin_mb as f64,
            poll_interval: Duration::from_millis(cfg.status_poll_ms),
            in_flight: AtomicUsize::new(0),
            notify: Notify::new(),
            client: reqwest::Client::new(),
        })
    }

    pub async fn acquire(self: &Arc<Self>) -> Result<GpuQueuePermit, StageError> {
        loop {
            while self.in_flight.load(Ordering::SeqCst) >= self.max_in_flight {
                self.notify.notified().await;
            }

            let status = self.fetch_status().await?;
            let dynamic_cap = match self.dispatch_cap(&status) {
                Some(cap) => cap,
                None => {
                    tokio::time::sleep(self.poll_interval).await;
                    continue;
                }
            };

            if self.try_reserve_slot(dynamic_cap) {
                return Ok(GpuQueuePermit {
                    queue: Arc::clone(self),
                });
            } else {
                tokio::time::sleep(self.poll_interval).await;
                continue;
            }
        }
    }

    async fn fetch_status(&self) -> Result<GpuStatusResponse, StageError> {
        let resp = self
            .client
            .get(&self.status_url)
            .send()
            .await
            .map_err(|e| StageError::AiService {
                stage: self.stage_name,
                message: format!("GPU queue status request failed: {}", e),
            })?;

        if !resp.status().is_success() {
            return Err(StageError::AiService {
                stage: self.stage_name,
                message: format!("GPU queue status returned HTTP {}", resp.status()),
            });
        }

        resp.json::<GpuStatusResponse>()
            .await
            .map_err(|e| StageError::AiService {
                stage: self.stage_name,
                message: format!("GPU queue status decode failed: {}", e),
            })
    }

    fn dispatch_cap(&self, status: &GpuStatusResponse) -> Option<usize> {
        let free_mb = match gpu_free_mb(status) {
            Some(free) => free,
            None => return None,
        };
        if free_mb <= self.safety_margin_mb {
            return None;
        }

        let dynamic_cap = self.dynamic_in_flight_cap(status, free_mb);

        if let Some(active) = effective_active_inference(status) {
            if active >= dynamic_cap {
                return None;
            }
        }

        let local = self.in_flight.load(Ordering::SeqCst);
        if local >= dynamic_cap {
            return None;
        }

        Some(dynamic_cap)
    }

    fn dynamic_in_flight_cap(&self, status: &GpuStatusResponse, free_mb: f64) -> usize {
        let active = effective_active_inference(status).unwrap_or(0);
        let mut cap = self.max_in_flight;

        if let Some(pool) = status.upsampler_pool_size {
            cap = cap.min(pool.max(1));
        }

        // Estimate additional safe slots from current free VRAM.
        let additional_free = (free_mb - self.safety_margin_mb).max(0.0);
        let per_inference_mb = per_inference_mb(status);
        let additional_slots = (additional_free / per_inference_mb).floor().max(0.0) as usize;
        let memory_cap = (active + additional_slots).max(1);

        cap.min(memory_cap).max(1)
    }

    fn try_reserve_slot(&self, cap: usize) -> bool {
        loop {
            let current = self.in_flight.load(Ordering::SeqCst);
            if current >= cap {
                return false;
            }
            if self
                .in_flight
                .compare_exchange(current, current + 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return true;
            }
        }
    }
}

fn effective_active_inference(status: &GpuStatusResponse) -> Option<usize> {
    status.active_inference.or(status.active_requests)
}

fn gpu_used_mb(status: &GpuStatusResponse) -> Option<f64> {
    if let Some(used) = status.gpu_memory_used {
        return Some(used.max(0.0));
    }

    let gpu = status.gpu.as_ref()?;
    if let Some(reserved) = gpu.memory_reserved_mb {
        return Some(reserved.max(0.0));
    }
    gpu.memory_allocated_mb.map(|v| v.max(0.0))
}

fn per_inference_mb(status: &GpuStatusResponse) -> f64 {
    if let Some(measured) = status.measured_inference_mb {
        if measured > 0.0 {
            return measured.clamp(256.0, 8192.0);
        }
    }

    let default_mb = 1200.0;
    let active = effective_active_inference(status).unwrap_or(0);
    if active == 0 {
        return default_mb;
    }

    match gpu_used_mb(status) {
        Some(used) if used > 0.0 => {
            // Add 25% safety factor to observed usage per running inference.
            ((used / active as f64) * 1.25).clamp(512.0, 4096.0)
        }
        _ => default_mb,
    }
}

fn gpu_free_mb(status: &GpuStatusResponse) -> Option<f64> {
    if let Some(free) = status.gpu_memory_free {
        return Some(free);
    }

    if let (Some(total), Some(used)) = (status.gpu_memory_total, status.gpu_memory_used) {
        return Some((total - used).max(0.0));
    }

    let gpu = status.gpu.as_ref()?;
    let total = gpu.memory_total_mb?;
    if let Some(reserved) = gpu.memory_reserved_mb {
        return Some((total - reserved).max(0.0));
    }
    gpu.memory_allocated_mb
        .map(|allocated| (total - allocated).max(0.0))
}