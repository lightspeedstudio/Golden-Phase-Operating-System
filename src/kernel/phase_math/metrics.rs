//! Runtime metrics and renderers.

use crate::platform::String;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueDepths {
    pub transitions: usize,
    pub trajectory: usize,
    pub proofs: usize,
    pub merkle: usize,
    pub network: usize,
    pub watchdog: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseMathMetrics {
    pub backend_name: &'static str,
    pub scalar_equivalence_ok: bool,
    pub worker_count: usize,
    pub queue_depths: QueueDepths,
    pub processed_batches: u64,
    pub processed_phase_ops: u64,
    pub processed_topology_ops: u64,
}

impl Default for PhaseMathMetrics {
    fn default() -> Self {
        Self {
            backend_name: "scalar",
            scalar_equivalence_ok: true,
            worker_count: 1,
            queue_depths: QueueDepths {
                transitions: 0,
                trajectory: 0,
                proofs: 0,
                merkle: 0,
                network: 0,
                watchdog: 0,
            },
            processed_batches: 0,
            processed_phase_ops: 0,
            processed_topology_ops: 0,
        }
    }
}

pub fn render_status(metrics: &PhaseMathMetrics) -> String {
    format!(
        "phase math: enabled\nactive backend: {}\nworkers: {}\nscalar equivalence: {}\nprocessed batches: {}\nphase ops: {}\ntopology ops: {}\n",
        metrics.backend_name,
        metrics.worker_count,
        if metrics.scalar_equivalence_ok { "OK" } else { "FAIL" },
        metrics.processed_batches,
        metrics.processed_phase_ops,
        metrics.processed_topology_ops,
    )
}

pub fn render_queues(metrics: &PhaseMathMetrics) -> String {
    format!(
        "transitions: {}\ntrajectory: {}\nproofs: {}\nmerkle: {}\nnetwork: {}\nwatchdog: {}\n",
        metrics.queue_depths.transitions,
        metrics.queue_depths.trajectory,
        metrics.queue_depths.proofs,
        metrics.queue_depths.merkle,
        metrics.queue_depths.network,
        metrics.queue_depths.watchdog,
    )
}
