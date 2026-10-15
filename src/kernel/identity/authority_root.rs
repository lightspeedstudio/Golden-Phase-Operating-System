//! `AuthorityRoot` (`A_0`) — the identity subsystem's mathematical/kernel
//! trust anchor (Gate 1). Thin wrapper over
//! [`crate::kernel::crypto::phase_hash::ph_authority_root`], exactly the
//! same pattern [`crate::kernel::boot_session`] already uses for
//! `BootSessionRoot` over `ph_boot_session_root`.
//!
//! ## Trust-chain dependency order (why there is no cycle)
//! The platform spec's intended sequence is:
//! `EquationRoot + installation identity + Root public-key hash + Root
//! policy -> AuthorityRoot -> KernelRoot -> BootSessionRoot -> boot-bound
//! sessions/capabilities`.
//!
//! This module computes `AuthorityRoot` from **only** `EquationRoot` (real,
//! already computed by [`crate::kernel::state_root::compute_equation_root`])
//! plus three identity-local inputs (installation ID, pinned Root
//! public-key hash, policy hash) — never from `KernelRoot` or
//! `BootSessionRoot`. Those two continue to be computed exactly as before
//! (`kernel::state_root::compute_kernel_state_root`,
//! `kernel::boot_session::compute_boot_session_root`) and are **not**
//! modified by this gate: folding `AuthorityRoot` byte-for-byte into
//! `KernelRoot`'s tagged-hash payload would change `KernelRoot` (and thus
//! `BootSessionRoot`) for every build, including ones with `identity` off —
//! violating this build program's "default sealed boot must be unchanged
//! when the feature is off" invariant (see `Cargo.toml`'s doc comment on
//! every one of the eight prior subsystem features).
//!
//! `AuthorityRoot` therefore sits *logically* between `EquationRoot` and
//! `KernelSessionRoot` per the spec's diagram, but is *computed*
//! independently, alongside them, consumed by later gates (certificates,
//! sessions, the identity ledger's `AuthorityStateRoot`) as their own trust
//! anchor. This is the same "consumes, never redefines" relationship
//! `netsess`'s `session_id` already has with `BootSessionRoot` (see
//! `kernel::netsess` module docs) — a real, declared, one-directional
//! dependency, not a hidden cycle.
//!
//! ## Two Root keys (per platform spec §3) — not yet distinguished in code
//! The spec requires distinguishing the **offline Root authority/CA private
//! key** (signs the Root account certificate; never embedded in the kernel)
//! from the **interactive Root account's own certificate/login key**. Gate 1
//! only carries `root_pubkey_hash = H(PK_root)` — the pinned Root
//! *authority* public key's hash — as an opaque 32-byte input; no private
//! key of either kind exists in this module or is ever accepted by it. The
//! account-level certificate/login-key distinction is Gate 2/3 work.

use crate::kernel::crypto::phase_hash::{ph_authority_root, AuthorityRootInput};
use crate::kernel::hash::StateHash;

/// `A_0 = PH_authority-root(E, I, H(PK_root), P)`. Pure; callers recompute
/// whenever `installation_id`, `root_pubkey_hash`, or `policy_hash` changes
/// (see [`crate::kernel::identity::Identity::recompute_authority_root`]).
pub fn compute_authority_root(
    equation_root: StateHash,
    installation_id: StateHash,
    root_pubkey_hash: StateHash,
    policy_hash: StateHash,
) -> StateHash {
    ph_authority_root(&AuthorityRootInput {
        equation_root,
        installation_id,
        root_pubkey_hash,
        policy_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_authority_root_is_deterministic_and_input_sensitive() {
        let a = compute_authority_root([1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]);
        let b = compute_authority_root([1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]);
        assert_eq!(a, b);
        let c = compute_authority_root([1u8; 32], [2u8; 32], [3u8; 32], [9u8; 32]);
        assert_ne!(a, c);
    }
}
