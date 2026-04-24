use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct CpuQueueConfig {
    pub min_in_flight: usize,
    pub target_load_per_core: f64,
    pub status_poll_ms: u64,
}

impl CpuQueueConfig {
    pub fn normalized(&self, max_in_flight: usize) -> Self {
        let max_in_flight = max_in_flight.max(1);
        let min_in_flight = self.min_in_flight.max(1).min(max_in_flight);
        let target_load_per_core = if self.target_load_per_core.is_finite()
            && self.target_load_per_core > 0.1
        {
            self.target_load_per_core
        } else {
            0.9
        };

        Self {
            min_in_flight,
            target_load_per_core,
            status_poll_ms: self.status_poll_ms.max(20),
        }
    }
}

#[derive(Debug)]
pub struct CpuDynamicLimiter {
    max_in_flight: usize,
    min_in_flight: usize,
    target_load_per_core: f64,
    poll_interval: Duration,
    next_refresh: Instant,
    cached_limit: usize,
}

impl CpuDynamicLimiter {
    pub fn new(max_in_flight: usize, config: CpuQueueConfig) -> Self {
        let max_in_flight = max_in_flight.max(1);
        let cfg = config.normalized(max_in_flight);
        Self {
            max_in_flight,
            min_in_flight: cfg.min_in_flight,
            target_load_per_core: cfg.target_load_per_core,
            poll_interval: Duration::from_millis(cfg.status_poll_ms),
            next_refresh: Instant::now(),
            cached_limit: max_in_flight,
        }
    }

    pub fn current_limit(&mut self) -> usize {
        let now = Instant::now();
        if now < self.next_refresh {
            return self.cached_limit;
        }

        self.cached_limit = self.compute_limit();
        self.next_refresh = now + self.poll_interval;
        self.cached_limit
    }

    fn compute_limit(&self) -> usize {
        let Some(load_per_core) = read_load_per_core() else {
            return self.max_in_flight;
        };

        if load_per_core <= self.target_load_per_core {
            return self.max_in_flight;
        }

        let scale = (self.target_load_per_core / load_per_core).clamp(0.2, 1.0);
        let dynamic = ((self.max_in_flight as f64) * scale).floor() as usize;
        dynamic.max(self.min_in_flight).min(self.max_in_flight)
    }
}

fn read_load_per_core() -> Option<f64> {
    let text = std::fs::read_to_string("/proc/loadavg").ok()?;
    let load_1m = text.split_whitespace().next()?.parse::<f64>().ok()?;
    let cpu_count = std::thread::available_parallelism().ok()?.get() as f64;
    if cpu_count <= 0.0 {
        return None;
    }
    Some(load_1m / cpu_count)
}
