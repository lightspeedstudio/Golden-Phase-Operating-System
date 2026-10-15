//! Kernel/root decryptor for ANOM protected phase state.

use crate::core::gphase_core::PhaseState;
use crate::kernel::anom::record::{build_aad, AnomAadContext, AnomProtectedRecord, ANOM_AAD_TAG};
use crate::kernel::crypto::aead::{open_bytes, phase_state_key, AeadError};
use crate::kernel::crypto::keyring::AnomKeyring;
use crate::kernel::hash::{tagged_hash, StateHash};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnomError {
    NoKey,
    Tampered,
    BadPlaintext,
    CommitmentMismatch,
    Aead(AeadError),
}

impl From<AeadError> for AnomError {
    fn from(value: AeadError) -> Self {
        AnomError::Aead(value)
    }
}

pub fn open_anom_phase_state(
    keyring: &AnomKeyring,
    protected: &AnomProtectedRecord,
    expected: &AnomAadContext,
) -> Result<PhaseState, AnomError> {
    if expected.commitment_hash != protected.commitment_hash {
        return Err(AnomError::CommitmentMismatch);
    }
    let aad = build_aad(expected);
    if protected.aad_hash != tagged_hash(ANOM_AAD_TAG, &[&aad]) {
        return Err(AnomError::Tampered);
    }
    let key = phase_state_key(&keyring.k_anom);
    let plaintext = open_bytes(&key, &protected.nonce, &aad, &protected.ciphertext)?;
    let packed = crate::kernel::anom::record::AnomPlaintext::unpack(&plaintext)
        .ok_or(AnomError::BadPlaintext)?;
    Ok(packed.true_phase)
}

pub fn session_seal_from_passphrase(
    passphrase: &[u8],
    equation_root: &StateHash,
    boot_session_root: &StateHash,
) -> StateHash {
    AnomKeyring::derive(passphrase, equation_root, boot_session_root).k_anom_session_seal
}
