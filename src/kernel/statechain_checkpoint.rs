//! HMAC-SHA256 integrity MAC over kernel-root checkpoints.
//!
//! ## Honest disclosure: integrity-only, not adversarial auth
//! The MAC key is `tagged_hash(SIGNING_KEY_TAG, equation_root)`. `equation_root`
//! is computed from the published Golden Phase equation
//! ([`crate::kernel::state_root::compute_equation_root`]) and is stored in the
//! checkpoint blob, so the key is public-derivable. Anyone who can read a
//! checkpoint can recompute a valid MAC for a mutated `kernel_root`.
//!
//! This is an integrity MAC against accidental corruption on a trusted
//! operator path. It is acceptable for alpha. It is not a signature and is
//! not an adversarial threat model.
//!
//! Adversarial sealing is [`crate::kernel::checkpoint_auth`] (secret-key
//! HMAC-SHA256 over a GPC1 record). This V1/V2 module stays integrity-only
//! and is never used to create authenticated checkpoints.
//!
//! The `signature` field name is the HMAC tag (alpha API stability).

use crate::kernel::hash::{tagged_hash, StateHash};
use crate::kernel::state_root::compute_prime_log_commitment;
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub const CHECKPOINT_TAG: &str = "KAIROS_SIGNED_CHECKPOINT_V1";
#[cfg(feature = "phasestack")]
pub const CHECKPOINT_TAG_V2: &str = "KAIROS_SIGNED_CHECKPOINT_V2";
pub const SIGNING_KEY_TAG: &str = "KAIROS_STATECHAIN_SIGNING_KEY_V1";

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedCheckpoint {
    pub sequence: u64,
    pub tick: u64,
    pub kernel_root: StateHash,
    pub equation_root: StateHash,
    pub prime_log: StateHash,
    #[cfg(feature = "phasestack")]
    pub app_registry_root: StateHash,
    /// HMAC-SHA256 tag over [`checkpoint_body`]. Not an authentication
    /// signature; see module docs.
    pub signature: StateHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointError {
    /// HMAC tag mismatch (integrity MAC, not an adversarial signature).
    BadSignature,
    Empty,
}

/// Derive the checkpoint HMAC key from `equation_root`.
///
/// The result is not a secret: `equation_root` is public, so this key is
/// public-derivable. See module docs.
pub fn derive_signing_key(equation_root: &StateHash) -> StateHash {
    tagged_hash(SIGNING_KEY_TAG, &[equation_root])
}

pub fn checkpoint_body(
    sequence: u64,
    tick: u64,
    kernel_root: &StateHash,
    equation_root: &StateHash,
    prime_log: &StateHash,
) -> StateHash {
    tagged_hash(
        CHECKPOINT_TAG,
        &[
            &sequence.to_le_bytes(),
            &tick.to_le_bytes(),
            kernel_root,
            equation_root,
            prime_log,
        ],
    )
}

/// Seal a checkpoint with the HMAC integrity MAC.
///
/// `signing_key` must be [`derive_signing_key`] of `equation_root`.
/// This authenticates nothing against a party who can recompute that key.
pub fn sign_checkpoint(
    signing_key: &StateHash,
    sequence: u64,
    tick: u64,
    kernel_root: &StateHash,
    equation_root: &StateHash,
    prime_log: &StateHash,
) -> SignedCheckpoint {
    let body = checkpoint_body(sequence, tick, kernel_root, equation_root, prime_log);
    let signature = hmac_sign(signing_key, &body);
    SignedCheckpoint {
        sequence,
        tick,
        kernel_root: *kernel_root,
        equation_root: *equation_root,
        prime_log: *prime_log,
        #[cfg(feature = "phasestack")]
        app_registry_root: [0u8; 32],
        signature,
    }
}

/// Check the HMAC integrity tag. Does not prove origin under an adversarial
/// model; see module docs.
pub fn verify_checkpoint(signing_key: &StateHash, checkpoint: &SignedCheckpoint) -> bool {
    let body = checkpoint_body(
        checkpoint.sequence,
        checkpoint.tick,
        &checkpoint.kernel_root,
        &checkpoint.equation_root,
        &checkpoint.prime_log,
    );
    let expected = hmac_sign(signing_key, &body);
    expected == checkpoint.signature
        && checkpoint.kernel_root != [0u8; 32]
        && checkpoint.equation_root != [0u8; 32]
}

#[cfg(feature = "phasestack")]
pub fn checkpoint_body_v2(
    sequence: u64,
    tick: u64,
    kernel_root: &StateHash,
    equation_root: &StateHash,
    prime_log: &StateHash,
    app_registry_root: &StateHash,
) -> StateHash {
    tagged_hash(
        CHECKPOINT_TAG_V2,
        &[
            &sequence.to_le_bytes(),
            &tick.to_le_bytes(),
            kernel_root,
            equation_root,
            prime_log,
            app_registry_root,
        ],
    )
}

/// Same integrity MAC as [`sign_checkpoint`], with `app_registry_root` in the body.
#[cfg(feature = "phasestack")]
pub fn sign_checkpoint_v2(
    signing_key: &StateHash,
    sequence: u64,
    tick: u64,
    kernel_root: &StateHash,
    equation_root: &StateHash,
    prime_log: &StateHash,
    app_registry_root: &StateHash,
) -> SignedCheckpoint {
    let body = checkpoint_body_v2(
        sequence,
        tick,
        kernel_root,
        equation_root,
        prime_log,
        app_registry_root,
    );
    let signature = hmac_sign(signing_key, &body);
    SignedCheckpoint {
        sequence,
        tick,
        kernel_root: *kernel_root,
        equation_root: *equation_root,
        prime_log: *prime_log,
        app_registry_root: *app_registry_root,
        signature,
    }
}

/// Same integrity check as [`verify_checkpoint`], against the v2 body.
#[cfg(feature = "phasestack")]
pub fn verify_checkpoint_v2(signing_key: &StateHash, checkpoint: &SignedCheckpoint) -> bool {
    let body = checkpoint_body_v2(
        checkpoint.sequence,
        checkpoint.tick,
        &checkpoint.kernel_root,
        &checkpoint.equation_root,
        &checkpoint.prime_log,
        &checkpoint.app_registry_root,
    );
    let expected = hmac_sign(signing_key, &body);
    expected == checkpoint.signature
        && checkpoint.kernel_root != [0u8; 32]
        && checkpoint.equation_root != [0u8; 32]
}

pub fn prime_log_for_equation(equation_root: &StateHash) -> StateHash {
    compute_prime_log_commitment(&crate::kernel::state_root::EquationRoot {
        hash: *equation_root,
    })
    .hash
}

fn hmac_sign(key: &StateHash, message: &StateHash) -> StateHash {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts 32-byte keys");
    mac.update(message);
    mac.finalize().into_bytes().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_checkpoint() -> (StateHash, SignedCheckpoint) {
        let equation_root = crate::kernel::state_root::compute_equation_root().hash;
        let key = derive_signing_key(&equation_root);
        let kernel_root = tagged_hash("KAIROS_CHECKPOINT_TEST_KERNEL", &[b"root"]);
        let prime_log = prime_log_for_equation(&equation_root);
        let checkpoint = sign_checkpoint(&key, 0, 1, &kernel_root, &equation_root, &prime_log);
        (key, checkpoint)
    }

    #[test]
    fn honest_seal_verifies() {
        let (key, checkpoint) = sample_checkpoint();
        assert!(verify_checkpoint(&key, &checkpoint));
    }

    #[test]
    fn public_derivable_key_forges_mutated_kernel_root() {
        let (_honest_key, honest) = sample_checkpoint();
        let key = derive_signing_key(&honest.equation_root);
        let mut kernel_root = honest.kernel_root;
        kernel_root[0] ^= 1;
        let forged = sign_checkpoint(
            &key,
            honest.sequence,
            honest.tick,
            &kernel_root,
            &honest.equation_root,
            &honest.prime_log,
        );
        assert_ne!(forged.kernel_root, honest.kernel_root);
        assert!(verify_checkpoint(&key, &forged));
    }

    #[test]
    fn hmac_tag_is_32_bytes_not_ed25519_64() {
        let (key, checkpoint) = sample_checkpoint();
        assert_eq!(checkpoint.signature.len(), 32);
        let body = checkpoint_body(
            checkpoint.sequence,
            checkpoint.tick,
            &checkpoint.kernel_root,
            &checkpoint.equation_root,
            &checkpoint.prime_log,
        );
        let kp = crate::kernel::crypto::sign::SignKeypair::from_seed(&key);
        let ed = kp.sign(&body);
        assert_eq!(ed.len(), 64);
        assert_ne!(&ed[..32], &checkpoint.signature[..]);
        let mut padded = [0u8; 64];
        padded[..32].copy_from_slice(&checkpoint.signature);
        assert!(!crate::kernel::crypto::sign::verify(
            &kp.public_bytes(),
            &body,
            &padded
        ));
        assert_ne!(CHECKPOINT_TAG.as_bytes(), b"PSC1");
    }

    #[cfg(feature = "phasesentinel")]
    #[test]
    fn psc1_seal_is_ed25519_not_hmac_checkpoint() {
        use crate::kernel::sentinel::SentinelCheckpoint;
        let (hmac_key, hmac_cp) = sample_checkpoint();
        let mut psc1 = SentinelCheckpoint {
            generation: 1,
            content_root: hmac_cp.kernel_root,
            r_state: hmac_cp.kernel_root,
            r_memory: hmac_cp.kernel_root,
            r_program: hmac_cp.kernel_root,
            r_ps: [0u8; 32],
            signature: [0u8; 64],
        };
        assert!(psc1.canonical_tbs().starts_with(b"PSC1"));
        psc1.sign(&hmac_key);
        assert_eq!(psc1.signature.len(), 64);
        let pk = crate::kernel::crypto::sign::SignKeypair::from_seed(&hmac_key).public_bytes();
        assert!(psc1.verify(&pk).is_ok());
        assert!(!verify_checkpoint(&hmac_key, &SignedCheckpoint {
            signature: {
                let mut tag = [0u8; 32];
                tag.copy_from_slice(&psc1.signature[..32]);
                tag
            },
            ..hmac_cp.clone()
        }));
    }
}
