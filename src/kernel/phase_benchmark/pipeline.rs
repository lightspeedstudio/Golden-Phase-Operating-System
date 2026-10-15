//! Full KairOS pipeline timing (compute + trajectory + proof + Merkle + statechain).

use crate::core::gphase_core::PhaseState;
use crate::kernel::hash::tagged_hash;
use crate::kernel::hash::StateHash;
use crate::kernel::phase_benchmark::bigint;
use crate::kernel::phase_benchmark::engines::{self, EngineOutput};
use crate::kernel::phase_benchmark::types::FullPipelineTiming;
use crate::kernel::Kernel;
use crate::platform::Vec;
use crate::stateclock::{record_kairos_on_kernel, KairosEventKind};

pub const STATECHAIN_OBJECT_ID: u32 = 8;

const STEP_TAG: &str = "KAIROS_PHASE_BENCHMARK_STEP_V1";
const TRAJECTORY_TAG: &str = "KAIROS_PHASE_BENCHMARK_TRAJ_V1";

pub struct PipelineResult {
    pub output: EngineOutput,
    pub timing: FullPipelineTiming,
    pub trajectory_root: StateHash,
    pub kairos_events: u64,
}

pub fn run_full_pipeline(kernel: &mut Kernel, target: u64) -> PipelineResult {
    let tables = kernel.phase_math.tables.clone();

    let compute_start = instant_now();
    let output = engines::fib_kairos_phase_doubling(target, &tables);
    let compute_ns = instant_elapsed(compute_start);

    let trajectory_start = instant_now();
    let (trajectory_root, step_count) = build_trajectory(target, &output);
    let trajectory_ns = instant_elapsed(trajectory_start);

    let proof_start = instant_now();
    let proof_root = tagged_hash(
        "KAIROS_PHASE_BENCHMARK_PROOF_V1",
        &[&trajectory_root, &bigint::hash_value(&output.current)],
    );
    let proof_ns = instant_elapsed(proof_start);

    let merkle_start = instant_now();
    let merkle_root =
        crate::kernel::phi_stability::merkle::merkle_root(&[trajectory_root, proof_root]);
    let merkle_ns = instant_elapsed(merkle_start);

    let statechain_start = instant_now();
    let _ = kernel.statechain.record_transition(
        crate::kernel::state_proof::ObjectKind::Driver,
        STATECHAIN_OBJECT_ID,
        crate::kernel::state_root::SubsystemKind::Driver,
        PhaseState::Phi2,
        PhaseState::Phi3,
        "phase_benchmark",
        None,
        &target.to_be_bytes(),
    );
    let statechain_ns = instant_elapsed(statechain_start);

    record_kairos_on_kernel(kernel, KairosEventKind::Other, "phase-benchmark");

    let total_ns = compute_ns + trajectory_ns + proof_ns + merkle_ns + statechain_ns;
    let kairos_events = output.counters.kairos_events + step_count;

    PipelineResult {
        output,
        timing: FullPipelineTiming {
            compute_ns,
            trajectory_ns,
            proof_ns,
            merkle_ns,
            statechain_ns,
            total_ns,
        },
        trajectory_root: merkle_root,
        kairos_events,
    }
}

pub fn build_trajectory(target: u64, output: &EngineOutput) -> (StateHash, u64) {
    let f_hash = bigint::hash_value(&output.current);
    let f1_hash = bigint::hash_value(&output.next);
    let bit_len = output.counters.recurrence_levels;
    let mut leaves = Vec::new();
    let mut prev = [0u8; 32];
    for step in 0..=bit_len {
        let bit = if step == 0 {
            0u8
        } else {
            ((target >> (bit_len - step)) & 1) as u8
        };
        let leaf = tagged_hash(
            STEP_TAG,
            &[
                &target.to_be_bytes(),
                &step.to_be_bytes(),
                &[bit],
                &f_hash,
                &f1_hash,
                &prev,
            ],
        );
        leaves.push(leaf);
        prev = leaf;
    }
    let root = tagged_hash(TRAJECTORY_TAG, &[&leaves.len().to_be_bytes(), &prev]);
    (root, leaves.len() as u64)
}

#[cfg(feature = "std")]
type BenchInstant = std::time::Instant;

#[cfg(feature = "std")]
fn instant_now() -> BenchInstant {
    std::time::Instant::now()
}

#[cfg(feature = "std")]
fn instant_elapsed(start: BenchInstant) -> u128 {
    start.elapsed().as_nanos()
}
