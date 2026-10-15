//! Program manifest `M_i`. Not an identity user certificate.

use crate::kernel::crypto::phase_hash::ph_sentinel_manifest;
use crate::kernel::crypto::sign;
use crate::kernel::hash::StateHash;
use crate::platform::Vec;

use super::types::Rights;
use super::SentinelError;

const MAGIC: &[u8; 4] = b"PSM1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramManifest {
    pub generation: u64,
    pub expiry_tick: u64,
    pub content_hash: StateHash,
    pub requested_rights: Rights,
    pub signer_pubkey: [u8; 32],
    pub signature: [u8; 64],
}

impl ProgramManifest {
    pub fn canonical_tbs(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&self.generation.to_le_bytes());
        out.extend_from_slice(&self.expiry_tick.to_le_bytes());
        out.extend_from_slice(&self.content_hash);
        out.extend_from_slice(&self.requested_rights.to_le_bytes());
        out.extend_from_slice(&self.signer_pubkey);
        out
    }

    pub fn manifest_hash(&self) -> StateHash {
        ph_sentinel_manifest(&self.canonical_tbs())
    }

    pub fn sign(&mut self, secret: &[u8; 32]) {
        let kp = sign::SignKeypair::from_seed(secret);
        self.sign_with(&kp);
    }

    pub fn sign_with(&mut self, kp: &sign::SignKeypair) {
        self.signer_pubkey = kp.public_bytes();
        self.signature = kp.sign(&self.manifest_hash());
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = self.canonical_tbs();
        out.extend_from_slice(&self.signature);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, SentinelError> {
        const TBS: usize = 4 + 8 + 8 + 32 + 4 + 32;
        if bytes.len() != TBS + 64 {
            return Err(SentinelError::Unsigned);
        }
        if &bytes[0..4] != MAGIC {
            return Err(SentinelError::Unsigned);
        }
        let generation = u64::from_le_bytes(bytes[4..12].try_into().unwrap());
        let expiry_tick = u64::from_le_bytes(bytes[12..20].try_into().unwrap());
        let mut content_hash = [0u8; 32];
        content_hash.copy_from_slice(&bytes[20..52]);
        let requested_rights = u32::from_le_bytes(bytes[52..56].try_into().unwrap());
        let mut signer_pubkey = [0u8; 32];
        signer_pubkey.copy_from_slice(&bytes[56..88]);
        let mut signature = [0u8; 64];
        signature.copy_from_slice(&bytes[88..152]);
        let m = Self {
            generation,
            expiry_tick,
            content_hash,
            requested_rights,
            signer_pubkey,
            signature,
        };
        m.verify_signature()?;
        Ok(m)
    }

    pub fn verify_signature(&self) -> Result<(), SentinelError> {
        let msg = self.manifest_hash();
        if sign::verify(&self.signer_pubkey, &msg, &self.signature) {
            Ok(())
        } else {
            Err(SentinelError::BadSignature)
        }
    }
}
