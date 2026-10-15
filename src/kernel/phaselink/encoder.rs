//! PhaseLink encoder.

use crate::kernel::authority::PublicPhaseView;
use crate::kernel::hash::tagged_hash;
use crate::kernel::phaselink::error::PhaseLinkError;
use crate::kernel::phaselink::frame::{
    compute_frame_hash, encode_frame, serialize_frame_without_hash,
};
use crate::kernel::phaselink::proof::build_output_proof;
use crate::kernel::phaselink::types::{
    PayloadEncoding, PhaseLinkContext, PhaseLinkFrame, PhaseLinkObjectKind, PHASELINK_VERSION,
};
use crate::kernel::state_proof::{hash_payload, ObjectKind};
#[cfg(feature = "std")]
use crate::kernel::statechain::StateEvent;
use crate::platform::Vec;

pub struct PhaseLinkEncoder;

impl PhaseLinkEncoder {
    pub fn encode_frame(frame: &PhaseLinkFrame) -> Result<Vec<u8>, PhaseLinkError> {
        encode_frame(frame)
    }

    #[cfg(feature = "std")]
    pub fn build_state_frame(
        context: &PhaseLinkContext,
        event: &StateEvent,
        payload: &[u8],
    ) -> Result<PhaseLinkFrame, PhaseLinkError> {
        let proof = build_output_proof(
            context.equation_root,
            context.boot_session_root,
            context.source_node,
            context.destination_node,
            event.proof.object_kind,
            event.proof.object_id,
            &event.proof.operation_name,
            event.proof.previous_state,
            event.proof.requested_state,
            context.sequence_number,
            context.previous_frame_hash,
            payload,
            context.chronos_tick,
            context.kairos_tick,
            context.stateclock_sector,
        );
        let payload_hash = hash_payload(payload);
        let public_commitment = tagged_hash(
            "KAIROS_PHASELINK_PUBLIC_COMMITMENT_V1",
            &[
                &payload_hash,
                &context.previous_frame_hash,
                &context.sequence_number.to_be_bytes(),
            ],
        );
        let mut frame = PhaseLinkFrame {
            version: PHASELINK_VERSION,
            flags: context.flags,
            source_node: context.source_node,
            destination_node: context.destination_node,
            equation_root: context.equation_root,
            boot_session_root: context.boot_session_root,
            sequence_number: context.sequence_number,
            chronos_tick: context.chronos_tick,
            kairos_tick: context.kairos_tick,
            stateclock_sector: context.stateclock_sector,
            object_kind: PhaseLinkObjectKind::from(event.proof.object_kind),
            object_id: event.proof.object_id.to_string(),
            operation: event.proof.operation_name.clone(),
            previous_phase: event.proof.previous_state,
            next_phase: event.proof.requested_state,
            previous_frame_hash: context.previous_frame_hash,
            transition_proof: proof.transition,
            payload_encoding: context.payload_encoding,
            payload: payload.to_vec(),
            payload_hash,
            public_commitment,
            frame_hash: [0u8; 32],
        };
        let encoded = serialize_frame_without_hash(&frame)?;
        frame.frame_hash = compute_frame_hash(&encoded);
        Ok(frame)
    }

    pub fn public_commitment_frame(
        context: &PhaseLinkContext,
        object_kind: ObjectKind,
        object_id: &str,
        operation: &str,
        previous_phase: crate::core::gphase_core::PhaseState,
        next_phase: crate::core::gphase_core::PhaseState,
        commitment: PublicPhaseView,
    ) -> Result<PhaseLinkFrame, PhaseLinkError> {
        let payload = commitment.render_short().into_bytes();
        let payload_hash = hash_payload(&payload);
        let mut flags = context.flags;
        flags.anom_public = true;
        let proof = build_output_proof(
            context.equation_root,
            context.boot_session_root,
            context.source_node,
            context.destination_node,
            object_kind,
            object_id.parse().unwrap_or(0),
            operation,
            previous_phase,
            next_phase,
            context.sequence_number,
            context.previous_frame_hash,
            &payload,
            context.chronos_tick,
            context.kairos_tick,
            context.stateclock_sector,
        );
        let public_commitment = tagged_hash(
            "KAIROS_PHASELINK_ANOM_COMMITMENT_V1",
            &[&payload_hash, &context.previous_frame_hash],
        );
        let mut frame = PhaseLinkFrame {
            version: PHASELINK_VERSION,
            flags,
            source_node: context.source_node,
            destination_node: context.destination_node,
            equation_root: context.equation_root,
            boot_session_root: context.boot_session_root,
            sequence_number: context.sequence_number,
            chronos_tick: context.chronos_tick,
            kairos_tick: context.kairos_tick,
            stateclock_sector: context.stateclock_sector,
            object_kind: PhaseLinkObjectKind::from(object_kind),
            object_id: object_id.into(),
            operation: operation.into(),
            previous_phase,
            next_phase,
            previous_frame_hash: context.previous_frame_hash,
            transition_proof: proof.transition,
            payload_encoding: PayloadEncoding::PublicCommitment,
            payload,
            payload_hash,
            public_commitment,
            frame_hash: [0u8; 32],
        };
        let encoded = serialize_frame_without_hash(&frame)?;
        frame.frame_hash = compute_frame_hash(&encoded);
        Ok(frame)
    }
}
