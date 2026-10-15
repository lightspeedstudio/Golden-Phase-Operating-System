//! Phase-state benchmark types.

use crate::kernel::hash::StateHash;
use crate::platform::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineKind {
    BinaryLinear,
    BinaryFastDoubling,
    KairosPhaseDoubling,
}

impl EngineKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::BinaryLinear => "BINARY-LINEAR",
            Self::BinaryFastDoubling => "BINARY-FAST-DOUBLING",
            Self::KairosPhaseDoubling => "KAIROS-PHASE-DOUBLING",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct EngineCounters {
    pub recurrence_levels: u32,
    pub additions: u64,
    pub subtractions: u64,
    pub multiplications: u64,
    pub branches: u64,
    pub phase_lookups: u64,
    pub compose_lookups: u64,
    pub reflection_lookups: u64,
    pub topology_lookups: u64,
    pub chi_lookups: u64,
    pub transition_lookups: u64,
    pub dynamic_branches: u64,
    pub state_transitions: u64,
    pub allocations: u64,
    pub allocated_bytes: u64,
    pub kairos_events: u64,
    pub trajectory_records: u64,
}

#[derive(Debug, Clone)]
pub struct TimingSample {
    pub compute_ns: u128,
    pub full_ns: u128,
    pub counters: EngineCounters,
}

#[derive(Debug, Clone)]
pub struct TimingStats {
    pub samples_ns: Vec<u128>,
    pub median_ns: u128,
    pub min_ns: u128,
    pub max_ns: u128,
    pub stddev_ns: f64,
    pub cv_percent: f64,
}

#[derive(Debug, Clone)]
pub struct EngineRunResult {
    pub engine: EngineKind,
    pub target: u64,
    pub digit_count: u64,
    pub result_hash: StateHash,
    pub compute_stats: TimingStats,
    pub full_stats: Option<TimingStats>,
    pub counters: EngineCounters,
}

#[derive(Debug, Clone)]
pub struct CompareVerdict {
    pub label: &'static str,
    pub speedup: f64,
    pub time_reduction_percent: f64,
    pub allocation_reduction_percent: f64,
}

#[derive(Debug, Clone, Default)]
pub struct FullPipelineTiming {
    pub compute_ns: u128,
    pub trajectory_ns: u128,
    pub proof_ns: u128,
    pub merkle_ns: u128,
    pub statechain_ns: u128,
    pub total_ns: u128,
}

#[derive(Debug, Clone)]
pub struct ScaleReport {
    pub engine: EngineKind,
    pub base_target: u64,
    pub worker_count: usize,
    pub jobs: u32,
    pub single_job_ns: u128,
    pub total_ns: u128,
    pub jobs_per_second: f64,
    pub sync_operations: u64,
    pub efficiency: f64,
}

#[derive(Debug, Clone)]
pub struct ScaleCompareReport {
    pub target: u64,
    pub worker_count: usize,
    pub binary_fast: ScaleReport,
    pub kairos_phase: ScaleReport,
    pub result_equality: bool,
}

#[derive(Debug, Clone)]
pub struct CompareReport {
    pub target: u64,
    pub digit_count: u64,
    pub repetitions: u32,
    pub result_hash: StateHash,
    pub result_equality: bool,
    pub binary_linear: Option<EngineRunResult>,
    pub binary_fast: EngineRunResult,
    pub kairos_phase: EngineRunResult,
    pub verdict: CompareVerdict,
    pub full_pipeline: Option<FullPipelineTiming>,
}

#[derive(Debug, Default)]
pub struct PhaseBenchmarkController {
    pub last_report: Option<CompareReport>,
    pub runs_completed: u64,
    pub default_repetitions: u32,
    pub last_target: u64,
    pub last_result_hash: StateHash,
}

impl PhaseBenchmarkController {
    pub fn new() -> Self {
        Self {
            default_repetitions: 20,
            ..Self::default()
        }
    }
}

pub const MIN_REPETITIONS: u32 = 20;
pub const PAIR_A_LUT: [usize; 2] = [0, 1];
pub const PAIR_B_LUT: [usize; 2] = [1, 2];
