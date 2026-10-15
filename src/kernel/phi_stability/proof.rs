//! PHI-STABILITY commitments and proofs.

use crate::kernel::hash::tagged_hash;
use crate::kernel::hash::StateHash;
use crate::kernel::phi_stability::fixed::U1024;
use crate::kernel::phi_stability::merkle::merkle_root;
use crate::kernel::phi_stability::types::{PhiIntegerMode, PhiStepRecord, PhiTestId};
use crate::platform::Vec;

pub const RESULT_TAG: &str = "KAIROS_PHI_STABILITY_RESULT_V1";
pub const PAIR_TAG: &str = "KAIROS_PHI_STABILITY_PAIR_V1";
pub const STEP_TAG: &str = "KAIROS_PHI_STABILITY_STEP_V1";
pub const CLOCK_TAG: &str = "KAIROS_PHI_STABILITY_CLOCK_V1";
pub const TEST_TAG: &str = "KAIROS_PHI_STABILITY_TEST_V1";

pub fn hash_fib_value(value: &U1024) -> StateHash {
    let bytes = value.to_be_bytes();
    tagged_hash("KAIROS_PHI_STABILITY_FIB_V1", &[&bytes])
}

pub fn pair_hash(f_n: &StateHash, f_n_plus_1: &StateHash) -> StateHash {
    tagged_hash(PAIR_TAG, &[f_n, f_n_plus_1])
}

pub fn result_hash(target_index: u64, integer_mode: PhiIntegerMode, value: &U1024) -> StateHash {
    let mode_byte = match integer_mode {
        PhiIntegerMode::ExactHosted => 0u8,
        PhiIntegerMode::FixedWidthBaremetal { bits } => {
            let hi = (bits >> 8) as u8;
            let lo = (bits & 0xff) as u8;
            return tagged_hash(
                RESULT_TAG,
                &[
                    &target_index.to_be_bytes(),
                    &[1, hi, lo],
                    &value.to_be_bytes(),
                ],
            );
        }
        PhiIntegerMode::ModularBaremetal => 2u8,
    };
    tagged_hash(
        RESULT_TAG,
        &[
            &target_index.to_be_bytes(),
            &[mode_byte],
            &value.to_be_bytes(),
        ],
    )
}

pub fn hash_pair_commitment(f_n: &StateHash, f_n_plus_1: &StateHash) -> StateHash {
    pair_hash(f_n, f_n_plus_1)
}

pub fn step_hash(record: &PhiStepRecord) -> StateHash {
    tagged_hash(
        STEP_TAG,
        &[
            &record.test_id.0.to_be_bytes(),
            &record.step_index.to_be_bytes(),
            &[record.input_bit as u8],
            &[record.previous_phase.as_u8()],
            &[record.next_phase.as_u8()],
            &[match record.topology_relation {
                crate::kernel::phi_stability::types::TopologyRelation::Same => 0,
                crate::kernel::phi_stability::types::TopologyRelation::Reflection => 1,
            }],
            &[record.chi as u8],
            &record.chronos_tick.to_be_bytes(),
            &record.kairos_tick.to_be_bytes(),
            &[record.stateclock_sector as u8],
            &record.stateclock_epsilon.to_be_bytes(),
            &record.pair_commitment,
            &record.previous_step_hash,
        ],
    )
}

pub fn clock_snapshot_hash(
    chronos_tick: u64,
    kairos_tick: u64,
    sector: i8,
    epsilon: i64,
) -> StateHash {
    tagged_hash(
        CLOCK_TAG,
        &[
            &chronos_tick.to_be_bytes(),
            &kairos_tick.to_be_bytes(),
            &[sector as u8],
            &epsilon.to_be_bytes(),
        ],
    )
}

pub fn trajectory_root(step_hashes: &[StateHash]) -> StateHash {
    merkle_root(step_hashes)
}

pub fn clock_root(clock_hashes: &[StateHash]) -> StateHash {
    merkle_root(clock_hashes)
}

pub fn test_root(
    equation_root: &StateHash,
    boot_session_root: &StateHash,
    test_id: PhiTestId,
    target_index: u64,
    result_hash: &StateHash,
    pair_hash: &StateHash,
    trajectory_root: &StateHash,
    clock_root: &StateHash,
    phase_transition_count: u64,
    kairos_event_count: u64,
) -> StateHash {
    tagged_hash(
        TEST_TAG,
        &[
            equation_root,
            boot_session_root,
            &test_id.0.to_be_bytes(),
            &target_index.to_be_bytes(),
            result_hash,
            pair_hash,
            trajectory_root,
            clock_root,
            &phase_transition_count.to_be_bytes(),
            &kairos_event_count.to_be_bytes(),
        ],
    )
}

pub fn state_event_hash(
    operation: &str,
    test_id: PhiTestId,
    target_index: u64,
    chronos_tick: u64,
    kairos_tick: u64,
    previous_event_hash: &StateHash,
) -> StateHash {
    tagged_hash(
        "KAIROS_PHI_STABILITY_EVENT_V1",
        &[
            operation.as_bytes(),
            &test_id.0.to_be_bytes(),
            &target_index.to_be_bytes(),
            &chronos_tick.to_be_bytes(),
            &kairos_tick.to_be_bytes(),
            previous_event_hash,
        ],
    )
}

pub fn verify_step_chain(steps: &[PhiStepRecord]) -> bool {
    let mut prev = [0u8; 32];
    for step in steps {
        if step.previous_step_hash != prev {
            return false;
        }
        let expected = step_hash(step);
        if step.step_hash != expected {
            return false;
        }
        prev = step.step_hash;
    }
    true
}

pub fn collect_step_hashes(steps: &[PhiStepRecord]) -> Vec<StateHash> {
    steps.iter().map(|s| s.step_hash).collect()
}

pub fn collect_clock_hashes(snapshots: &[(u64, u64, i8, i64)]) -> Vec<StateHash> {
    snapshots
        .iter()
        .map(|(c, k, s, e)| clock_snapshot_hash(*c, *k, *s, *e))
        .collect()
}
