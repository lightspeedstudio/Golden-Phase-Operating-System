//! AES-256-GCM AEAD for ANOM phase-state sealing.

use crate::kernel::crypto::keyring::{derive_subkey, K_ANOM_PHASE_STATE};
use crate::kernel::hash::StateHash;
use crate::platform::Vec;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AeadError {
    Encrypt,
    Decrypt,
    Tampered,
}

pub fn phase_state_key(k_anom: &StateHash) -> StateHash {
    derive_subkey(k_anom, K_ANOM_PHASE_STATE)
}

pub fn seal_bytes(
    key: &StateHash,
    nonce: &[u8; 12],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, AeadError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| AeadError::Encrypt)?;
    let nonce = Nonce::from_slice(nonce);
    cipher
        .encrypt(
            nonce,
            aes_gcm::aead::Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| AeadError::Encrypt)
}

pub fn open_bytes(
    key: &StateHash,
    nonce: &[u8; 12],
    aad: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, AeadError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| AeadError::Decrypt)?;
    let nonce = Nonce::from_slice(nonce);
    cipher
        .decrypt(
            nonce,
            aes_gcm::aead::Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| AeadError::Tampered)
}
