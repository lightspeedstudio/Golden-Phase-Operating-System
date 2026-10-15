//! `R_PS` and `K_authority`. MEASURED equation/kernel roots wrapped in PhaseHash.

use crate::kernel::crypto::phase_hash::{ph_sentinel_authority, ph_sentinel_root};
use crate::kernel::hash::StateHash;

use super::policy::SentinelPolicy;

/// `K_authority = PH(GPOS/PhaseSentinel/authority/v1, PK_root)`.
/// AUTHENTICATED after the pinned key verifies Ed25519 signatures. The pin
/// itself is operator-supplied (same honesty rule as identity `root_pubkey_hash`).
pub fn k_authority(root_pubkey: &[u8; 32]) -> StateHash {
    ph_sentinel_authority(root_pubkey)
}

/// `R_PS = PH(GPOS/PhaseSentinel/root/v1, [R_eq, R_kernel, R_policy, K_authority, g])`.
/// `R_eq` and `R_kernel` are MEASURED_ROOT values consumed from the ledger.
pub fn r_ps(
    equation_root: &StateHash,
    kernel_root: &StateHash,
    policy: &SentinelPolicy,
    root_pubkey: &[u8; 32],
    generation: u64,
) -> StateHash {
    ph_sentinel_root(
        equation_root,
        kernel_root,
        &policy.root(),
        &k_authority(root_pubkey),
        generation,
    )
}
