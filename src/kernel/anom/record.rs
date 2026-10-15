//! AnomProtectedRecord sealing and AAD construction.

use crate::core::gphase_core::PhaseState;
use crate::kernel::authority::PublicPhaseView;
use crate::kernel::crypto::aead::{phase_state_key, seal_bytes, AeadError};
use crate::kernel::crypto::keyring::AnomKeyring;
use crate::kernel::crypto::nonce::NonceGenerator;
use crate::kernel::hash::{tagged_hash, StateHash};
use crate::kernel::state_proof::ObjectKind;
use crate::platform::{String, Vec};

pub const ANOM_AAD_TAG: &str = "KAIROS_ANOM_PHASE_V1";
pub const ANOM_COMMITMENT_TAG: &str = "KAIROS_ANOM_COMMITMENT_V1";

#[derive(Debug, Clone)]
pub struct AnomPlaintext {
    pub true_phase: PhaseState,
    pub previous_phase: PhaseState,
    pub operation: String,
    pub correction_policy_state: u8,
    pub chronos_tick: u64,
    pub kairos_tick: u64,
    pub sector_id: u8,
    pub x_bits: u64,
    pub random_padding: [u8; 8],
}

impl AnomPlaintext {
    pub fn pack(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(self.true_phase.as_u8());
        out.push(self.previous_phase.as_u8());
        out.extend_from_slice(self.operation.as_bytes());
        out.push(0);
        out.push(self.correction_policy_state);
        out.extend_from_slice(&self.chronos_tick.to_le_bytes());
        out.extend_from_slice(&self.kairos_tick.to_le_bytes());
        out.push(self.sector_id);
        out.extend_from_slice(&self.x_bits.to_le_bytes());
        out.extend_from_slice(&self.random_padding);
        out
    }

    pub fn unpack(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 37 {
            return None;
        }
        let true_phase = PhaseState::from_u8(bytes[0])?;
        let previous_phase = PhaseState::from_u8(bytes[1])?;
        let mut op_end = 2;
        while op_end < bytes.len() && bytes[op_end] != 0 {
            op_end += 1;
        }
        if op_end >= bytes.len() {
            return None;
        }
        let operation = String::from_utf8(bytes[2..op_end].to_vec()).ok()?;
        let mut i = op_end + 1;
        let correction_policy_state = *bytes.get(i)?;
        i += 1;
        let chronos_tick = u64::from_le_bytes(bytes[i..i + 8].try_into().ok()?);
        i += 8;
        let kairos_tick = u64::from_le_bytes(bytes[i..i + 8].try_into().ok()?);
        i += 8;
        let sector_id = *bytes.get(i)?;
        i += 1;
        let x_bits = u64::from_le_bytes(bytes[i..i + 8].try_into().ok()?);
        i += 8;
        let random_padding: [u8; 8] = bytes[i..i + 8].try_into().ok()?;
        Some(Self {
            true_phase,
            previous_phase,
            operation,
            correction_policy_state,
            chronos_tick,
            kairos_tick,
            sector_id,
            x_bits,
            random_padding,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AnomProtectedRecord {
    pub object_kind: ObjectKind,
    pub object_id: u32,
    pub sequence_number: u64,
    pub previous_event_hash: StateHash,
    pub public_view: PublicPhaseView,
    pub nonce: [u8; 12],
    pub aad_hash: StateHash,
    pub ciphertext: Vec<u8>,
    pub commitment_hash: StateHash,
    pub operation: String,
    pub chronos_tick: u64,
    pub kairos_tick: u64,
    pub sector_id: u8,
    pub x: f64,
    pub kernel_state_root: Option<StateHash>,
}

#[derive(Debug, Clone)]
pub struct AnomAadContext {
    pub equation_root: StateHash,
    pub boot_session_root: StateHash,
    pub kernel_state_root: Option<StateHash>,
    pub previous_event_hash: StateHash,
    pub object_kind: ObjectKind,
    pub object_id: u32,
    pub sequence_number: u64,
    pub commitment_hash: StateHash,
    pub operation: String,
    pub chronos_tick: u64,
    pub kairos_tick: u64,
    pub sector_id: u8,
    pub x: f64,
}

#[allow(clippy::too_many_arguments)]
pub fn precommitment(
    equation_root: &StateHash,
    boot_session_root: &StateHash,
    object_kind: ObjectKind,
    object_id: u32,
    sequence_number: u64,
    previous_event_hash: &StateHash,
    operation: &str,
    chronos_tick: u64,
    kairos_tick: u64,
    sector_id: u8,
    x: f64,
    nonce: &[u8; 12],
) -> StateHash {
    tagged_hash(
        ANOM_COMMITMENT_TAG,
        &[
            equation_root,
            boot_session_root,
            &[object_kind as u8],
            &object_id.to_le_bytes(),
            &sequence_number.to_le_bytes(),
            previous_event_hash,
            operation.as_bytes(),
            &chronos_tick.to_le_bytes(),
            &kairos_tick.to_le_bytes(),
            &[sector_id],
            &x.to_le_bytes(),
            nonce,
        ],
    )
}

pub fn build_aad(ctx: &AnomAadContext) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(ANOM_AAD_TAG.as_bytes());
    out.extend_from_slice(&ctx.equation_root);
    out.extend_from_slice(&ctx.boot_session_root);
    if let Some(kr) = ctx.kernel_state_root {
        out.extend_from_slice(&kr);
    }
    out.extend_from_slice(&ctx.previous_event_hash);
    out.push(ctx.object_kind as u8);
    out.extend_from_slice(&ctx.object_id.to_le_bytes());
    out.extend_from_slice(&ctx.sequence_number.to_le_bytes());
    out.extend_from_slice(&ctx.commitment_hash);
    out.extend_from_slice(ctx.operation.as_bytes());
    out.extend_from_slice(&ctx.chronos_tick.to_le_bytes());
    out.extend_from_slice(&ctx.kairos_tick.to_le_bytes());
    out.push(ctx.sector_id);
    out.extend_from_slice(&ctx.x.to_le_bytes());
    out
}

pub fn compute_commitment_hash(record: &AnomProtectedRecord) -> StateHash {
    record.commitment_hash
}

fn padding_from_nonce(nonce: &[u8; 12], seq: u64) -> [u8; 8] {
    let h = tagged_hash("KAIROS_ANOM_PADDING_V1", &[nonce, &seq.to_le_bytes()]);
    let mut pad = [0u8; 8];
    pad.copy_from_slice(&h[..8]);
    pad
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SealError {
    Aead(AeadError),
}

#[allow(clippy::too_many_arguments)]
pub fn seal_anom_phase_state(
    keyring: &AnomKeyring,
    nonce_gen: &mut NonceGenerator,
    true_phase: PhaseState,
    previous_phase: PhaseState,
    object_kind: ObjectKind,
    object_id: u32,
    sequence_number: u64,
    operation: &str,
    previous_event_hash: StateHash,
    ctx: &AnomAadContext,
) -> Result<AnomProtectedRecord, SealError> {
    let nonce = nonce_gen.next_nonce().bytes;
    let commitment = precommitment(
        &ctx.equation_root,
        &ctx.boot_session_root,
        object_kind,
        object_id,
        sequence_number,
        &previous_event_hash,
        operation,
        ctx.chronos_tick,
        ctx.kairos_tick,
        ctx.sector_id,
        ctx.x,
        &nonce,
    );
    let aad_ctx = AnomAadContext {
        equation_root: ctx.equation_root,
        boot_session_root: ctx.boot_session_root,
        kernel_state_root: ctx.kernel_state_root,
        previous_event_hash,
        object_kind,
        object_id,
        sequence_number,
        commitment_hash: commitment,
        operation: operation.into(),
        chronos_tick: ctx.chronos_tick,
        kairos_tick: ctx.kairos_tick,
        sector_id: ctx.sector_id,
        x: ctx.x,
    };
    let aad = build_aad(&aad_ctx);
    let aad_hash = tagged_hash(ANOM_AAD_TAG, &[&aad]);

    let plaintext = AnomPlaintext {
        true_phase,
        previous_phase,
        operation: operation.into(),
        correction_policy_state: 0,
        chronos_tick: ctx.chronos_tick,
        kairos_tick: ctx.kairos_tick,
        sector_id: ctx.sector_id,
        x_bits: ctx.x.to_bits(),
        random_padding: padding_from_nonce(&nonce, sequence_number),
    };

    let key = phase_state_key(&keyring.k_anom);
    let ciphertext = seal_bytes(&key, &nonce, &aad, &plaintext.pack()).map_err(SealError::Aead)?;

    Ok(AnomProtectedRecord {
        object_kind,
        object_id,
        sequence_number,
        previous_event_hash,
        public_view: PublicPhaseView::AnomCommitment(commitment),
        nonce,
        aad_hash,
        ciphertext,
        commitment_hash: commitment,
        operation: operation.into(),
        chronos_tick: ctx.chronos_tick,
        kairos_tick: ctx.kairos_tick,
        sector_id: ctx.sector_id,
        x: ctx.x,
        kernel_state_root: ctx.kernel_state_root,
    })
}
