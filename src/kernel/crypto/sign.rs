//! Ed25519 signing primitive for identity and operator authentication.
//!
//! Bare-metal capable (no_std + alloc). Deterministic keypairs derive from a
//! 32-byte seed so a boot session can reproduce its signing identity without a
//! runtime CSPRNG; production keys should come from sealed storage.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

/// An ed25519 signing identity.
pub struct SignKeypair {
    signing: SigningKey,
}

impl SignKeypair {
    /// Deterministic keypair from a 32-byte seed.
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        Self {
            signing: SigningKey::from_bytes(seed),
        }
    }

    /// The 32-byte public key.
    pub fn public_bytes(&self) -> [u8; 32] {
        self.signing.verifying_key().to_bytes()
    }

    /// Sign `msg`, returning the 64-byte detached signature.
    pub fn sign(&self, msg: &[u8]) -> [u8; 64] {
        self.signing.sign(msg).to_bytes()
    }
}

/// Verify a detached ed25519 signature. Returns false on any decode or
/// verification failure (never panics).
pub fn verify(public: &[u8; 32], msg: &[u8], sig: &[u8; 64]) -> bool {
    let vk = match VerifyingKey::from_bytes(public) {
        Ok(vk) => vk,
        Err(_) => return false,
    };
    let signature = Signature::from_bytes(sig);
    vk.verify(msg, &signature).is_ok()
}
