//! PhaseLink proof helpers.

use crate::core::gphase_core::PhaseState;
use crate::kernel::authority::StateHash;
use crate::kernel::hash::tagged_hash;
use crate::kernel::phaselink::types::NodeId;
use crate::kernel::state_proof::{
    hash_payload, ObjectKind, StateTransitionProof, TopologyProofStatus,
};

pub const PHASELINK_PROOF_TAG: &str = "KAIROS_PHASELINK_PROOF_V1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseLinkProofEnvelope {
    pub transition: StateTransitionProof,
    pub proof_hash: StateHash,
}

#[allow(clippy::too_many_arguments)]
pub fn build_output_proof(
    equation_root: StateHash,
    boot_session_root: StateHash,
    source_node: NodeId,
    destination_node: NodeId,
    object_kind: ObjectKind,
    object_id: u32,
    operation: &str,
    previous_phase: PhaseState,
    next_phase: PhaseState,
    sequence_number: u64,
    previous_frame_hash: StateHash,
    payload: &[u8],
    chronos_tick: u64,
    kairos_tick: u64,
    sector: i8,
) -> PhaseLinkProofEnvelope {
    let transition = StateTransitionProof {
        object_kind,
        object_id,
        sequence_number,
        previous_state: previous_phase,
        requested_state: next_phase,
        operation_name: operation.into(),
        compose_result: None,
        topology_status: TopologyProofStatus::None,
        payload_hash: hash_payload(payload),
        previous_event_hash: previous_frame_hash,
        equation_root,
    };
    let proof_hash = tagged_hash(
        PHASELINK_PROOF_TAG,
        &[
            &equation_root,
            &boot_session_root,
            &source_node,
            &destination_node,
            &[object_kind as u8],
            &object_id.to_be_bytes(),
            operation.as_bytes(),
            &[previous_phase.as_u8()],
            &[next_phase.as_u8()],
            &sequence_number.to_be_bytes(),
            &previous_frame_hash,
            &hash_payload(payload),
            &chronos_tick.to_be_bytes(),
            &kairos_tick.to_be_bytes(),
            &[sector as u8],
        ],
    );
    PhaseLinkProofEnvelope {
        transition,
        proof_hash,
    }
}
