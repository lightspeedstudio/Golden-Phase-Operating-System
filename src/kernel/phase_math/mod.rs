//! Parallel Phase Math Engine.

pub mod backends {
    pub mod avx2;
    pub mod avx512;
    pub mod fpga;
    pub mod gpu;
    pub mod neon;
    pub mod scalar;
}
pub mod batch;
pub mod clock_math;
pub mod merkle_pipeline;
pub mod metrics;
pub mod packed;
pub mod proof_pipeline;
pub mod reference;
pub mod tables;
pub mod trajectory;
pub mod workers;

use crate::core::gphase_core::PhaseState;
use crate::kernel::phase_math::batch::{
    BatchLane, PhaseBatchInput, PhaseBatchOutput, PhaseOp, TopologyOp,
};
use crate::kernel::phase_math::clock_math::ClockWorkItem;
use crate::kernel::phase_math::merkle_pipeline::{MerkleLeafWorkItem, MerklePipeline};
use crate::kernel::phase_math::metrics::{PhaseMathMetrics, QueueDepths};
use crate::kernel::phase_math::proof_pipeline::{ProofPipeline, ProofWorkItem};
use crate::kernel::phase_math::tables::PhaseMathTables;
use crate::kernel::phase_math::workers::{detect_workers, object_worker, WorkerConfig};
use crate::platform::Vec;

const TRANSITION_QUEUE_CAPACITY: usize = 256;
const TRAJECTORY_QUEUE_CAPACITY: usize = 256;
const NETWORK_QUEUE_CAPACITY: usize = 256;
const WATCHDOG_QUEUE_CAPACITY: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseMathBackend {
    Scalar,
    Avx2,
    Avx512,
    Neon,
    Gpu,
    Fpga,
}

impl PhaseMathBackend {
    pub fn label(self) -> &'static str {
        match self {
            Self::Scalar => "scalar",
            Self::Avx2 => "avx2",
            Self::Avx512 => "avx512",
            Self::Neon => "neon",
            Self::Gpu => "gpu",
            Self::Fpga => "fpga",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionWorkItem {
    pub object_id: u32,
    pub from: PhaseState,
    pub to: PhaseState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchdogWorkItem {
    pub object_id: u32,
}

#[derive(Debug, Clone, Default)]
pub struct PhaseMathController {
    pub tables: PhaseMathTables,
    pub backend: PhaseMathBackend,
    pub workers: WorkerConfig,
    pub metrics: PhaseMathMetrics,
    pub transition_queue: Vec<TransitionWorkItem>,
    pub trajectory_queue: Vec<TopologyOp>,
    pub proof_pipeline: ProofPipeline,
    pub merkle_pipeline: MerklePipeline,
    pub network_queue: Vec<u32>,
    pub watchdog_queue: Vec<WatchdogWorkItem>,
    pub clock_queue: Vec<ClockWorkItem>,
}

impl PhaseMathController {
    pub fn new() -> Self {
        let tables = PhaseMathTables::build();
        let workers = detect_workers();
        let backend = select_backend();
        let metrics = PhaseMathMetrics {
            backend_name: backend.label(),
            worker_count: workers.active_worker_count,
            scalar_equivalence_ok: reference::verify_tables(&tables),
            ..PhaseMathMetrics::default()
        };
        Self {
            tables,
            backend,
            workers,
            metrics,
            transition_queue: Vec::with_capacity(TRANSITION_QUEUE_CAPACITY),
            trajectory_queue: Vec::with_capacity(TRAJECTORY_QUEUE_CAPACITY),
            proof_pipeline: ProofPipeline::default(),
            merkle_pipeline: MerklePipeline::default(),
            network_queue: Vec::with_capacity(NETWORK_QUEUE_CAPACITY),
            watchdog_queue: Vec::with_capacity(WATCHDOG_QUEUE_CAPACITY),
            clock_queue: Vec::with_capacity(TRAJECTORY_QUEUE_CAPACITY),
        }
    }

    pub fn process_batch(&mut self, input: &PhaseBatchInput) -> PhaseBatchOutput {
        let out = reference::process_batch(&self.tables, input);
        self.metrics.processed_batches += 1;
        self.metrics.processed_phase_ops += input.phase_ops.len() as u64;
        self.metrics.processed_topology_ops += input.topology_ops.len() as u64;
        self.refresh_queue_depths();
        out
    }

    pub fn force_backend(&mut self, backend: PhaseMathBackend) {
        self.backend = backend;
        self.metrics.backend_name = backend.label();
    }

    pub fn verify_scalar_equivalence(&mut self) -> bool {
        self.metrics.scalar_equivalence_ok = reference::verify_tables(&self.tables);
        self.metrics.scalar_equivalence_ok
    }

    pub fn enqueue_transition(&mut self, object_id: u32, from: PhaseState, to: PhaseState) {
        bounded_push(
            &mut self.transition_queue,
            TRANSITION_QUEUE_CAPACITY,
            TransitionWorkItem {
                object_id,
                from,
                to,
            },
        );
        self.refresh_queue_depths();
    }

    pub fn enqueue_trajectory(&mut self, a: PhaseState, b: PhaseState, c: PhaseState) {
        bounded_push(
            &mut self.trajectory_queue,
            TRAJECTORY_QUEUE_CAPACITY,
            TopologyOp { a, b, c },
        );
        self.refresh_queue_depths();
    }

    pub fn enqueue_clock(&mut self, proper_tick: u64, kairos_tick: u64, sector_id: u8) {
        bounded_push(
            &mut self.clock_queue,
            TRAJECTORY_QUEUE_CAPACITY,
            ClockWorkItem {
                proper_tick,
                kairos_tick,
                sector_id,
            },
        );
        self.refresh_queue_depths();
    }

    pub fn enqueue_proof(&mut self, object_id: u32, sequence_hint: u64) {
        self.proof_pipeline.push(ProofWorkItem {
            object_id,
            sequence_hint,
        });
        self.refresh_queue_depths();
    }

    pub fn enqueue_merkle_leaf(&mut self, object_id: u32) {
        self.merkle_pipeline.push(MerkleLeafWorkItem { object_id });
        self.refresh_queue_depths();
    }

    pub fn enqueue_network(&mut self, object_id: u32) {
        bounded_push(&mut self.network_queue, NETWORK_QUEUE_CAPACITY, object_id);
        self.refresh_queue_depths();
    }

    pub fn enqueue_watchdog(&mut self, object_id: u32) {
        bounded_push(
            &mut self.watchdog_queue,
            WATCHDOG_QUEUE_CAPACITY,
            WatchdogWorkItem { object_id },
        );
        self.refresh_queue_depths();
    }

    pub fn benchmark(&mut self) -> PhaseBatchOutput {
        let batch = PhaseBatchInput {
            lane: Some(BatchLane::Immediate),
            phase_ops: vec![
                PhaseOp {
                    left: PhaseState::Phi0,
                    right: PhaseState::Phi1,
                },
                PhaseOp {
                    left: PhaseState::Phi2,
                    right: PhaseState::Phi3,
                },
            ],
            topology_ops: vec![TopologyOp {
                a: PhaseState::Phi1,
                b: PhaseState::Phi2,
                c: PhaseState::Phi3,
            }],
        };
        self.process_batch(&batch)
    }

    pub fn backend_report(&self) -> crate::platform::String {
        format!(
            "scalar reference: ready\nworker cores: {}\nSIMD capability: {}\nactive backend: {}\ntrajectory workers: {}\nproof workers: {}\nFPGA accelerator: {}\nscalar equivalence: {}\n",
            self.workers.active_worker_count,
            simd_label(),
            self.backend.label().to_ascii_uppercase(),
            if self.workers.trajectory_workers_ready { "ready" } else { "offline" },
            if self.workers.proof_workers_ready { "ready" } else { "offline" },
            if backends::fpga::available() { "present" } else { "not present" },
            if self.metrics.scalar_equivalence_ok { "OK" } else { "FAIL" },
        )
    }

    pub fn object_owner(&self, object_id: u32) -> usize {
        object_worker(object_id, self.workers.active_worker_count)
    }

    fn refresh_queue_depths(&mut self) {
        self.metrics.queue_depths = QueueDepths {
            transitions: self.transition_queue.len(),
            trajectory: self.trajectory_queue.len() + self.clock_queue.len(),
            proofs: self.proof_pipeline.queue.len(),
            merkle: self.merkle_pipeline.queue.len(),
            network: self.network_queue.len(),
            watchdog: self.watchdog_queue.len(),
        };
    }
}

impl Default for PhaseMathBackend {
    fn default() -> Self {
        select_backend()
    }
}

fn bounded_push<T>(queue: &mut Vec<T>, cap: usize, item: T) {
    if queue.len() >= cap {
        queue.remove(0);
    }
    queue.push(item);
}

fn select_backend() -> PhaseMathBackend {
    if backends::fpga::available() {
        PhaseMathBackend::Fpga
    } else if backends::gpu::available() {
        PhaseMathBackend::Gpu
    } else if backends::avx512::available() {
        PhaseMathBackend::Avx512
    } else if backends::avx2::available() {
        PhaseMathBackend::Avx2
    } else if backends::neon::available() {
        PhaseMathBackend::Neon
    } else {
        PhaseMathBackend::Scalar
    }
}

fn simd_label() -> &'static str {
    if backends::avx512::available() {
        backends::avx512::backend_name()
    } else if backends::avx2::available() {
        backends::avx2::backend_name()
    } else if backends::neon::available() {
        backends::neon::backend_name()
    } else {
        "none"
    }
}
