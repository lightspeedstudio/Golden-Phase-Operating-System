//! Protected phase records — seal and open per-object phase state.

use crate::core::gphase_core::PhaseState;
use crate::kernel::anom::object_policy::{ProtectedObjectId, OBJECT_STATE_TAG};
use crate::kernel::crypto::aead::{open_bytes, seal_bytes, AeadError};
use crate::kernel::hash::{tagged_hash, StateHash};
use crate::kernel::state_proof::ObjectKind;
use crate::platform::{String, Vec};

#[derive(Debug, Clone)]
pub struct ProtectedPhasePlaintext {
    pub previous_phase: PhaseState,
    pub true_phase: PhaseState,
    pub operation: String,
    pub proper_tick: u64,
    pub kairos_tick: u64,
    pub sector_id: u8,
    pub private_trajectory: Vec<u8>,
    pub random_padding: Vec<u8>,
}

impl ProtectedPhasePlaintext {
    pub fn from_clock(
        previous_phase: PhaseState,
        true_phase: PhaseState,
        operation: impl Into<String>,
        clock: &crate::stateclock::StateClockSnapshot,
        private_trajectory: Vec<u8>,
        random_padding: Vec<u8>,
    ) -> Self {
        Self {
            previous_phase,
            true_phase,
            operation: operation.into(),
            proper_tick: clock.proper_tick,
            kairos_tick: clock.kairos_tick,
            sector_id: clock.sector_id,
            private_trajectory,
            random_padding,
        }
    }

    pub fn pack(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(self.previous_phase.as_u8());
        out.push(self.true_phase.as_u8());
        out.extend_from_slice(self.operation.as_bytes());
        out.push(0);
        out.extend_from_slice(&self.proper_tick.to_le_bytes());
        out.extend_from_slice(&self.kairos_tick.to_le_bytes());
        out.push(self.sector_id);
        out.extend_from_slice(&(self.private_trajectory.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.private_trajectory);
        out.extend_from_slice(&self.random_padding);
        out
    }

    pub fn unpack(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 6 {
            return None;
        }
        let previous_phase = PhaseState::from_u8(bytes[0])?;
        let true_phase = PhaseState::from_u8(bytes[1])?;
        let mut op_end = 2;
        while op_end < bytes.len() && bytes[op_end] != 0 {
            op_end += 1;
        }
        if op_end >= bytes.len() {
            return None;
        }
        let operation = String::from_utf8(bytes[2..op_end].to_vec()).ok()?;
        let mut i = op_end + 1;
        if bytes.len() < i + 17 {
            return None;
        }
        let proper_tick = u64::from_le_bytes(bytes[i..i + 8].try_into().ok()?);
        i += 8;
        let kairos_tick = u64::from_le_bytes(bytes[i..i + 8].try_into().ok()?);
        i += 8;
        let sector_id = *bytes.get(i)?;
        i += 1;
        if bytes.len() < i + 4 {
            return None;
        }
        let traj_len = u32::from_le_bytes(bytes[i..i + 4].try_into().ok()?) as usize;
        i += 4;
        if bytes.len() < i + traj_len {
            return None;
        }
        let private_trajectory = bytes[i..i + traj_len].to_vec();
        i += traj_len;
        let random_padding = bytes[i..].to_vec();
        Some(Self {
            previous_phase,
            true_phase,
            operation,
            proper_tick,
            kairos_tick,
            sector_id,
            private_trajectory,
            random_padding,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ObjectStateAadContext {
    pub equation_root: StateHash,
    pub boot_session_root: StateHash,
    pub object_id: ProtectedObjectId,
    pub protection_version: u32,
    pub sequence_number: u64,
    pub previous_event_hash: StateHash,
    pub operation: String,
    pub public_commitment: StateHash,
    pub chronos_tick: u64,
    pub kairos_tick: u64,
    pub sector_id: u8,
}

pub fn build_object_state_aad(ctx: &ObjectStateAadContext) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(OBJECT_STATE_TAG.as_bytes());
    out.extend_from_slice(&ctx.equation_root);
    out.extend_from_slice(&ctx.boot_session_root);
    out.push(ctx.object_id.kind as u8);
    out.extend_from_slice(ctx.object_id.value.as_bytes());
    out.extend_from_slice(&ctx.protection_version.to_le_bytes());
    out.extend_from_slice(&ctx.sequence_number.to_le_bytes());
    out.extend_from_slice(&ctx.previous_event_hash);
    out.extend_from_slice(ctx.operation.as_bytes());
    out.extend_from_slice(&ctx.public_commitment);
    out.extend_from_slice(&ctx.chronos_tick.to_le_bytes());
    out.extend_from_slice(&ctx.kairos_tick.to_le_bytes());
    out.push(ctx.sector_id);
    out
}

pub fn object_precommitment(ctx: &ObjectStateAadContext, nonce: &[u8; 12]) -> StateHash {
    tagged_hash(
        "KAIROS_ANOM_OBJECT_COMMITMENT_V1",
        &[
            &ctx.equation_root,
            &ctx.boot_session_root,
            &[ctx.object_id.kind as u8],
            ctx.object_id.value.as_bytes(),
            &ctx.protection_version.to_le_bytes(),
            &ctx.sequence_number.to_le_bytes(),
            &ctx.previous_event_hash,
            ctx.operation.as_bytes(),
            &ctx.chronos_tick.to_le_bytes(),
            &ctx.kairos_tick.to_le_bytes(),
            &[ctx.sector_id],
            nonce,
        ],
    )
}

#[derive(Debug, Clone)]
pub struct ProtectedPhaseRecord {
    pub object_id: ProtectedObjectId,
    pub sequence_number: u64,
    pub public_commitment: StateHash,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub authentication_tag: Vec<u8>,
    pub aad_hash: StateHash,
    pub previous_event_hash: StateHash,
    pub protected_event_hash: StateHash,
    pub operation: String,
    pub chronos_tick: u64,
    pub kairos_tick: u64,
    pub sector_id: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectSealError {
    Aead(AeadError),
}

pub fn seal_protected_phase(
    object_data_key: &StateHash,
    nonce: &[u8; 12],
    plaintext: &ProtectedPhasePlaintext,
    ctx: &ObjectStateAadContext,
) -> Result<ProtectedPhaseRecord, ObjectSealError> {
    let commitment = object_precommitment(ctx, nonce);
    let mut aad_ctx = ctx.clone();
    aad_ctx.public_commitment = commitment;
    let aad = build_object_state_aad(&aad_ctx);
    let aad_hash = tagged_hash(OBJECT_STATE_TAG, &[&aad]);
    let ciphertext = seal_bytes(object_data_key, nonce, &aad, &plaintext.pack())
        .map_err(ObjectSealError::Aead)?;
    let protected_event_hash = tagged_hash(
        "KAIROS_ANOM_OBJECT_EVENT_V1",
        &[&commitment, &aad_hash, &ciphertext],
    );
    Ok(ProtectedPhaseRecord {
        object_id: ctx.object_id.clone(),
        sequence_number: ctx.sequence_number,
        public_commitment: commitment,
        nonce: nonce.to_vec(),
        ciphertext,
        authentication_tag: Vec::new(),
        aad_hash,
        previous_event_hash: ctx.previous_event_hash,
        protected_event_hash,
        operation: ctx.operation.clone(),
        chronos_tick: ctx.chronos_tick,
        kairos_tick: ctx.kairos_tick,
        sector_id: ctx.sector_id,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectOpenError {
    Tampered,
    BadPlaintext,
    CommitmentMismatch,
    Aead(AeadError),
}

impl From<AeadError> for ObjectOpenError {
    fn from(value: AeadError) -> Self {
        ObjectOpenError::Aead(value)
    }
}

pub fn open_protected_phase(
    object_data_key: &StateHash,
    record: &ProtectedPhaseRecord,
    ctx: &ObjectStateAadContext,
) -> Result<ProtectedPhasePlaintext, ObjectOpenError> {
    if ctx.public_commitment != record.public_commitment {
        return Err(ObjectOpenError::CommitmentMismatch);
    }
    let aad = build_object_state_aad(ctx);
    if record.aad_hash != tagged_hash(OBJECT_STATE_TAG, &[&aad]) {
        return Err(ObjectOpenError::Tampered);
    }
    let nonce: [u8; 12] = record.nonce[..12]
        .try_into()
        .map_err(|_| ObjectOpenError::Tampered)?;
    let plain = open_bytes(object_data_key, &nonce, &aad, &record.ciphertext)?;
    ProtectedPhasePlaintext::unpack(&plain).ok_or(ObjectOpenError::BadPlaintext)
}

pub fn object_kind_for_protected(
    kind: crate::kernel::anom::object_policy::ProtectedObjectKind,
) -> ObjectKind {
    use crate::kernel::anom::object_policy::ProtectedObjectKind;
    match kind {
        ProtectedObjectKind::Process => ObjectKind::Process,
        ProtectedObjectKind::Memory => ObjectKind::MemoryPage,
        ProtectedObjectKind::File => ObjectKind::File,
        ProtectedObjectKind::Vm => ObjectKind::Vm,
        ProtectedObjectKind::Craft => ObjectKind::Craft,
        ProtectedObjectKind::StateClock => ObjectKind::Driver,
        ProtectedObjectKind::Network => ObjectKind::Driver,
    }
}
