//! PHI-STABILITY phase trajectory records.

use crate::core::gphase_core::{topology_status, PhaseState, TopologyKind};
use crate::kernel::hash::StateHash;
use crate::kernel::phase_math::tables::legal_transition;
use crate::kernel::phi_stability::error::PhiStabilityError;
use crate::kernel::phi_stability::fixed::U1024;
use crate::kernel::phi_stability::proof::{hash_fib_value, hash_pair_commitment, step_hash};
use crate::kernel::phi_stability::types::{
    PhiStepRecord, PhiTestId, TopologyRelation, PHI_STABILITY_MAX_STEPS,
};
use crate::platform::Vec;
use crate::protocol::checksum::state_sheet;
use crate::stateclock::StateClockSnapshot;

pub fn transition_phase(
    current: PhaseState,
    target: PhaseState,
) -> Result<PhaseState, PhiStabilityError> {
    if !legal_transition(current, target) {
        return Err(PhiStabilityError::InvalidPhaseTransition);
    }
    Ok(target)
}

pub fn lifecycle_phase_for_step(step_index: u32, total_bits: u32) -> (PhaseState, PhaseState) {
    if step_index == 0 {
        return (PhaseState::Phi0, PhaseState::Phi1);
    }
    if step_index > total_bits {
        return (PhaseState::Phi2, PhaseState::Phi3);
    }
    if step_index % 2 == 1 {
        (PhaseState::Phi1, PhaseState::Phi2)
    } else {
        (PhaseState::Phi2, PhaseState::Phi1)
    }
}

pub fn topology_for_phases(a: PhaseState, b: PhaseState) -> TopologyRelation {
    match topology_status(a, b, b).kind {
        TopologyKind::Same => TopologyRelation::Same,
        TopologyKind::Reflection => TopologyRelation::Reflection,
    }
}

pub fn chi_for_phase(phase: PhaseState) -> i8 {
    let sheet = state_sheet(phase);
    (sheet * 127.0) as i8
}

pub fn epsilon_fixed(snapshot: &StateClockSnapshot) -> i64 {
    (snapshot.epsilon * 1_000_000.0) as i64
}

pub struct TrajectoryBuilder {
    pub steps: Vec<PhiStepRecord>,
}

impl Default for TrajectoryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TrajectoryBuilder {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    pub fn record_step(
        &mut self,
        test_id: PhiTestId,
        step_index: u32,
        input_bit: bool,
        previous_phase: PhaseState,
        next_phase: PhaseState,
        snapshot: &StateClockSnapshot,
        f_k_hash: &StateHash,
        f_k1_hash: &StateHash,
        previous_step_hash: StateHash,
    ) -> Result<StateHash, PhiStabilityError> {
        if self.steps.len() >= PHI_STABILITY_MAX_STEPS {
            return Err(PhiStabilityError::QueueFull);
        }
        let mut record = PhiStepRecord {
            test_id,
            step_index,
            input_bit,
            previous_phase,
            next_phase,
            topology_relation: topology_for_phases(previous_phase, next_phase),
            chi: chi_for_phase(previous_phase),
            chronos_tick: snapshot.chronos_tick,
            kairos_tick: snapshot.kairos_tick,
            stateclock_sector: snapshot.sector_id as i8,
            stateclock_epsilon: epsilon_fixed(snapshot),
            pair_commitment: hash_pair_commitment(f_k_hash, f_k1_hash),
            previous_step_hash,
            step_hash: [0u8; 32],
        };
        record.step_hash = step_hash(&record);
        let hash = record.step_hash;
        self.steps.push(record);
        Ok(hash)
    }
}

pub fn hash_pair_at_step(a: &U1024, b: &U1024) -> (StateHash, StateHash) {
    (hash_fib_value(a), hash_fib_value(b))
}
