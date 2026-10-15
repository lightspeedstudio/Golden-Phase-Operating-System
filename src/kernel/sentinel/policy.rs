//! Sentinel policy. Fail-closed default: no rights, no signers.

use crate::kernel::crypto::phase_hash::ph_sentinel_policy;
use crate::kernel::hash::StateHash;
use crate::platform::Vec;

use super::types::Rights;

const MAGIC: &[u8; 4] = b"PSP1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentinelPolicy {
    pub generation: u64,
    pub max_rights: Rights,
    pub min_generation: u64,
    pub allowed_signers: Vec<[u8; 32]>,
}

impl SentinelPolicy {
    pub fn deny_all() -> Self {
        Self {
            generation: 0,
            max_rights: 0,
            min_generation: 0,
            allowed_signers: Vec::new(),
        }
    }

    pub fn canonical(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&self.generation.to_le_bytes());
        out.extend_from_slice(&self.max_rights.to_le_bytes());
        out.extend_from_slice(&self.min_generation.to_le_bytes());
        out.extend_from_slice(&(self.allowed_signers.len() as u32).to_le_bytes());
        for pk in &self.allowed_signers {
            out.extend_from_slice(pk);
        }
        out
    }

    pub fn root(&self) -> StateHash {
        ph_sentinel_policy(&self.canonical())
    }

    pub fn allows_signer(&self, pk: &[u8; 32]) -> bool {
        self.allowed_signers.iter().any(|s| s == pk)
    }
}
