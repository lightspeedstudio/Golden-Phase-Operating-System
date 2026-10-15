//! Worker ownership and concurrency descriptors.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerConfig {
    pub active_worker_count: usize,
    pub trajectory_workers_ready: bool,
    pub proof_workers_ready: bool,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            active_worker_count: 1,
            trajectory_workers_ready: true,
            proof_workers_ready: true,
        }
    }
}

pub fn object_worker(object_id: u32, active_worker_count: usize) -> usize {
    if active_worker_count == 0 {
        0
    } else {
        object_id as usize % active_worker_count
    }
}

#[cfg(feature = "std")]
pub fn detect_workers() -> WorkerConfig {
    let count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    WorkerConfig {
        active_worker_count: count.max(1),
        trajectory_workers_ready: true,
        proof_workers_ready: true,
    }
}

#[cfg(not(feature = "std"))]
pub fn detect_workers() -> WorkerConfig {
    WorkerConfig::default()
}
