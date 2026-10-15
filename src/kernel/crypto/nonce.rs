//! Nonce generation for ANOM AEAD records.

use crate::kernel::hash::StateHash;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnomNonce {
    pub bytes: [u8; 12],
}

#[derive(Debug)]
pub struct NonceGenerator {
    counter: u64,
    session_salt: StateHash,
}

impl NonceGenerator {
    pub fn new(session_salt: StateHash) -> Self {
        Self {
            counter: 0,
            session_salt,
        }
    }

    pub fn next_nonce(&mut self) -> AnomNonce {
        self.counter += 1;
        let mut bytes = [0u8; 12];
        bytes[..8].copy_from_slice(&self.counter.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.session_salt[..4]);
        AnomNonce { bytes }
    }
}
