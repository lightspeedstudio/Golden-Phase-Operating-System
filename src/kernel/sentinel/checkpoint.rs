//! Verified Sentinel checkpoint. Distinct from HMAC `SignedCheckpoint`.
//! MEASURED label: `R_memory` is a PhaseHash of a software snapshot, not
//! hardware-rooted. Recover is refused until this type verifies.

use crate::kernel::crypto::phase_hash::ph_sentinel_checkpoint;
use crate::kernel::crypto::sign;
use crate::kernel::hash::StateHash;
use crate::platform::Vec;

use super::SentinelError;

const MAGIC: &[u8; 4] = b"PSC1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentinelCheckpoint {
    pub generation: u64,
    pub content_root: StateHash,
    pub r_state: StateHash,
    pub r_memory: StateHash,
    pub r_program: StateHash,
    pub r_ps: StateHash,
    pub signature: [u8; 64],
}

impl SentinelCheckpoint {
    pub fn canonical_tbs(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&self.generation.to_le_bytes());
        out.extend_from_slice(&self.content_root);
        out.extend_from_slice(&self.r_state);
        out.extend_from_slice(&self.r_memory);
        out.extend_from_slice(&self.r_program);
        out.extend_from_slice(&self.r_ps);
        out
    }

    pub fn message_hash(&self) -> StateHash {
        ph_sentinel_checkpoint(&self.canonical_tbs())
    }

    pub fn sign(&mut self, secret: &[u8; 32]) {
        let kp = sign::SignKeypair::from_seed(secret);
        self.signature = kp.sign(&self.message_hash());
    }

    pub fn verify(&self, pubkey: &[u8; 32]) -> Result<(), SentinelError> {
        if sign::verify(pubkey, &self.message_hash(), &self.signature) {
            Ok(())
        } else {
            Err(SentinelError::CheckpointInvalid)
        }
    }
}
