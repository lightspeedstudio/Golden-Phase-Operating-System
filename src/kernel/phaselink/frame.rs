//! Deterministic frame serialization.

use crate::core::gphase_core::PhaseState;
use crate::kernel::hash::tagged_hash;
use crate::kernel::phaselink::error::PhaseLinkError;
use crate::kernel::phaselink::proof::PhaseLinkProofEnvelope;
use crate::kernel::phaselink::types::{
    Hash, PayloadEncoding, PhaseLinkFlags, PhaseLinkFrame, PhaseLinkObjectKind, PHASELINK_MAGIC,
    PHASELINK_MAX_FRAME_SIZE, PHASELINK_MAX_OBJECT_ID_LEN, PHASELINK_MAX_PAYLOAD_SIZE,
    PHASELINK_MAX_PROOF_LEN, PHASELINK_VERSION,
};
use crate::kernel::state_proof::{ObjectKind, StateTransitionProof, TopologyProofStatus};
use crate::platform::{String, Vec};

pub const PHASELINK_FRAME_TAG: &str = "KAIROS_PHASELINK_FRAME_V1";

pub fn serialize_frame_without_hash(frame: &PhaseLinkFrame) -> Result<Vec<u8>, PhaseLinkError> {
    if frame.version != PHASELINK_VERSION {
        return Err(PhaseLinkError::UnsupportedVersion);
    }
    if frame.payload.len() > PHASELINK_MAX_PAYLOAD_SIZE
        || frame.object_id.len() > PHASELINK_MAX_OBJECT_ID_LEN
    {
        return Err(PhaseLinkError::PayloadTooLarge);
    }
    let proof = serialize_proof(&frame.transition_proof)?;
    let mut out = Vec::new();
    out.extend_from_slice(&PHASELINK_MAGIC);
    out.push(frame.version);
    out.push(frame.flags.to_bits());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&frame.source_node);
    out.extend_from_slice(&frame.destination_node);
    out.extend_from_slice(&frame.equation_root);
    out.extend_from_slice(&frame.boot_session_root);
    out.extend_from_slice(&frame.sequence_number.to_be_bytes());
    out.extend_from_slice(&frame.chronos_tick.to_be_bytes());
    out.extend_from_slice(&frame.kairos_tick.to_be_bytes());
    out.push(frame.stateclock_sector as u8);
    out.push(frame.object_kind as u8);
    write_string(&mut out, &frame.object_id)?;
    write_string(&mut out, &frame.operation)?;
    out.push(frame.previous_phase.as_u8());
    out.push(frame.next_phase.as_u8());
    out.extend_from_slice(&frame.previous_frame_hash);
    out.extend_from_slice(&(proof.len() as u32).to_be_bytes());
    out.extend_from_slice(&proof);
    out.push(frame.payload_encoding as u8);
    out.extend_from_slice(&(frame.payload.len() as u32).to_be_bytes());
    out.extend_from_slice(&frame.payload);
    out.extend_from_slice(&frame.payload_hash);
    out.extend_from_slice(&frame.public_commitment);

    let header_len = 12u16;
    let total_len = (out.len() + 32) as u32;
    out[6..8].copy_from_slice(&header_len.to_be_bytes());
    out[8..12].copy_from_slice(&total_len.to_be_bytes());
    if total_len as usize > PHASELINK_MAX_FRAME_SIZE {
        return Err(PhaseLinkError::PayloadTooLarge);
    }
    Ok(out)
}

pub fn compute_frame_hash(bytes_without_hash: &[u8]) -> Hash {
    tagged_hash(PHASELINK_FRAME_TAG, &[bytes_without_hash])
}

pub fn encode_frame(frame: &PhaseLinkFrame) -> Result<Vec<u8>, PhaseLinkError> {
    let mut out = serialize_frame_without_hash(frame)?;
    out.extend_from_slice(&compute_frame_hash(&out));
    Ok(out)
}

pub fn decode_frame(bytes: &[u8]) -> Result<PhaseLinkFrame, PhaseLinkError> {
    if bytes.len() < 4 + 1 + 1 + 2 + 4 + 32 {
        return Err(PhaseLinkError::TruncatedFrame);
    }
    let mut off = 0usize;
    if bytes.get(0..4) != Some(&PHASELINK_MAGIC) {
        return Err(PhaseLinkError::InvalidMagic);
    }
    off += 4;
    let version = read_u8(bytes, &mut off)?;
    if version != PHASELINK_VERSION {
        return Err(PhaseLinkError::UnsupportedVersion);
    }
    let flags =
        PhaseLinkFlags::from_bits(read_u8(bytes, &mut off)?).ok_or(PhaseLinkError::InvalidFlags)?;
    let _header_length = read_u16_be(bytes, &mut off)?;
    let total_length = read_u32_be(bytes, &mut off)? as usize;
    if total_length != bytes.len() {
        return Err(PhaseLinkError::InvalidLength);
    }
    let source_node = read_hash(bytes, &mut off)?;
    let destination_node = read_hash(bytes, &mut off)?;
    let equation_root = read_hash(bytes, &mut off)?;
    let boot_session_root = read_hash(bytes, &mut off)?;
    let sequence_number = read_u64_be(bytes, &mut off)?;
    let chronos_tick = read_u64_be(bytes, &mut off)?;
    let kairos_tick = read_u64_be(bytes, &mut off)?;
    let stateclock_sector = read_u8(bytes, &mut off)? as i8;
    let object_kind = match read_u8(bytes, &mut off)? {
        0 => PhaseLinkObjectKind::Process,
        1 => PhaseLinkObjectKind::MemoryPage,
        2 => PhaseLinkObjectKind::File,
        3 => PhaseLinkObjectKind::Directory,
        4 => PhaseLinkObjectKind::Block,
        5 => PhaseLinkObjectKind::Syscall,
        6 => PhaseLinkObjectKind::Vm,
        7 => PhaseLinkObjectKind::Driver,
        8 => PhaseLinkObjectKind::Craft,
        _ => return Err(PhaseLinkError::InvalidObjectId),
    };
    let object_id = read_string(bytes, &mut off)?;
    let operation = read_string(bytes, &mut off)?;
    let previous_phase =
        PhaseState::from_u8(read_u8(bytes, &mut off)?).ok_or(PhaseLinkError::InvalidPhaseSymbol)?;
    let next_phase =
        PhaseState::from_u8(read_u8(bytes, &mut off)?).ok_or(PhaseLinkError::InvalidPhaseSymbol)?;
    let previous_frame_hash = read_hash(bytes, &mut off)?;
    let proof_len = read_u32_be(bytes, &mut off)? as usize;
    if proof_len > PHASELINK_MAX_PROOF_LEN {
        return Err(PhaseLinkError::PayloadTooLarge);
    }
    let proof_bytes = read_bytes(bytes, &mut off, proof_len)?;
    let transition_proof = deserialize_proof(proof_bytes)?;
    let payload_encoding = PayloadEncoding::from_u8(read_u8(bytes, &mut off)?)
        .ok_or(PhaseLinkError::SerializationFailure)?;
    let payload_len = read_u32_be(bytes, &mut off)? as usize;
    if payload_len > PHASELINK_MAX_PAYLOAD_SIZE {
        return Err(PhaseLinkError::PayloadTooLarge);
    }
    let payload = read_bytes(bytes, &mut off, payload_len)?.to_vec();
    let payload_hash = read_hash(bytes, &mut off)?;
    let public_commitment = read_hash(bytes, &mut off)?;
    let frame_hash = read_hash(bytes, &mut off)?;
    if off != bytes.len() {
        return Err(PhaseLinkError::TrailingData);
    }
    let expected_hash = compute_frame_hash(&bytes[..bytes.len() - 32]);
    if expected_hash != frame_hash {
        return Err(PhaseLinkError::FrameHashMismatch);
    }
    Ok(PhaseLinkFrame {
        version,
        flags,
        source_node,
        destination_node,
        equation_root,
        boot_session_root,
        sequence_number,
        chronos_tick,
        kairos_tick,
        stateclock_sector,
        object_kind,
        object_id,
        operation,
        previous_phase,
        next_phase,
        previous_frame_hash,
        transition_proof,
        payload_encoding,
        payload,
        payload_hash,
        public_commitment,
        frame_hash,
    })
}

fn serialize_proof(proof: &StateTransitionProof) -> Result<Vec<u8>, PhaseLinkError> {
    let mut out = Vec::new();
    out.push(proof.object_kind as u8);
    out.extend_from_slice(&proof.object_id.to_be_bytes());
    out.extend_from_slice(&proof.sequence_number.to_be_bytes());
    out.push(proof.previous_state.as_u8());
    out.push(proof.requested_state.as_u8());
    write_string(&mut out, &proof.operation_name)?;
    out.push(proof.compose_result.map(|p| p.as_u8()).unwrap_or(255));
    out.push(proof.topology_status.as_u8());
    out.extend_from_slice(&proof.payload_hash);
    out.extend_from_slice(&proof.previous_event_hash);
    out.extend_from_slice(&proof.equation_root);
    Ok(out)
}

fn deserialize_proof(bytes: &[u8]) -> Result<StateTransitionProof, PhaseLinkError> {
    let mut off = 0usize;
    let object_kind = match read_u8(bytes, &mut off)? {
        0 => ObjectKind::Process,
        1 => ObjectKind::MemoryPage,
        2 => ObjectKind::File,
        3 => ObjectKind::Directory,
        4 => ObjectKind::Block,
        5 => ObjectKind::Syscall,
        6 => ObjectKind::Vm,
        7 => ObjectKind::Driver,
        8 => ObjectKind::Craft,
        #[cfg(feature = "phasestack")]
        9 => ObjectKind::App,
        _ => return Err(PhaseLinkError::InvalidObjectId),
    };
    let object_id = read_u32_be(bytes, &mut off)?;
    let sequence_number = read_u64_be(bytes, &mut off)?;
    let previous_state =
        PhaseState::from_u8(read_u8(bytes, &mut off)?).ok_or(PhaseLinkError::InvalidPhaseSymbol)?;
    let requested_state =
        PhaseState::from_u8(read_u8(bytes, &mut off)?).ok_or(PhaseLinkError::InvalidPhaseSymbol)?;
    let operation_name = read_string(bytes, &mut off)?;
    let compose_raw = read_u8(bytes, &mut off)?;
    let compose_result = if compose_raw == 255 {
        None
    } else {
        PhaseState::from_u8(compose_raw)
    };
    let topology_status = match read_u8(bytes, &mut off)? {
        0 => TopologyProofStatus::None,
        1 => TopologyProofStatus::Same,
        2 => TopologyProofStatus::Reflection,
        _ => return Err(PhaseLinkError::InvalidProof),
    };
    let payload_hash = read_hash(bytes, &mut off)?;
    let previous_event_hash = read_hash(bytes, &mut off)?;
    let equation_root = read_hash(bytes, &mut off)?;
    if off != bytes.len() {
        return Err(PhaseLinkError::TrailingData);
    }
    Ok(StateTransitionProof {
        object_kind,
        object_id,
        sequence_number,
        previous_state,
        requested_state,
        operation_name,
        compose_result,
        topology_status,
        payload_hash,
        previous_event_hash,
        equation_root,
    })
}

fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), PhaseLinkError> {
    if value.len() > PHASELINK_MAX_OBJECT_ID_LEN {
        return Err(PhaseLinkError::InvalidObjectId);
    }
    out.extend_from_slice(&(value.len() as u16).to_be_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn read_string(bytes: &[u8], off: &mut usize) -> Result<String, PhaseLinkError> {
    let len = read_u16_be(bytes, off)? as usize;
    let slice = read_bytes(bytes, off, len)?;
    let s = core::str::from_utf8(slice).map_err(|_| PhaseLinkError::SerializationFailure)?;
    Ok(s.into())
}

fn read_bytes<'a>(
    bytes: &'a [u8],
    off: &mut usize,
    len: usize,
) -> Result<&'a [u8], PhaseLinkError> {
    if *off + len > bytes.len() {
        return Err(PhaseLinkError::TruncatedFrame);
    }
    let out = &bytes[*off..*off + len];
    *off += len;
    Ok(out)
}

fn read_hash(bytes: &[u8], off: &mut usize) -> Result<Hash, PhaseLinkError> {
    let slice = read_bytes(bytes, off, 32)?;
    let mut out = [0u8; 32];
    out.copy_from_slice(slice);
    Ok(out)
}

fn read_u8(bytes: &[u8], off: &mut usize) -> Result<u8, PhaseLinkError> {
    let out = *bytes.get(*off).ok_or(PhaseLinkError::TruncatedFrame)?;
    *off += 1;
    Ok(out)
}

fn read_u16_be(bytes: &[u8], off: &mut usize) -> Result<u16, PhaseLinkError> {
    let slice = read_bytes(bytes, off, 2)?;
    Ok(u16::from_be_bytes([slice[0], slice[1]]))
}

fn read_u32_be(bytes: &[u8], off: &mut usize) -> Result<u32, PhaseLinkError> {
    let slice = read_bytes(bytes, off, 4)?;
    Ok(u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u64_be(bytes: &[u8], off: &mut usize) -> Result<u64, PhaseLinkError> {
    let slice = read_bytes(bytes, off, 8)?;
    Ok(u64::from_be_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

pub fn proof_envelope(frame: &PhaseLinkFrame) -> PhaseLinkProofEnvelope {
    PhaseLinkProofEnvelope {
        transition: frame.transition_proof.clone(),
        proof_hash: tagged_hash(
            "KAIROS_PHASELINK_PROOF_HASH_ONLY_V1",
            &[
                &frame.transition_proof.equation_root,
                &frame.payload_hash,
                &frame.previous_frame_hash,
            ],
        ),
    }
}
