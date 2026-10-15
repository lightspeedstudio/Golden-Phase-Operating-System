//! PhaseHash v1 — domain-separated, length-prefixed tuple-hash construction
//! (SHA-256 backend) used for every content-addressed root in GPOS:
//!
//! - **PH-C** (`ph_content`)       — raw content hash.
//! - **PH-O** (`ph_orbit`)         — Kairos-State symmetry-orbit hash; invariant
//!   under [`crate::core::gphase_core::reflect`] (`rho(x) = 3 - x`), so a state
//!   and its mirror hash identically.
//! - **PH-S** (`ph_state`)         — chained per-object state hash (previous
//!   link + Chronos tick + Kairos-sector + metadata).
//! - **PH-R** (`ph_root` / `ph_boot_image_root`) — root hashes over ordered
//!   sets of leaves (filesystem / boot-image roots).
//! - `ph_boot_session_root`        — dedicated boot-chain variant binding the
//!   equation root, init PID, kernel root, and boot tick into one session root.
//! - `ph_authority_root`           — identity subsystem's `A_0`: binds the
//!   equation root, installation identity, pinned Root public-key hash, and
//!   Root policy hash into one mathematical trust anchor (see
//!   `kernel::identity::authority_root`).
//! - `ph_cert_tbs` / `ph_cert`     — identity subsystem's certificate
//!   to-be-signed hash and outer certificate hash (see
//!   `kernel::identity::certificate`).
//! - `ph_identity_chain`           — identity subsystem's ledger hash-chain
//!   link (`L_n` from `L_{n-1}`, see `kernel::identity::ledger`).
//! - `ph_authority_state_root`     — identity subsystem's `AuthorityStateRoot`:
//!   binds `A_0`, the ledger chain head, and the five per-category Merkle
//!   roots (Principal/Certificate/Credential/Revocation/Policy) into one
//!   value, which is what gets checkpointed into the real `StatechainLedger`
//!   (see `kernel::identity::ledger`).
//! - `ph_tensor_proof` (`feature = "tensor"`) — binds packed `TensorState`
//!   plus sequence, previous root, BootSessionRoot, authority generation,
//!   and Chronos. Tamper detection only; matching V still does not grant.
//!
//! ## Construction
//! `tuple_hash(domain, parts)` hashes `MAGIC || len(domain) || domain ||
//! count(parts) || (len(part) || part)*` through SHA-256. The magic, the
//! explicit domain length prefix, and the explicit part count make every
//! encoding unambiguous: no two distinct `(domain, parts)` inputs can ever
//! serialize to the same byte string, so collisions in the pre-image imply a
//! SHA-256 collision.

use crate::core::gphase_core::{self, PhaseState};
use crate::kernel::hash::StateHash;
use crate::platform::Vec;
use sha2::{Digest, Sha256};

/// Fixed 6-byte construction magic, hashed first so PhaseHash v1 digests can
/// never collide with a bare/undomained SHA-256 of the same bytes.
const MAGIC: &[u8; 6] = b"GP-PH1";

pub const DOMAIN_CONTENT: &str = "GPOS/PhaseHash/content/v1";
pub const DOMAIN_ORBIT: &str = "GPOS/PhaseHash/orbit/v1";
pub const DOMAIN_STATE: &str = "GPOS/PhaseHash/state/v1";
pub const DOMAIN_ROOT: &str = "GPOS/PhaseHash/root/v1";
pub const DOMAIN_BOOT_IMAGE_ROOT: &str = "GPOS/PhaseHash/boot-image-root/v1";
pub const DOMAIN_BOOT_SESSION_ROOT: &str = "GPOS/PhaseHash/boot-session-root/v1";
pub const DOMAIN_IDENTITY_AUTHORITY_ROOT: &str = "GPOS/PhaseHash/identity-authority-root/v1";
pub const DOMAIN_IDENTITY_CERT_TBS: &str = "GPOS/PhaseHash/identity-cert-tbs/v1";
pub const DOMAIN_IDENTITY_CERT: &str = "GPOS/PhaseHash/identity-cert/v1";
pub const DOMAIN_IDENTITY_CHAIN: &str = "GPOS/PhaseHash/identity-chain/v1";
pub const DOMAIN_IDENTITY_AUTHORITY_STATE: &str = "GPOS/PhaseHash/identity-authority-state/v1";
pub const DOMAIN_IDENTITY_SESSION: &str = "GPOS/PhaseHash/identity-session/v1";
pub const DOMAIN_IDENTITY_CAPABILITY: &str = "GPOS/PhaseHash/identity-capability/v1";
/// Integrity binding for a packed tensor snapshot. Behind `feature = "tensor"`.
/// Not cryptographic authorization. `1/256` packed-state match is not this hash.
#[cfg(feature = "tensor")]
pub const DOMAIN_TENSOR_PROOF: &str = "GPOS/PhaseHash/tensor-proof/v1";
#[cfg(feature = "phasestack")]
pub const DOMAIN_DEVELOPER_CERT_TBS: &str = "GPOS/PhaseHash/developer-cert-tbs/v1";
#[cfg(feature = "phasestack")]
pub const DOMAIN_DEVELOPER_CERT: &str = "GPOS/PhaseHash/developer-cert/v1";
#[cfg(feature = "phasestack")]
pub const DOMAIN_APP_ID: &str = "GPOS/PhaseHash/app-id/v1";
#[cfg(feature = "phasestack")]
pub const DOMAIN_MANIFEST: &str = "GPOS/PhaseHash/manifest/v1";
#[cfg(feature = "phasestack")]
pub const DOMAIN_CODE: &str = "GPOS/PhaseHash/code/v1";
#[cfg(feature = "phasestack")]
pub const DOMAIN_HOOK_MANIFEST: &str = "GPOS/PhaseHash/hook-manifest/v1";
#[cfg(feature = "phasestack")]
pub const DOMAIN_PACKAGE_ROOT: &str = "GPOS/PhaseHash/package-root/v1";
#[cfg(feature = "phasestack")]
pub const DOMAIN_APP_REGISTRY: &str = "GPOS/PhaseHash/app-registry/v1";
#[cfg(feature = "phasesentinel")]
pub const DOMAIN_SENTINEL_ROOT: &str = "GPOS/PhaseSentinel/root/v1";
#[cfg(feature = "phasesentinel")]
pub const DOMAIN_SENTINEL_MANIFEST: &str = "GPOS/PhaseSentinel/manifest/v1";
#[cfg(feature = "phasesentinel")]
pub const DOMAIN_SENTINEL_POLICY: &str = "GPOS/PhaseSentinel/policy/v1";
#[cfg(feature = "phasesentinel")]
pub const DOMAIN_SENTINEL_AUTHORITY: &str = "GPOS/PhaseSentinel/authority/v1";
#[cfg(feature = "phasesentinel")]
pub const DOMAIN_SENTINEL_CHECKPOINT: &str = "GPOS/PhaseSentinel/checkpoint/v1";
#[cfg(feature = "phasesentinel")]
pub const DOMAIN_SENTINEL_EVIDENCE: &str = "GPOS/PhaseSentinel/evidence/v1";
#[cfg(all(feature = "phasesentinel", feature = "phasestack"))]
pub const DOMAIN_SENTINEL_LAUNCH: &str = "GPOS/PhaseSentinel/launch-receipt/v1";

fn write_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u32).to_le_bytes());
    hasher.update(bytes);
}

/// Core tuple-hash construction: `MAGIC || u32(len(domain)) || domain ||
/// u32(count(parts)) || { u32(len(part)) || part }`.
pub fn tuple_hash(domain: &str, parts: &[&[u8]]) -> StateHash {
    let mut hasher = Sha256::new();
    hasher.update(MAGIC);
    write_len_prefixed(&mut hasher, domain.as_bytes());
    hasher.update((parts.len() as u32).to_le_bytes());
    for part in parts {
        write_len_prefixed(&mut hasher, part);
    }
    hasher.finalize().into()
}

/// PH-C: content hash of an arbitrary byte string.
pub fn ph_content(data: &[u8]) -> StateHash {
    tuple_hash(DOMAIN_CONTENT, &[data])
}

/// Canonical 2-element reflection orbit `{state, reflect(state)}` for `state`,
/// ordered by discriminant so both orbit members map to the same pair.
/// `reflect(x) = 3 - x` on `PhaseState`'s four discrete values has no fixed
/// points (0<->3, 1<->2), so every orbit has exactly two members.
pub fn ph_reflection_orbit(state: PhaseState) -> [PhaseState; 2] {
    let mirror = gphase_core::reflect(state);
    if state.as_u8() <= mirror.as_u8() {
        [state, mirror]
    } else {
        [mirror, state]
    }
}

/// PH-O: symmetry-orbit hash. `ph_orbit(s) == ph_orbit(reflect(s))` for every
/// `s`, because both hash the same canonical orbit pair.
pub fn ph_orbit(state: PhaseState) -> StateHash {
    let orbit = ph_reflection_orbit(state);
    tuple_hash(DOMAIN_ORBIT, &[&[orbit[0].as_u8()], &[orbit[1].as_u8()]])
}

/// Input to PH-S: a chained per-object state hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateInput {
    /// Previous PH-S output in this object's chain (`[0u8; 32]` for genesis).
    pub previous: StateHash,
    pub chronos: u64,
    pub kairos_sector: u8,
    /// Hash of whatever content/metadata this state transition is over.
    pub metadata_hash: StateHash,
}

/// PH-S: state hash binding the previous link, live Chronos tick, live
/// Kairos-sector, and a metadata hash. Sensitive to every one of its inputs.
pub fn ph_state(input: &StateInput) -> StateHash {
    tuple_hash(
        DOMAIN_STATE,
        &[
            &input.previous,
            &input.chronos.to_le_bytes(),
            &[input.kairos_sector],
            &input.metadata_hash,
        ],
    )
}

/// PH-R: root hash over an ordered list of leaf hashes.
pub fn ph_root(leaves: &[StateHash]) -> StateHash {
    let parts: Vec<&[u8]> = leaves.iter().map(|leaf| leaf.as_slice()).collect();
    tuple_hash(DOMAIN_ROOT, &parts)
}

/// PH-R variant: `BootImageRoot`, unique to one authenticated (FSBL,
/// bitstream, kernel) triad.
pub fn ph_boot_image_root(
    fsbl_hash: &StateHash,
    bitstream_hash: &StateHash,
    kernel_hash: &StateHash,
) -> StateHash {
    tuple_hash(
        DOMAIN_BOOT_IMAGE_ROOT,
        &[fsbl_hash, bitstream_hash, kernel_hash],
    )
}

/// Input to `ph_boot_session_root`: everything that must be re-derived
/// identically to prove two boots share the same equation/kernel/PID/tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootSessionRootInput {
    pub equation_root: StateHash,
    pub init_pid: u32,
    pub kernel_root: StateHash,
    pub boot_tick: u64,
}

/// `BootSessionRoot`: unique per physical boot, binding the statechain
/// equation root, init PID, kernel root, and boot tick.
pub fn ph_boot_session_root(input: &BootSessionRootInput) -> StateHash {
    tuple_hash(
        DOMAIN_BOOT_SESSION_ROOT,
        &[
            &input.equation_root,
            &input.init_pid.to_le_bytes(),
            &input.kernel_root,
            &input.boot_tick.to_le_bytes(),
        ],
    )
}

/// Input to `ph_authority_root`: everything the identity subsystem's
/// mathematical trust anchor (`A_0`) binds. See
/// `kernel::identity::authority_root` module docs for the full trust-chain
/// rationale (why this depends only on `equation_root`, never on
/// `BootSessionRoot`, so the dependency graph stays acyclic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthorityRootInput {
    /// The real, already-computed `EquationRoot` (`E`).
    pub equation_root: StateHash,
    /// Installation/device identity (`I`). See module docs on
    /// `kernel::identity` for the honest disclosure that no hardware
    /// DNA/eFuse reader exists in this repo yet — this is an
    /// operator/deployment-supplied value, not a hardware-read unique ID.
    pub installation_id: StateHash,
    /// `H(PK_root)` — hash of the pinned Root public key. `[0u8; 32]` before
    /// Root has ever been provisioned (see `RootCredentialState::Unprovisioned`
    /// in a later gate); a real value only once a pinned Root authority
    /// public key exists.
    pub root_pubkey_hash: StateHash,
    /// Root policy hash (`P`) — hash of the current Root/authority policy
    /// record. Defaults to the hash of the empty/default policy until a real
    /// policy is provisioned.
    pub policy_hash: StateHash,
}

/// `A_0 = PH_authority-root(E, I, H(PK_root), P)` — the identity subsystem's
/// mathematical/kernel trust anchor. Pure and recomputable: callers
/// recompute this whenever `installation_id`, `root_pubkey_hash`, or
/// `policy_hash` changes (e.g. Root provisioning, policy change), rather
/// than caching a value that silently goes stale.
pub fn ph_authority_root(input: &AuthorityRootInput) -> StateHash {
    tuple_hash(
        DOMAIN_IDENTITY_AUTHORITY_ROOT,
        &[
            &input.equation_root,
            &input.installation_id,
            &input.root_pubkey_hash,
            &input.policy_hash,
        ],
    )
}

/// `PH_cert-tbs(TBS_u)` — hash of a certificate's canonical to-be-signed
/// encoding. This is the message the issuer's Ed25519 key actually signs
/// (`Signature_u = Sign_SK_issuer(PH_cert-tbs(TBS_u))`) — see
/// `kernel::identity::certificate::encode_tbs`.
pub fn ph_cert_tbs(tbs_bytes: &[u8]) -> StateHash {
    tuple_hash(DOMAIN_IDENTITY_CERT_TBS, &[tbs_bytes])
}

/// `C_u = PH_cert(TBS_u, Signature_u)` — the outer certificate hash, binding
/// the raw TBS bytes *and* the signature together (so revoking/looking up
/// "this exact signed object" is unambiguous, distinct from `ph_cert_tbs`
/// which only ever covers the pre-signature bytes).
pub fn ph_cert(tbs_bytes: &[u8], signature: &[u8; 64]) -> StateHash {
    tuple_hash(DOMAIN_IDENTITY_CERT, &[tbs_bytes, signature])
}

/// Input to `ph_identity_chain`: one link of the identity ledger's own
/// hash-chained event log (`kernel::identity::ledger`). Every field is
/// bound, so tampering with any one of them (reordering, forging a
/// principal, replaying a stale tick, substituting a different payload)
/// changes the resulting link and breaks the chain from that point forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentityChainInput {
    /// Previous link's output (`[0u8; 32]` for the genesis event).
    pub previous: StateHash,
    /// Monotonic event sequence number within the ledger.
    pub sequence: u64,
    /// `IdentityEventKind::as_u8()`.
    pub kind: u8,
    /// The principal the event is about (`kernel::identity::principal::PrincipalId`).
    pub principal_id: u32,
    /// Live Chronos/statechain tick the event was recorded at.
    pub tick: u64,
    /// `ph_content` hash of the event's own (never-plaintext) payload.
    pub payload_hash: StateHash,
}

/// `L_n = PH_identity-chain(L_{n-1}, sequence, kind, principal_id, tick,
/// payload_hash)` — one link of the identity ledger's hash chain. See
/// `kernel::identity::ledger::IdentityLedger::record`.
pub fn ph_identity_chain(input: &IdentityChainInput) -> StateHash {
    tuple_hash(
        DOMAIN_IDENTITY_CHAIN,
        &[
            &input.previous,
            &input.sequence.to_le_bytes(),
            &[input.kind],
            &input.principal_id.to_le_bytes(),
            &input.tick.to_le_bytes(),
            &input.payload_hash,
        ],
    )
}

/// Input to `ph_authority_state_root`: everything the identity subsystem's
/// live-state commitment binds. See `kernel::identity::ledger` module docs
/// for the full rationale (why this is a distinct value from `A_0`
/// (`AuthorityRoot`) itself, and why it is what gets checkpointed into the
/// real `StatechainLedger`, not `A_0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthorityStateRootInput {
    /// The identity subsystem's mathematical trust anchor (`A_0`).
    pub authority_root: StateHash,
    /// The identity ledger's current hash-chain head (`L_n`).
    pub chain_head: StateHash,
    /// Merkle root over every `PrincipalCreated`/`Disabled`/`Enabled` event
    /// hash recorded so far.
    pub principal_root: StateHash,
    /// Merkle root over every `RootCertificateInstalled`/`CertificateIssued`
    /// event hash recorded so far.
    pub certificate_root: StateHash,
    /// Merkle root over every `CredentialProvisioned`/`Changed`/`Locked`/
    /// `Unlocked`/`RecoveryRequired` event hash recorded so far.
    pub credential_root: StateHash,
    /// Merkle root over every `CertificateRevoked` event hash recorded so
    /// far.
    pub revocation_root: StateHash,
    /// Merkle root over every `PolicyChanged` event hash recorded so far.
    pub policy_root: StateHash,
}

/// `AuthorityStateRoot = PH_authority-state(A_0, L_n, PrincipalRoot,
/// CertificateRoot, CredentialRoot, RevocationRoot, PolicyRoot)` — the
/// identity subsystem's live-state commitment, recomputed after every
/// ledger event and checkpointed into the real `StatechainLedger` (see
/// `Kernel::identity_checkpoint_statechain`).
pub fn ph_authority_state_root(input: &AuthorityStateRootInput) -> StateHash {
    tuple_hash(
        DOMAIN_IDENTITY_AUTHORITY_STATE,
        &[
            &input.authority_root,
            &input.chain_head,
            &input.principal_root,
            &input.certificate_root,
            &input.credential_root,
            &input.revocation_root,
            &input.policy_root,
        ],
    )
}

/// `PH_identity-session(principal, origin, method, boot_session_root, opened_tick)`
/// — binds a live user session to the boot it was opened on (Gate 7).
pub fn ph_identity_session(
    principal_id: u32,
    origin: u8,
    method: u8,
    boot_session_root: &StateHash,
    opened_tick: u64,
) -> StateHash {
    tuple_hash(
        DOMAIN_IDENTITY_SESSION,
        &[
            &principal_id.to_le_bytes(),
            &[origin],
            &[method],
            boot_session_root,
            &opened_tick.to_le_bytes(),
        ],
    )
}

/// `PH_identity-capability(principal, session, op, generation)` — capability
/// proof hash for identity-scoped `mod` rights (Gate 8). Distinct from
/// `captable` object-handle capabilities.
pub fn ph_identity_capability(
    principal_id: u32,
    session_id: u32,
    op: u8,
    generation: u64,
) -> StateHash {
    tuple_hash(
        DOMAIN_IDENTITY_CAPABILITY,
        &[
            &principal_id.to_le_bytes(),
            &session_id.to_le_bytes(),
            &[op],
            &generation.to_le_bytes(),
        ],
    )
}

/// Inputs to [`ph_tensor_proof`]. `tensor` is the canonical little-endian
/// one-byte packed payload (`TensorState::as_u8`).
#[cfg(feature = "tensor")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TensorProofInput {
    pub tensor: u8,
    pub sequence: u64,
    pub previous_root: StateHash,
    pub boot_session_root: StateHash,
    pub authority_generation: u64,
    pub chronos: u64,
}

/// `PH_tensor-proof(TensorState, Sequence, PreviousRoot, BootSessionRoot,
/// AuthorityGeneration, Chronos)`.
///
/// Integrity / replay binding for an advisory tensor result. Does not
/// authorize. Does not replace `authorize`, session, or PhaseGuard.
#[cfg(feature = "tensor")]
pub fn ph_tensor_proof(input: &TensorProofInput) -> StateHash {
    tuple_hash(
        DOMAIN_TENSOR_PROOF,
        &[
            &[input.tensor],
            &input.sequence.to_le_bytes(),
            &input.previous_root,
            &input.boot_session_root,
            &input.authority_generation.to_le_bytes(),
            &input.chronos.to_le_bytes(),
        ],
    )
}

#[cfg(feature = "phasestack")]
pub fn ph_developer_cert_tbs(tbs_bytes: &[u8]) -> StateHash {
    tuple_hash(DOMAIN_DEVELOPER_CERT_TBS, &[tbs_bytes])
}

#[cfg(feature = "phasestack")]
pub fn ph_developer_cert(tbs_bytes: &[u8], signature: &[u8; 64]) -> StateHash {
    tuple_hash(DOMAIN_DEVELOPER_CERT, &[tbs_bytes, signature])
}

#[cfg(feature = "phasestack")]
pub fn ph_app_id(developer_cert_hash: &StateHash, canonical_name: &[u8]) -> StateHash {
    tuple_hash(DOMAIN_APP_ID, &[developer_cert_hash, canonical_name])
}

#[cfg(feature = "phasestack")]
pub fn ph_manifest(canonical_manifest: &[u8]) -> StateHash {
    tuple_hash(DOMAIN_MANIFEST, &[canonical_manifest])
}

#[cfg(feature = "phasestack")]
pub fn ph_code(exact_bytes: &[u8]) -> StateHash {
    tuple_hash(DOMAIN_CODE, &[exact_bytes])
}

#[cfg(feature = "phasestack")]
pub fn ph_hook_manifest(canonical_hooks: &[u8]) -> StateHash {
    tuple_hash(DOMAIN_HOOK_MANIFEST, &[canonical_hooks])
}

#[cfg(feature = "phasestack")]
pub fn ph_package_root(
    app_id: &StateHash,
    version: &[u8],
    manifest_hash: &StateHash,
    code_hash: &StateHash,
    hook_manifest_hash: &StateHash,
    resource_root: &StateHash,
    developer_cert_hash: &StateHash,
) -> StateHash {
    tuple_hash(
        DOMAIN_PACKAGE_ROOT,
        &[
            app_id,
            version,
            manifest_hash,
            code_hash,
            hook_manifest_hash,
            resource_root,
            developer_cert_hash,
        ],
    )
}

#[cfg(feature = "phasesentinel")]
pub fn ph_sentinel_root(
    equation_root: &StateHash,
    kernel_root: &StateHash,
    policy_root: &StateHash,
    k_authority: &StateHash,
    generation: u64,
) -> StateHash {
    tuple_hash(
        DOMAIN_SENTINEL_ROOT,
        &[
            equation_root,
            kernel_root,
            policy_root,
            k_authority,
            &generation.to_le_bytes(),
        ],
    )
}

#[cfg(feature = "phasesentinel")]
pub fn ph_sentinel_manifest(canonical: &[u8]) -> StateHash {
    tuple_hash(DOMAIN_SENTINEL_MANIFEST, &[canonical])
}

#[cfg(feature = "phasesentinel")]
pub fn ph_sentinel_policy(canonical: &[u8]) -> StateHash {
    tuple_hash(DOMAIN_SENTINEL_POLICY, &[canonical])
}

#[cfg(feature = "phasesentinel")]
pub fn ph_sentinel_authority(root_pubkey: &[u8; 32]) -> StateHash {
    tuple_hash(DOMAIN_SENTINEL_AUTHORITY, &[root_pubkey])
}

#[cfg(feature = "phasesentinel")]
pub fn ph_sentinel_checkpoint(canonical: &[u8]) -> StateHash {
    tuple_hash(DOMAIN_SENTINEL_CHECKPOINT, &[canonical])
}

#[cfg(feature = "phasesentinel")]
pub fn ph_sentinel_evidence(canonical: &[u8]) -> StateHash {
    tuple_hash(DOMAIN_SENTINEL_EVIDENCE, &[canonical])
}

#[cfg(all(feature = "phasesentinel", feature = "phasestack"))]
pub fn ph_sentinel_launch_receipt(parts: &[&[u8]]) -> StateHash {
    tuple_hash(DOMAIN_SENTINEL_LAUNCH, parts)
}

#[cfg(feature = "phasestack")]
pub fn ph_app_registry(
    installed_package_root: &StateHash,
    granted_capability_root: &StateHash,
    hook_registry_root: &StateHash,
    package_revocation_root: &StateHash,
    app_ledger_head: &StateHash,
) -> StateHash {
    tuple_hash(
        DOMAIN_APP_REGISTRY,
        &[
            installed_package_root,
            granted_capability_root,
            hook_registry_root,
            package_revocation_root,
            app_ledger_head,
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Manually re-builds the exact `tuple_hash` pre-image byte-for-byte and
    /// hashes it with a bare `Sha256`, independent of `tuple_hash` itself.
    /// This proves the *construction* (magic + length-prefix scheme), not
    /// just that `tuple_hash` is internally self-consistent.
    fn reference_tuple_hash(domain: &str, parts: &[&[u8]]) -> StateHash {
        let mut preimage = Vec::new();
        preimage.extend_from_slice(MAGIC);
        preimage.extend_from_slice(&(domain.len() as u32).to_le_bytes());
        preimage.extend_from_slice(domain.as_bytes());
        preimage.extend_from_slice(&(parts.len() as u32).to_le_bytes());
        for part in parts {
            preimage.extend_from_slice(&(part.len() as u32).to_le_bytes());
            preimage.extend_from_slice(part);
        }
        Sha256::digest(&preimage).into()
    }

    #[test]
    fn tuple_hash_matches_manually_built_preimage() {
        let cases: &[(&str, &[&[u8]])] = &[
            ("", &[]),
            ("GPOS/PhaseHash/content/v1", &[b"hello"]),
            (
                "GPOS/PhaseHash/state/v1",
                &[b"prev", b"chronos", b"sector", b"meta"],
            ),
            ("empty-part", &[b""]),
        ];
        for (domain, parts) in cases {
            assert_eq!(
                tuple_hash(domain, parts),
                reference_tuple_hash(domain, parts),
                "construction mismatch for domain {domain:?}"
            );
        }
    }

    #[test]
    fn tuple_hash_domain_separates_identical_parts() {
        let parts: &[&[u8]] = &[b"same-bytes"];
        assert_ne!(tuple_hash("domain-a", parts), tuple_hash("domain-b", parts));
    }

    #[test]
    fn tuple_hash_is_sensitive_to_part_boundaries() {
        // ("ab", "c") and ("a", "bc") must NOT collide even though the
        // concatenated bytes are identical — the length prefixes must bind
        // the split, not just the flattened content.
        let a: &[&[u8]] = &[b"ab", b"c"];
        let b: &[&[u8]] = &[b"a", b"bc"];
        assert_ne!(tuple_hash("d", a), tuple_hash("d", b));
    }

    #[test]
    fn tuple_hash_is_sensitive_to_part_count() {
        let flat: &[&[u8]] = &[b"abc"];
        let split: &[&[u8]] = &[b"ab", b"c"];
        // Same domain, same flattened bytes, different part count.
        assert_ne!(tuple_hash("d", flat), tuple_hash("d", split));
    }

    #[test]
    fn ph_content_is_deterministic_and_input_sensitive() {
        assert_eq!(ph_content(b"same"), ph_content(b"same"));
        assert_ne!(ph_content(b"same"), ph_content(b"diff"));
    }

    #[test]
    fn ph_orbit_is_invariant_under_reflection() {
        for state in PhaseState::ALL {
            let mirror = gphase_core::reflect(state);
            assert_eq!(ph_orbit(state), ph_orbit(mirror));
        }
    }

    #[test]
    fn ph_orbit_distinguishes_the_two_orbits() {
        // {Phi0, Phi3} and {Phi1, Phi2} are the only two orbits under
        // rho(x) = 3 - x; they must hash to different roots.
        assert_ne!(ph_orbit(PhaseState::Phi0), ph_orbit(PhaseState::Phi1));
    }

    #[test]
    fn ph_state_is_sensitive_to_every_field() {
        let base = StateInput {
            previous: [1u8; 32],
            chronos: 10,
            kairos_sector: 2,
            metadata_hash: [2u8; 32],
        };
        let base_hash = ph_state(&base);

        let with = |f: fn(StateInput) -> StateInput| ph_state(&f(base));
        assert_ne!(
            base_hash,
            with(|mut s| {
                s.previous = [9u8; 32];
                s
            })
        );
        assert_ne!(
            base_hash,
            with(|mut s| {
                s.chronos += 1;
                s
            })
        );
        assert_ne!(
            base_hash,
            with(|mut s| {
                s.kairos_sector += 1;
                s
            })
        );
        assert_ne!(
            base_hash,
            with(|mut s| {
                s.metadata_hash = [9u8; 32];
                s
            })
        );
    }

    #[test]
    fn ph_root_is_sensitive_to_leaf_order() {
        let leaves_ab = [[1u8; 32], [2u8; 32]];
        let leaves_ba = [[2u8; 32], [1u8; 32]];
        assert_ne!(ph_root(&leaves_ab), ph_root(&leaves_ba));
    }

    #[test]
    fn ph_boot_image_root_is_sensitive_to_each_component() {
        let fsbl = [1u8; 32];
        let bitstream = [2u8; 32];
        let kernel = [3u8; 32];
        let base = ph_boot_image_root(&fsbl, &bitstream, &kernel);
        assert_ne!(base, ph_boot_image_root(&[9u8; 32], &bitstream, &kernel));
        assert_ne!(base, ph_boot_image_root(&fsbl, &[9u8; 32], &kernel));
        assert_ne!(base, ph_boot_image_root(&fsbl, &bitstream, &[9u8; 32]));
    }

    #[test]
    fn ph_boot_session_root_is_sensitive_to_every_field() {
        let base = BootSessionRootInput {
            equation_root: [1u8; 32],
            init_pid: 7,
            kernel_root: [2u8; 32],
            boot_tick: 42,
        };
        let base_hash = ph_boot_session_root(&base);

        let with =
            |f: fn(BootSessionRootInput) -> BootSessionRootInput| ph_boot_session_root(&f(base));
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.equation_root = [9u8; 32];
                i
            })
        );
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.init_pid += 1;
                i
            })
        );
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.kernel_root = [9u8; 32];
                i
            })
        );
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.boot_tick += 1;
                i
            })
        );
    }

    #[test]
    fn ph_authority_root_is_sensitive_to_every_field() {
        let base = AuthorityRootInput {
            equation_root: [1u8; 32],
            installation_id: [2u8; 32],
            root_pubkey_hash: [3u8; 32],
            policy_hash: [4u8; 32],
        };
        let base_hash = ph_authority_root(&base);

        let with = |f: fn(AuthorityRootInput) -> AuthorityRootInput| ph_authority_root(&f(base));
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.equation_root = [9u8; 32];
                i
            })
        );
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.installation_id = [9u8; 32];
                i
            })
        );
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.root_pubkey_hash = [9u8; 32];
                i
            })
        );
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.policy_hash = [9u8; 32];
                i
            })
        );
    }

    #[test]
    fn ph_authority_root_is_deterministic() {
        let input = AuthorityRootInput {
            equation_root: [5u8; 32],
            installation_id: [6u8; 32],
            root_pubkey_hash: [7u8; 32],
            policy_hash: [8u8; 32],
        };
        assert_eq!(ph_authority_root(&input), ph_authority_root(&input));
    }

    #[test]
    fn ph_authority_root_domain_separates_from_boot_session_root() {
        // Same-shaped 32-byte inputs must not collide across the two
        // dedicated boot-chain/authority-chain domains.
        let shared = [1u8; 32];
        let boot = ph_boot_session_root(&BootSessionRootInput {
            equation_root: shared,
            init_pid: 0,
            kernel_root: shared,
            boot_tick: 0,
        });
        let authority = ph_authority_root(&AuthorityRootInput {
            equation_root: shared,
            installation_id: shared,
            root_pubkey_hash: shared,
            policy_hash: [0u8; 32],
        });
        assert_ne!(boot, authority);
    }

    #[test]
    fn ph_cert_tbs_is_deterministic_and_input_sensitive() {
        assert_eq!(ph_cert_tbs(b"tbs-bytes"), ph_cert_tbs(b"tbs-bytes"));
        assert_ne!(ph_cert_tbs(b"tbs-bytes-a"), ph_cert_tbs(b"tbs-bytes-b"));
    }

    #[test]
    fn ph_cert_binds_tbs_and_signature_together() {
        let sig_a = [1u8; 64];
        let sig_b = [2u8; 64];
        let base = ph_cert(b"tbs", &sig_a);
        assert_eq!(base, ph_cert(b"tbs", &sig_a));
        assert_ne!(base, ph_cert(b"tbs", &sig_b));
        assert_ne!(base, ph_cert(b"other-tbs", &sig_a));
    }

    #[test]
    fn ph_cert_tbs_and_ph_cert_are_domain_separated_from_each_other() {
        let sig = [0u8; 64];
        assert_ne!(ph_cert_tbs(b"x"), ph_cert(b"x", &sig));
    }

    #[test]
    fn ph_boot_session_root_is_deterministic() {
        let input = BootSessionRootInput {
            equation_root: [5u8; 32],
            init_pid: 1,
            kernel_root: [6u8; 32],
            boot_tick: 3,
        };
        assert_eq!(ph_boot_session_root(&input), ph_boot_session_root(&input));
    }

    fn base_chain_input() -> IdentityChainInput {
        IdentityChainInput {
            previous: [1u8; 32],
            sequence: 7,
            kind: 3,
            principal_id: 42,
            tick: 100,
            payload_hash: [2u8; 32],
        }
    }

    #[test]
    fn ph_identity_chain_is_deterministic() {
        let input = base_chain_input();
        assert_eq!(ph_identity_chain(&input), ph_identity_chain(&input));
    }

    #[test]
    fn ph_identity_chain_is_sensitive_to_every_field() {
        let base = base_chain_input();
        let base_hash = ph_identity_chain(&base);
        let with = |f: fn(IdentityChainInput) -> IdentityChainInput| ph_identity_chain(&f(base));
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.previous = [9u8; 32];
                i
            })
        );
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.sequence += 1;
                i
            })
        );
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.kind += 1;
                i
            })
        );
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.principal_id += 1;
                i
            })
        );
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.tick += 1;
                i
            })
        );
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.payload_hash = [9u8; 32];
                i
            })
        );
    }

    fn base_authority_state_input() -> AuthorityStateRootInput {
        AuthorityStateRootInput {
            authority_root: [1u8; 32],
            chain_head: [2u8; 32],
            principal_root: [3u8; 32],
            certificate_root: [4u8; 32],
            credential_root: [5u8; 32],
            revocation_root: [6u8; 32],
            policy_root: [7u8; 32],
        }
    }

    #[test]
    fn ph_authority_state_root_is_deterministic() {
        let input = base_authority_state_input();
        assert_eq!(
            ph_authority_state_root(&input),
            ph_authority_state_root(&input)
        );
    }

    #[test]
    fn ph_authority_state_root_is_sensitive_to_every_field() {
        let base = base_authority_state_input();
        let base_hash = ph_authority_state_root(&base);
        let with = |f: fn(AuthorityStateRootInput) -> AuthorityStateRootInput| {
            ph_authority_state_root(&f(base))
        };
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.authority_root = [9u8; 32];
                i
            })
        );
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.chain_head = [9u8; 32];
                i
            })
        );
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.principal_root = [9u8; 32];
                i
            })
        );
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.certificate_root = [9u8; 32];
                i
            })
        );
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.credential_root = [9u8; 32];
                i
            })
        );
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.revocation_root = [9u8; 32];
                i
            })
        );
        assert_ne!(
            base_hash,
            with(|mut i| {
                i.policy_root = [9u8; 32];
                i
            })
        );
    }

    #[test]
    fn ph_authority_state_root_domain_separates_from_authority_root() {
        // Same-shaped 32-byte inputs must not collide across the two
        // dedicated authority-chain domains.
        let shared = [1u8; 32];
        let authority = ph_authority_root(&AuthorityRootInput {
            equation_root: shared,
            installation_id: shared,
            root_pubkey_hash: shared,
            policy_hash: shared,
        });
        let state = ph_authority_state_root(&AuthorityStateRootInput {
            authority_root: shared,
            chain_head: shared,
            principal_root: shared,
            certificate_root: shared,
            credential_root: shared,
            revocation_root: shared,
            policy_root: shared,
        });
        assert_ne!(authority, state);
    }
}
