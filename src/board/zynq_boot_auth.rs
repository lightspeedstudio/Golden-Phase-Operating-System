// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Mission-ready Stage 1 auth: fail closed.
//!
//! - Domain-separated `BootImageRoot` (kernel PhaseHash)
//! - Ed25519 over that 32-byte root (kernel `sign::verify`)
//! - Reject Xilinx encrypted / RSA partitions (we do not run vendor crypto)
//!
//! The compiled verifying key is the **development** LightSpeed Studio root.
//! Production boards must replace [`STAGE1_ROOT_PUBKEY`]. The private seed is
//! not linked into the bootloader.

use crate::kernel::crypto::sign;
use crate::kernel::hash::StateHash;

/// Stage 1 verifying key (public half only). RFC 8032 test-1 public key.
///
/// Development / bring-up. Replace with a factory-provisioned key before
/// production. This constant is not a secret.
pub const STAGE1_ROOT_PUBKEY: [u8; 32] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

/// RFC 8032 test-1 secret. Sign tool and tests only — not compiled into Stage 1.
#[cfg(any(test, feature = "std"))]
pub fn dev_only_seed() -> [u8; 32] {
    [
        0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c,
        0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae,
        0x7f, 0x60,
    ]
}

/// Detached Ed25519 signature length.
pub const SIG_LEN: usize = 64;

/// Split `BOOT.BIN || sig[64]` into image and signature. Fail closed if short.
pub fn split_signed(boot: &[u8]) -> Option<(&[u8], &[u8; 64])> {
    if boot.len() < SIG_LEN {
        return None;
    }
    let (img, sig) = boot.split_at(boot.len() - SIG_LEN);
    let sig = sig.try_into().ok()?;
    Some((img, sig))
}

/// Verify Ed25519(sig) over the 32-byte BootImageRoot.
pub fn verify_image_root(pubkey: &[u8; 32], root: &StateHash, sig: &[u8; 64]) -> bool {
    sign::verify(pubkey, root, sig)
}

#[cfg(any(test, feature = "std"))]
pub fn sign_image_root(seed: &[u8; 32], root: &StateHash) -> [u8; 64] {
    sign::SignKeypair::from_seed(seed).sign(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::crypto::phase_hash::{ph_boot_image_root, ph_content};

    #[test]
    fn reject_short_trailer() {
        assert!(split_signed(&[0u8; 63]).is_none());
    }

    #[test]
    fn signed_root_accepts_and_rejects_tamper() {
        let seed = dev_only_seed();
        let kp = sign::SignKeypair::from_seed(&seed);
        let a = ph_content(b"stage1");
        let b = ph_content(b"bit");
        let k = ph_content(b"kernel");
        let root = ph_boot_image_root(&a, &b, &k);
        let sig = kp.sign(&root);
        let pk = kp.public_bytes();
        assert_eq!(pk, STAGE1_ROOT_PUBKEY);
        assert!(verify_image_root(&pk, &root, &sig));
        let mut bad = root;
        bad[0] ^= 1;
        assert!(!verify_image_root(&pk, &bad, &sig));
        let mut badsig = sig;
        badsig[0] ^= 1;
        assert!(!verify_image_root(&pk, &root, &badsig));
    }
}
