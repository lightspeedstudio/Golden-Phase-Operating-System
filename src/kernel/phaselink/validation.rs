//! Frame validation pipeline.

use crate::core::gphase_core::PhaseState;
use crate::kernel::phaselink::error::PhaseLinkError;
use crate::kernel::phaselink::quarantine::{QuarantineReason, QuarantinedFrame};
use crate::kernel::phaselink::types::{
    EquationRootPolicy, PhaseGatewayPolicy, PhaseLinkFrame, RemoteSequenceState, SequenceGapPolicy,
    PHASELINK_MAX_FRAME_SIZE,
};
use crate::kernel::process::VALID_TRANSITIONS;
use crate::kernel::state_proof::ObjectKind;
use crate::platform::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameValidationResult {
    Accepted,
    Quarantined(QuarantineReason),
    Rejected(PhaseLinkError),
}

#[derive(Debug, Clone)]
pub struct ValidatorContext {
    pub equation_root: [u8; 32],
    pub boot_session_root: [u8; 32],
    pub trusted_nodes: HashSet<[u8; 32]>,
    pub equation_policy: EquationRootPolicy,
    pub gap_policy: SequenceGapPolicy,
    pub gateway: PhaseGatewayPolicy,
    pub remotes: HashMap<[u8; 32], RemoteSequenceState>,
}

pub struct PhaseLinkValidator;

impl PhaseLinkValidator {
    pub fn validate(frame: &PhaseLinkFrame, ctx: &ValidatorContext) -> FrameValidationResult {
        if frame.payload.len() > PHASELINK_MAX_FRAME_SIZE {
            return FrameValidationResult::Quarantined(QuarantineReason::OversizedPayload);
        }
        if frame.equation_root != ctx.equation_root {
            return match ctx.equation_policy {
                EquationRootPolicy::ExactMatch => {
                    FrameValidationResult::Rejected(PhaseLinkError::EquationRootMismatch)
                }
                EquationRootPolicy::AllowListed | EquationRootPolicy::QuarantineUnknown => {
                    FrameValidationResult::Quarantined(QuarantineReason::UnknownEquationRoot)
                }
            };
        }
        if !ctx.trusted_nodes.contains(&frame.source_node) {
            return FrameValidationResult::Quarantined(QuarantineReason::UnknownNode);
        }
        if frame.flags.anom_public && ctx.gateway.allow_anom_decryption {
            return FrameValidationResult::Rejected(PhaseLinkError::AnomAccessDenied);
        }
        if !is_legal_transition(frame.previous_phase, frame.next_phase) {
            return FrameValidationResult::Quarantined(QuarantineReason::InvalidTransition);
        }
        if !proof_matches_frame(frame) {
            return FrameValidationResult::Rejected(PhaseLinkError::InvalidProof);
        }
        if let Some(remote) = ctx.remotes.get(&frame.source_node) {
            if remote.boot_session_root != frame.boot_session_root {
                return FrameValidationResult::Quarantined(QuarantineReason::BootSessionChanged);
            }
            if frame.sequence_number <= remote.last_sequence_number {
                return FrameValidationResult::Quarantined(QuarantineReason::ReplayDetected);
            }
            if frame.sequence_number > remote.last_sequence_number + 1 {
                return match ctx.gap_policy {
                    SequenceGapPolicy::Reject => {
                        FrameValidationResult::Rejected(PhaseLinkError::SequenceGap)
                    }
                    SequenceGapPolicy::Quarantine => {
                        FrameValidationResult::Quarantined(QuarantineReason::SequenceGap)
                    }
                    SequenceGapPolicy::AcceptAndRecordGap => FrameValidationResult::Accepted,
                };
            }
            if frame.previous_frame_hash != remote.last_frame_hash {
                return FrameValidationResult::Quarantined(QuarantineReason::PreviousHashMismatch);
            }
        }
        FrameValidationResult::Accepted
    }

    pub fn quarantined_meta(frame: &PhaseLinkFrame, reason: QuarantineReason) -> QuarantinedFrame {
        QuarantinedFrame {
            received_length: frame.payload.len(),
            source_hint: Some(frame.source_node),
            calculated_hash: frame.frame_hash,
            reason,
            chronos_tick: frame.chronos_tick,
            kairos_tick: frame.kairos_tick,
        }
    }
}

fn is_legal_transition(previous: PhaseState, next: PhaseState) -> bool {
    if previous == next {
        return true;
    }
    VALID_TRANSITIONS
        .iter()
        .find(|(from, _)| *from == previous)
        .map(|(_, tos)| tos.contains(&next))
        .unwrap_or(false)
}

fn proof_matches_frame(frame: &PhaseLinkFrame) -> bool {
    let proof = &frame.transition_proof;
    proof.equation_root == frame.equation_root
        && proof.previous_event_hash == frame.previous_frame_hash
        && proof.sequence_number == frame.sequence_number
        && proof.previous_state == frame.previous_phase
        && proof.requested_state == frame.next_phase
        && proof.operation_name == frame.operation
        && proof.payload_hash == frame.payload_hash
        && proof.object_kind == ObjectKind::from(frame.object_kind)
}
