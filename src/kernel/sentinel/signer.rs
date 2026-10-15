//! Dedicated PhaseSentinel signer table. Not `identity::CertificateRegistry`.

use crate::kernel::crypto::sign;
use crate::kernel::hash::StateHash;
use crate::platform::Vec;

use super::root::k_authority;
use super::SentinelError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SentinelSigner {
    pub pubkey: [u8; 32],
    pub generation: u64,
    pub expires_tick: u64,
    pub revoked: bool,
}

pub struct SignerTable {
    pub root_pubkey: Option<[u8; 32]>,
    signers: Vec<SentinelSigner>,
}

impl SignerTable {
    pub fn new() -> Self {
        Self {
            root_pubkey: None,
            signers: Vec::new(),
        }
    }

    pub fn pin_root(&mut self, pubkey: [u8; 32]) {
        self.root_pubkey = Some(pubkey);
        if !self.signers.iter().any(|s| s.pubkey == pubkey) {
            self.signers.push(SentinelSigner {
                pubkey,
                generation: 0,
                expires_tick: u64::MAX,
                revoked: false,
            });
        }
    }

    pub fn k_authority(&self) -> Option<StateHash> {
        self.root_pubkey.as_ref().map(k_authority)
    }

    pub fn install(&mut self, signer: SentinelSigner) -> Result<(), SentinelError> {
        if self.root_pubkey.is_none() {
            return Err(SentinelError::NoAnchor);
        }
        if let Some(existing) = self.signers.iter_mut().find(|s| s.pubkey == signer.pubkey) {
            *existing = signer;
        } else {
            self.signers.push(signer);
        }
        Ok(())
    }

    pub fn revoke(&mut self, pubkey: &[u8; 32]) -> Result<(), SentinelError> {
        match self.signers.iter_mut().find(|s| s.pubkey == *pubkey) {
            Some(s) => {
                s.revoked = true;
                Ok(())
            }
            None => Err(SentinelError::Unauthorized),
        }
    }

    pub fn lookup(&self, pubkey: &[u8; 32]) -> Option<&SentinelSigner> {
        self.signers.iter().find(|s| s.pubkey == *pubkey)
    }

    pub fn verify_manifest(
        &self,
        pubkey: &[u8; 32],
        msg: &StateHash,
        sig: &[u8; 64],
        now_tick: u64,
        min_generation: u64,
    ) -> Result<(), SentinelError> {
        if self.root_pubkey.is_none() {
            return Err(SentinelError::NoAnchor);
        }
        let signer = self.lookup(pubkey).ok_or(SentinelError::Unauthorized)?;
        if signer.revoked {
            return Err(SentinelError::Revoked);
        }
        if now_tick >= signer.expires_tick {
            return Err(SentinelError::Expired);
        }
        if signer.generation < min_generation {
            return Err(SentinelError::Generation);
        }
        if !sign::verify(pubkey, msg, sig) {
            return Err(SentinelError::BadSignature);
        }
        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = &SentinelSigner> {
        self.signers.iter()
    }
}

impl Default for SignerTable {
    fn default() -> Self {
        Self::new()
    }
}
