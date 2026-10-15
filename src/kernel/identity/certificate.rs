//! User certificates (`kernel::identity`, Gate 2) — `UserCertificateV1`
//! canonical encoding, Ed25519 signature verification, and bounded
//! chain-of-trust verification terminating at a pinned Root certificate.
//!
//! ## What this reuses
//! - [`crate::kernel::crypto::sign::verify`] — real Ed25519 verification
//!   for identity certificates. That is not V1/V2
//!   [`crate::kernel::statechain_checkpoint`] (public-derivable HMAC,
//!   integrity-only) and not [`crate::kernel::checkpoint_auth`] (secret-key
//!   HMAC-SHA256). This module never implements its own signature math
//!   and never treats an HMAC tag as an Ed25519 signature.
//! - [`crate::kernel::crypto::phase_hash::ph_cert_tbs`] / `ph_cert` — new
//!   domains added alongside this file, same `tuple_hash` construction as
//!   every other PhaseHash variant.
//! - [`crate::kernel::identity::principal::PrincipalId`] /
//!   [`crate::kernel::identity::principal::PrincipalKind`] — Gate 1's
//!   types; a certificate's `subject`/`issuer` are principals, not a new
//!   parallel identity concept.
//!
//! ## Mathematics implemented (platform spec §4.1)
//! - `Signature_u = Sign_SK_issuer(PH_cert-tbs(TBS_u))` — see
//!   [`sign_certificate`].
//! - `C_u = PH_cert(TBS_u, Signature_u)` — see [`certificate_hash`].
//! - Chain verification walks `issuer_certificate_hash` links up to
//!   [`MAX_CHAIN_DEPTH`], with cycle detection, checking version, revocation,
//!   authority-generation validity, issuer-permission (`CERT_FLAG_CAN_ISSUE`),
//!   and the real Ed25519 signature at every hop — see
//!   [`CertificateRegistry::verify_chain`].
//!
//! ## Root pinning (no circular hash dependency)
//! The Root certificate is **self-issued by sentinel, not by embedding its
//! own hash**: `issuer_certificate_hash == [0u8; 32]` structurally marks "no
//! issuer" (a real hash can never legitimately equal the all-zero
//! sentinel — [`ph_cert_tbs`]/[`ph_cert`] domain-separate every input, so a
//! genuine collision with `[0u8; 32]` is exactly as infeasible as any other
//! SHA-256 preimage). Embedding a certificate's *own* hash inside its own
//! TBS bytes would be a genuine fixed point no hash function can satisfy
//! constructively; the sentinel avoids that problem rather than faking a
//! solution to it. [`CertificateRegistry::install_root_certificate`]
//! additionally requires the caller supply the externally pinned Root
//! public key and checks the certificate's own claimed key against it — the
//! pin comes from outside the certificate, never from trusting whatever the
//! certificate itself claims.
//!
//! ## Non-goals (Gate 2)
//! - No password credentials (Gate 3), no persistence (Gate 6): this
//!   registry is RAM-only, like every prior subsystem before its own
//!   persistence gate.
//! - Revocation here is a simple bounded set of revoked certificate hashes.
//!   The real Merkle `RevocationRoot`/`AuthorityStateRoot` commitment is
//!   Gate 5 — this gate proves the *check* is real (revoked certs are
//!   really rejected), not yet the ledger commitment around it.
//! - A certificate proves an authorized issuer associated a public key with
//!   a principal. It does not prove government/legal identity — the
//!   platform spec's own explicit caveat, repeated here per Gate 0's
//!   "declare non-claims" convention.

use crate::kernel::crypto::phase_hash::{ph_cert, ph_cert_tbs};
use crate::kernel::crypto::sign;
use crate::kernel::hash::StateHash;
use crate::kernel::identity::principal::{PrincipalId, PrincipalKind};
use crate::platform::{HashMap, HashSet, Vec};

/// The only certificate encoding version this gate understands. Any other
/// value is rejected by [`CertificateRegistry::issue`]/`verify_chain`
/// ("unknown versions rejected", platform spec §4.1).
pub const CERTIFICATE_VERSION_V1: u16 = 1;

/// Maximum certificate-chain depth (platform spec §4.1: "Maximum
/// certificate-chain depth of eight").
pub const MAX_CHAIN_DEPTH: usize = 8;

/// Hard bound on tracked certificates ("keep all tables ... bounded").
pub const MAX_CERTIFICATES: usize = 512;

/// Sentinel `issuer_certificate_hash` marking a self-issued (Root) certificate.
pub const NO_ISSUER: StateHash = [0u8; 32];

pub type CertificateSerial = u64;

/// Bitmask over [`crate::kernel::identity::principal::AuthorityScope`].
pub type AuthorityScopeMask = u8;
pub const SCOPE_OBJECT: AuthorityScopeMask = 1 << 0;
pub const SCOPE_HOST: AuthorityScopeMask = 1 << 1;
pub const SCOPE_CLUSTER: AuthorityScopeMask = 1 << 2;

pub fn scope_bit(scope: crate::kernel::identity::principal::AuthorityScope) -> AuthorityScopeMask {
    use crate::kernel::identity::principal::AuthorityScope;
    match scope {
        AuthorityScope::Object => SCOPE_OBJECT,
        AuthorityScope::Host => SCOPE_HOST,
        AuthorityScope::Cluster => SCOPE_CLUSTER,
    }
}

/// Whether `mask` grants `scope`.
pub fn scope_allowed(
    mask: AuthorityScopeMask,
    scope: crate::kernel::identity::principal::AuthorityScope,
) -> bool {
    mask & scope_bit(scope) != 0
}

/// Certificate flag bits.
pub type CertificateFlags = u16;
/// Set only on certificates permitted to issue further certificates
/// ("Issuer permission checking", platform spec §4.1).
pub const CERT_FLAG_CAN_ISSUE: CertificateFlags = 1 << 0;
/// Set only on the pinned Root certificate itself; required (alongside the
/// [`NO_ISSUER`] sentinel and `subject == issuer`) by
/// [`CertificateRegistry::install_root_certificate`].
pub const CERT_FLAG_ROOT: CertificateFlags = 1 << 1;

/// A canonically-encoded, Ed25519-signed user certificate. See module docs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserCertificateV1 {
    pub version: u16,
    pub serial: CertificateSerial,
    pub subject: PrincipalId,
    pub subject_kind: PrincipalKind,
    pub subject_public_key: [u8; 32],
    pub issuer: PrincipalId,
    pub issuer_certificate_hash: StateHash,
    pub policy_root: StateHash,
    pub authority_generation_start: u64,
    pub authority_generation_end: Option<u64>,
    pub allowed_scope: AuthorityScopeMask,
    pub flags: CertificateFlags,
    pub signature: [u8; 64],
}

/// Canonical, fixed-layout to-be-signed encoding of every field except
/// `signature` itself. Explicit field widths (no ambiguity, no
/// variable-length field without an explicit tag) — platform spec §4.1's
/// "Canonical binary encoding. Explicit version and field lengths."
pub fn encode_tbs(cert: &UserCertificateV1) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + 8 + 4 + 1 + 32 + 4 + 32 + 32 + 8 + 9 + 1 + 2);
    out.extend_from_slice(&cert.version.to_le_bytes());
    out.extend_from_slice(&cert.serial.to_le_bytes());
    out.extend_from_slice(&cert.subject.to_le_bytes());
    out.push(cert.subject_kind.as_u8());
    out.extend_from_slice(&cert.subject_public_key);
    out.extend_from_slice(&cert.issuer.to_le_bytes());
    out.extend_from_slice(&cert.issuer_certificate_hash);
    out.extend_from_slice(&cert.policy_root);
    out.extend_from_slice(&cert.authority_generation_start.to_le_bytes());
    match cert.authority_generation_end {
        Some(end) => {
            out.push(1);
            out.extend_from_slice(&end.to_le_bytes());
        }
        None => {
            out.push(0);
            out.extend_from_slice(&0u64.to_le_bytes());
        }
    }
    out.push(cert.allowed_scope);
    out.extend_from_slice(&cert.flags.to_le_bytes());
    out
}

/// Full on-disk encoding: canonical TBS bytes followed by the 64-byte
/// Ed25519 signature. Used by Gate 6 persistence.
pub fn encode_certificate(cert: &UserCertificateV1) -> Vec<u8> {
    let mut out = encode_tbs(cert);
    out.extend_from_slice(&cert.signature);
    out
}

/// Inverse of [`encode_certificate`]. Rejects truncated or over-long
/// encodings rather than guessing missing fields.
pub fn decode_certificate(bytes: &[u8]) -> Result<UserCertificateV1, CertificateError> {
    // TBS layout is fixed-width (see encode_tbs): 135 bytes + 64-byte signature.
    const TBS_LEN: usize = 2 + 8 + 4 + 1 + 32 + 4 + 32 + 32 + 8 + 1 + 8 + 1 + 2;
    if bytes.len() != TBS_LEN + 64 {
        return Err(CertificateError::UnknownVersion);
    }
    let mut off = 0usize;
    let take = |off: &mut usize, n: usize| -> Result<&[u8], CertificateError> {
        let end = off.checked_add(n).ok_or(CertificateError::UnknownVersion)?;
        if end > TBS_LEN {
            return Err(CertificateError::UnknownVersion);
        }
        let slice = &bytes[*off..end];
        *off = end;
        Ok(slice)
    };
    let version = u16::from_le_bytes(take(&mut off, 2)?.try_into().unwrap());
    let serial = u64::from_le_bytes(take(&mut off, 8)?.try_into().unwrap());
    let subject = u32::from_le_bytes(take(&mut off, 4)?.try_into().unwrap());
    let kind_byte = take(&mut off, 1)?[0];
    let subject_kind = PrincipalKind::from_u8(kind_byte).ok_or(CertificateError::UnknownVersion)?;
    let mut subject_public_key = [0u8; 32];
    subject_public_key.copy_from_slice(take(&mut off, 32)?);
    let issuer = u32::from_le_bytes(take(&mut off, 4)?.try_into().unwrap());
    let mut issuer_certificate_hash = [0u8; 32];
    issuer_certificate_hash.copy_from_slice(take(&mut off, 32)?);
    let mut policy_root = [0u8; 32];
    policy_root.copy_from_slice(take(&mut off, 32)?);
    let authority_generation_start = u64::from_le_bytes(take(&mut off, 8)?.try_into().unwrap());
    let end_flag = take(&mut off, 1)?[0];
    let end_val = u64::from_le_bytes(take(&mut off, 8)?.try_into().unwrap());
    let authority_generation_end = match end_flag {
        0 => None,
        1 => Some(end_val),
        _ => return Err(CertificateError::UnknownVersion),
    };
    let allowed_scope = take(&mut off, 1)?[0];
    let flags = u16::from_le_bytes(take(&mut off, 2)?.try_into().unwrap());
    let mut signature = [0u8; 64];
    signature.copy_from_slice(&bytes[TBS_LEN..]);
    Ok(UserCertificateV1 {
        version,
        serial,
        subject,
        subject_kind,
        subject_public_key,
        issuer,
        issuer_certificate_hash,
        policy_root,
        authority_generation_start,
        authority_generation_end,
        allowed_scope,
        flags,
        signature,
    })
}

/// `Signature_u = Sign_SK_issuer(PH_cert-tbs(TBS_u))`.
pub fn sign_certificate(cert: &UserCertificateV1, issuer_key: &sign::SignKeypair) -> [u8; 64] {
    let tbs = encode_tbs(cert);
    let tbs_hash = ph_cert_tbs(&tbs);
    issuer_key.sign(&tbs_hash)
}

/// Verify `cert.signature` against `issuer_public_key`, over
/// `PH_cert-tbs(TBS_u)` exactly as [`sign_certificate`] produced it. Never
/// panics; returns `false` on any mismatch.
pub fn verify_certificate_signature(
    cert: &UserCertificateV1,
    issuer_public_key: &[u8; 32],
) -> bool {
    let tbs = encode_tbs(cert);
    let tbs_hash = ph_cert_tbs(&tbs);
    sign::verify(issuer_public_key, &tbs_hash, &cert.signature)
}

/// `C_u = PH_cert(TBS_u, Signature_u)`.
pub fn certificate_hash(cert: &UserCertificateV1) -> StateHash {
    ph_cert(&encode_tbs(cert), &cert.signature)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertificateError {
    UnknownVersion,
    DuplicateSerial,
    TableFull,
    UnknownIssuer,
    ChainTooDeep,
    ChainCycle,
    Revoked,
    AuthorityGenerationInvalid,
    IssuerNotPermittedToIssue,
    SignatureInvalid,
    NotFound,
    /// [`CertificateRegistry::install_root_certificate`] was given a
    /// certificate that is not structurally self-issued (see [`NO_ISSUER`],
    /// `CERT_FLAG_ROOT`, `subject == issuer`), or whose claimed public key
    /// does not match the externally pinned Root public key.
    NotSelfSigned,
    /// [`CertificateRegistry::install_root_certificate`] was called on a
    /// registry that already has a pinned Root certificate.
    AlreadyHasRoot,
}

/// Bounded certificate trust store: issued certificates keyed by their own
/// [`certificate_hash`], a used-serial set (duplicate-serial rejection), a
/// revoked-hash set, and (once installed) the one pinned Root certificate's
/// hash chain verification terminates at.
#[derive(Debug, Clone)]
pub struct CertificateRegistry {
    certs_by_hash: HashMap<StateHash, UserCertificateV1>,
    serials_used: HashSet<CertificateSerial>,
    revoked: HashSet<StateHash>,
    root_certificate_hash: Option<StateHash>,
}

impl Default for CertificateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CertificateRegistry {
    pub fn new() -> Self {
        Self {
            certs_by_hash: HashMap::new(),
            serials_used: HashSet::new(),
            revoked: HashSet::new(),
            root_certificate_hash: None,
        }
    }

    pub fn root_certificate_hash(&self) -> Option<StateHash> {
        self.root_certificate_hash
    }

    pub fn get(&self, hash: StateHash) -> Option<&UserCertificateV1> {
        self.certs_by_hash.get(&hash)
    }

    pub fn is_revoked(&self, hash: StateHash) -> bool {
        self.revoked.contains(&hash)
    }

    fn verify_single(
        &self,
        cert: &UserCertificateV1,
        hash: StateHash,
        now_generation: u64,
    ) -> Result<(), CertificateError> {
        if cert.version != CERTIFICATE_VERSION_V1 {
            return Err(CertificateError::UnknownVersion);
        }
        if self.revoked.contains(&hash) {
            return Err(CertificateError::Revoked);
        }
        if now_generation < cert.authority_generation_start {
            return Err(CertificateError::AuthorityGenerationInvalid);
        }
        if let Some(end) = cert.authority_generation_end {
            if now_generation > end {
                return Err(CertificateError::AuthorityGenerationInvalid);
            }
        }
        Ok(())
    }

    /// Install the one pinned Root certificate this registry's chain
    /// verification terminates at. Callable exactly once. See module docs,
    /// "Root pinning", for why this is sentinel-based rather than
    /// hash-embedding, and why `root_public_key` is an externally supplied
    /// pin the certificate is checked *against*, not trusted from.
    pub fn install_root_certificate(
        &mut self,
        cert: UserCertificateV1,
        root_public_key: &[u8; 32],
    ) -> Result<StateHash, CertificateError> {
        if self.root_certificate_hash.is_some() {
            return Err(CertificateError::AlreadyHasRoot);
        }
        if cert.version != CERTIFICATE_VERSION_V1 {
            return Err(CertificateError::UnknownVersion);
        }
        if cert.flags & CERT_FLAG_ROOT == 0
            || cert.issuer_certificate_hash != NO_ISSUER
            || cert.subject != cert.issuer
            || cert.subject_kind != PrincipalKind::Root
        {
            return Err(CertificateError::NotSelfSigned);
        }
        if cert.subject_public_key != *root_public_key {
            return Err(CertificateError::NotSelfSigned);
        }
        if !verify_certificate_signature(&cert, root_public_key) {
            return Err(CertificateError::SignatureInvalid);
        }
        if self.serials_used.contains(&cert.serial) {
            return Err(CertificateError::DuplicateSerial);
        }
        let hash = certificate_hash(&cert);
        self.serials_used.insert(cert.serial);
        self.certs_by_hash.insert(hash, cert);
        self.root_certificate_hash = Some(hash);
        Ok(hash)
    }

    /// Issue (register) a new non-root certificate. Its immediate issuer
    /// must already be a registered, valid, `CERT_FLAG_CAN_ISSUE`
    /// certificate, and `cert.signature` must really verify against that
    /// issuer's public key — the same single-hop check every real CA
    /// performs at issuance time. (Full-path re-verification of an
    /// already-registered chain is [`Self::verify_chain`]'s job, used for
    /// certificates presented at authentication time that may not have come
    /// through this method.)
    pub fn issue(
        &mut self,
        cert: UserCertificateV1,
        now_generation: u64,
    ) -> Result<StateHash, CertificateError> {
        if cert.version != CERTIFICATE_VERSION_V1 {
            return Err(CertificateError::UnknownVersion);
        }
        if self.serials_used.contains(&cert.serial) {
            return Err(CertificateError::DuplicateSerial);
        }
        if self.certs_by_hash.len() >= MAX_CERTIFICATES {
            return Err(CertificateError::TableFull);
        }
        let issuer_cert = self
            .certs_by_hash
            .get(&cert.issuer_certificate_hash)
            .cloned()
            .ok_or(CertificateError::UnknownIssuer)?;
        self.verify_single(&issuer_cert, cert.issuer_certificate_hash, now_generation)?;
        if issuer_cert.flags & CERT_FLAG_CAN_ISSUE == 0 {
            return Err(CertificateError::IssuerNotPermittedToIssue);
        }
        if !verify_certificate_signature(&cert, &issuer_cert.subject_public_key) {
            return Err(CertificateError::SignatureInvalid);
        }
        let hash = certificate_hash(&cert);
        self.serials_used.insert(cert.serial);
        self.certs_by_hash.insert(hash, cert);
        Ok(hash)
    }

    /// Revoke a registered certificate by its own hash.
    pub fn revoke(&mut self, hash: StateHash) -> Result<(), CertificateError> {
        if !self.certs_by_hash.contains_key(&hash) {
            return Err(CertificateError::NotFound);
        }
        self.revoked.insert(hash);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.certs_by_hash.len()
    }

    /// Snapshot of every stored certificate, ordered by hash bytes. Used by
    /// Gate 6 persistence.
    pub fn snapshot(&self) -> crate::platform::Vec<(StateHash, UserCertificateV1)> {
        let mut out: crate::platform::Vec<(StateHash, UserCertificateV1)> = self
            .certs_by_hash
            .iter()
            .map(|(h, c)| (*h, c.clone()))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    pub fn revoked_snapshot(&self) -> crate::platform::Vec<StateHash> {
        let mut out: crate::platform::Vec<StateHash> = self.revoked.iter().copied().collect();
        out.sort();
        out
    }

    /// Replace the registry with a previously persisted snapshot. Does not
    /// re-run issuance checks — this is restore, not a mutation (Gate 6).
    pub fn restore(
        &mut self,
        certs: crate::platform::Vec<UserCertificateV1>,
        revoked: crate::platform::Vec<StateHash>,
        root_certificate_hash: Option<StateHash>,
    ) -> Result<(), CertificateError> {
        if certs.len() > MAX_CERTIFICATES {
            return Err(CertificateError::TableFull);
        }
        let mut certs_by_hash = crate::platform::HashMap::new();
        let mut serials_used = crate::platform::HashSet::new();
        for cert in certs {
            if cert.version != CERTIFICATE_VERSION_V1 {
                return Err(CertificateError::UnknownVersion);
            }
            if serials_used.contains(&cert.serial) {
                return Err(CertificateError::DuplicateSerial);
            }
            let hash = certificate_hash(&cert);
            serials_used.insert(cert.serial);
            certs_by_hash.insert(hash, cert);
        }
        if let Some(root) = root_certificate_hash {
            if !certs_by_hash.contains_key(&root) {
                return Err(CertificateError::NotFound);
            }
        }
        let mut revoked_set = crate::platform::HashSet::new();
        for hash in revoked {
            if !certs_by_hash.contains_key(&hash) {
                return Err(CertificateError::NotFound);
            }
            revoked_set.insert(hash);
        }
        self.certs_by_hash = certs_by_hash;
        self.serials_used = serials_used;
        self.revoked = revoked_set;
        self.root_certificate_hash = root_certificate_hash;
        Ok(())
    }

    /// Full chain-of-trust verification for `cert` (which need not itself
    /// be registered — e.g. a certificate presented over the wire at
    /// authentication time). Walks `issuer_certificate_hash` links,
    /// re-verifying every real Ed25519 signature along the way, until it
    /// reaches this registry's pinned Root certificate (by hash equality —
    /// see module docs). Returns the leaf's own hash on success. Bounded by
    /// [`MAX_CHAIN_DEPTH`]; detects cycles explicitly rather than looping.
    pub fn verify_chain(
        &self,
        cert: &UserCertificateV1,
        now_generation: u64,
    ) -> Result<StateHash, CertificateError> {
        let leaf_hash = certificate_hash(cert);
        self.verify_single(cert, leaf_hash, now_generation)?;

        let mut visited: Vec<StateHash> = Vec::with_capacity(MAX_CHAIN_DEPTH + 1);
        let mut current = cert.clone();
        let mut current_hash = leaf_hash;

        for depth in 0..=MAX_CHAIN_DEPTH {
            if visited.contains(&current_hash) {
                return Err(CertificateError::ChainCycle);
            }
            visited.push(current_hash);

            if self.root_certificate_hash == Some(current_hash) {
                return Ok(leaf_hash);
            }
            if depth == MAX_CHAIN_DEPTH {
                return Err(CertificateError::ChainTooDeep);
            }

            let issuer_hash = current.issuer_certificate_hash;
            let issuer_cert = self
                .certs_by_hash
                .get(&issuer_hash)
                .ok_or(CertificateError::UnknownIssuer)?;
            self.verify_single(issuer_cert, issuer_hash, now_generation)?;
            if issuer_cert.flags & CERT_FLAG_CAN_ISSUE == 0 {
                return Err(CertificateError::IssuerNotPermittedToIssue);
            }
            if !verify_certificate_signature(&current, &issuer_cert.subject_public_key) {
                return Err(CertificateError::SignatureInvalid);
            }

            current = issuer_cert.clone();
            current_hash = issuer_hash;
        }
        // Unreachable: the loop above always returns by depth == MAX_CHAIN_DEPTH.
        Err(CertificateError::ChainTooDeep)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::identity::principal::AuthorityScope;

    fn keypair(seed_byte: u8) -> sign::SignKeypair {
        sign::SignKeypair::from_seed(&[seed_byte; 32])
    }

    fn root_cert(root_key: &sign::SignKeypair) -> UserCertificateV1 {
        let mut cert = UserCertificateV1 {
            version: CERTIFICATE_VERSION_V1,
            serial: 1,
            subject: 0,
            subject_kind: PrincipalKind::Root,
            subject_public_key: root_key.public_bytes(),
            issuer: 0,
            issuer_certificate_hash: NO_ISSUER,
            policy_root: [0u8; 32],
            authority_generation_start: 0,
            authority_generation_end: None,
            allowed_scope: SCOPE_HOST | SCOPE_CLUSTER,
            flags: CERT_FLAG_ROOT | CERT_FLAG_CAN_ISSUE,
            signature: [0u8; 64],
        };
        cert.signature = sign_certificate(&cert, root_key);
        cert
    }

    fn child_cert(
        serial: u64,
        subject: PrincipalId,
        subject_kind: PrincipalKind,
        subject_key: &sign::SignKeypair,
        issuer_key: &sign::SignKeypair,
        issuer_hash: StateHash,
        can_issue: bool,
        gen_start: u64,
        gen_end: Option<u64>,
    ) -> UserCertificateV1 {
        let mut cert = UserCertificateV1 {
            version: CERTIFICATE_VERSION_V1,
            serial,
            subject,
            subject_kind,
            subject_public_key: subject_key.public_bytes(),
            issuer: 0,
            issuer_certificate_hash: issuer_hash,
            policy_root: [0u8; 32],
            authority_generation_start: gen_start,
            authority_generation_end: gen_end,
            allowed_scope: SCOPE_OBJECT,
            flags: if can_issue { CERT_FLAG_CAN_ISSUE } else { 0 },
            signature: [0u8; 64],
        };
        cert.signature = sign_certificate(&cert, issuer_key);
        cert
    }

    #[test]
    fn encode_tbs_is_sensitive_to_every_field() {
        let root_key = keypair(1);
        let base = root_cert(&root_key);
        let base_tbs = encode_tbs(&base);

        let mut changed_serial = base.clone();
        changed_serial.serial += 1;
        assert_ne!(encode_tbs(&changed_serial), base_tbs);

        let mut changed_scope = base.clone();
        changed_scope.allowed_scope ^= SCOPE_OBJECT;
        assert_ne!(encode_tbs(&changed_scope), base_tbs);

        let mut changed_end = base.clone();
        changed_end.authority_generation_end = Some(5);
        assert_ne!(encode_tbs(&changed_end), base_tbs);
    }

    #[test]
    fn sign_and_verify_round_trip() {
        let root_key = keypair(1);
        let cert = root_cert(&root_key);
        assert!(verify_certificate_signature(
            &cert,
            &root_key.public_bytes()
        ));
    }

    #[test]
    fn tampered_field_invalidates_signature() {
        let root_key = keypair(1);
        let mut cert = root_cert(&root_key);
        cert.allowed_scope = SCOPE_OBJECT; // tamper after signing
        assert!(!verify_certificate_signature(
            &cert,
            &root_key.public_bytes()
        ));
    }

    #[test]
    fn wrong_key_fails_verification() {
        let root_key = keypair(1);
        let other_key = keypair(2);
        let cert = root_cert(&root_key);
        assert!(!verify_certificate_signature(
            &cert,
            &other_key.public_bytes()
        ));
    }

    #[test]
    fn install_root_certificate_succeeds_for_a_valid_self_signed_root() {
        let root_key = keypair(1);
        let mut registry = CertificateRegistry::new();
        let cert = root_cert(&root_key);
        let hash = registry
            .install_root_certificate(cert, &root_key.public_bytes())
            .unwrap();
        assert_eq!(registry.root_certificate_hash(), Some(hash));
    }

    #[test]
    fn install_root_certificate_rejects_mismatched_pinned_key() {
        let root_key = keypair(1);
        let wrong_pin = keypair(9);
        let mut registry = CertificateRegistry::new();
        let cert = root_cert(&root_key);
        assert_eq!(
            registry.install_root_certificate(cert, &wrong_pin.public_bytes()),
            Err(CertificateError::NotSelfSigned)
        );
    }

    #[test]
    fn install_root_certificate_rejects_a_second_call() {
        let root_key = keypair(1);
        let mut registry = CertificateRegistry::new();
        registry
            .install_root_certificate(root_cert(&root_key), &root_key.public_bytes())
            .unwrap();
        assert_eq!(
            registry.install_root_certificate(root_cert(&root_key), &root_key.public_bytes()),
            Err(CertificateError::AlreadyHasRoot)
        );
    }

    #[test]
    fn issue_and_verify_a_one_hop_chain_to_root() {
        let root_key = keypair(1);
        let mod_key = keypair(2);
        let mut registry = CertificateRegistry::new();
        let root_hash = registry
            .install_root_certificate(root_cert(&root_key), &root_key.public_bytes())
            .unwrap();
        let mod_cert = child_cert(
            2,
            10,
            PrincipalKind::Mod,
            &mod_key,
            &root_key,
            root_hash,
            true,
            0,
            None,
        );
        let mod_hash = registry.issue(mod_cert.clone(), 0).unwrap();
        assert_eq!(registry.verify_chain(&mod_cert, 0), Ok(mod_hash));
    }

    #[test]
    fn issue_and_verify_a_two_hop_chain_through_mod_to_operator() {
        let root_key = keypair(1);
        let mod_key = keypair(2);
        let op_key = keypair(3);
        let mut registry = CertificateRegistry::new();
        let root_hash = registry
            .install_root_certificate(root_cert(&root_key), &root_key.public_bytes())
            .unwrap();
        let mod_cert = child_cert(
            2,
            10,
            PrincipalKind::Mod,
            &mod_key,
            &root_key,
            root_hash,
            true,
            0,
            None,
        );
        let mod_hash = registry.issue(mod_cert, 0).unwrap();
        let op_cert = child_cert(
            3,
            20,
            PrincipalKind::Operator,
            &op_key,
            &mod_key,
            mod_hash,
            false,
            0,
            None,
        );
        let op_hash = registry.issue(op_cert.clone(), 0).unwrap();
        assert_eq!(registry.verify_chain(&op_cert, 0), Ok(op_hash));
    }

    #[test]
    fn unknown_version_is_rejected() {
        let root_key = keypair(1);
        let mut registry = CertificateRegistry::new();
        let root_hash = registry
            .install_root_certificate(root_cert(&root_key), &root_key.public_bytes())
            .unwrap();
        let mod_key = keypair(2);
        let mut bad = child_cert(
            2,
            10,
            PrincipalKind::Mod,
            &mod_key,
            &root_key,
            root_hash,
            true,
            0,
            None,
        );
        bad.version = 99;
        assert_eq!(
            registry.issue(bad, 0),
            Err(CertificateError::UnknownVersion)
        );
    }

    #[test]
    fn duplicate_serial_is_rejected() {
        let root_key = keypair(1);
        let mod_key = keypair(2);
        let mut registry = CertificateRegistry::new();
        let root_hash = registry
            .install_root_certificate(root_cert(&root_key), &root_key.public_bytes())
            .unwrap();
        let a = child_cert(
            2,
            10,
            PrincipalKind::Mod,
            &mod_key,
            &root_key,
            root_hash,
            true,
            0,
            None,
        );
        registry.issue(a, 0).unwrap();
        let b = child_cert(
            2, // same serial as `a`
            11,
            PrincipalKind::Operator,
            &keypair(3),
            &root_key,
            root_hash,
            false,
            0,
            None,
        );
        assert_eq!(registry.issue(b, 0), Err(CertificateError::DuplicateSerial));
    }

    #[test]
    fn unknown_issuer_is_rejected() {
        let mod_key = keypair(2);
        let mut registry = CertificateRegistry::new();
        let orphan = child_cert(
            5,
            10,
            PrincipalKind::Mod,
            &mod_key,
            &mod_key,
            [77u8; 32], // no such issuer registered
            true,
            0,
            None,
        );
        assert_eq!(
            registry.issue(orphan, 0),
            Err(CertificateError::UnknownIssuer)
        );
    }

    #[test]
    fn issuer_without_can_issue_flag_is_rejected() {
        let root_key = keypair(1);
        let mod_key = keypair(2);
        let op_key = keypair(3);
        let mut registry = CertificateRegistry::new();
        let root_hash = registry
            .install_root_certificate(root_cert(&root_key), &root_key.public_bytes())
            .unwrap();
        let mod_cert = child_cert(
            2,
            10,
            PrincipalKind::Mod,
            &mod_key,
            &root_key,
            root_hash,
            false, // Mod NOT permitted to issue further certs
            0,
            None,
        );
        let mod_hash = registry.issue(mod_cert, 0).unwrap();
        let op_cert = child_cert(
            3,
            20,
            PrincipalKind::Operator,
            &op_key,
            &mod_key,
            mod_hash,
            false,
            0,
            None,
        );
        assert_eq!(
            registry.issue(op_cert, 0),
            Err(CertificateError::IssuerNotPermittedToIssue)
        );
    }

    #[test]
    fn forged_signature_is_rejected_at_issuance() {
        let root_key = keypair(1);
        let mod_key = keypair(2);
        let wrong_key = keypair(9);
        let mut registry = CertificateRegistry::new();
        let root_hash = registry
            .install_root_certificate(root_cert(&root_key), &root_key.public_bytes())
            .unwrap();
        // Signed with the wrong key, claiming to be issued by root.
        let mut forged = child_cert(
            2,
            10,
            PrincipalKind::Mod,
            &mod_key,
            &wrong_key,
            root_hash,
            true,
            0,
            None,
        );
        forged.issuer_certificate_hash = root_hash;
        assert_eq!(
            registry.issue(forged, 0),
            Err(CertificateError::SignatureInvalid)
        );
    }

    #[test]
    fn revoked_certificate_fails_chain_verification() {
        let root_key = keypair(1);
        let mod_key = keypair(2);
        let mut registry = CertificateRegistry::new();
        let root_hash = registry
            .install_root_certificate(root_cert(&root_key), &root_key.public_bytes())
            .unwrap();
        let mod_cert = child_cert(
            2,
            10,
            PrincipalKind::Mod,
            &mod_key,
            &root_key,
            root_hash,
            true,
            0,
            None,
        );
        let mod_hash = registry.issue(mod_cert.clone(), 0).unwrap();
        registry.revoke(mod_hash).unwrap();
        assert_eq!(
            registry.verify_chain(&mod_cert, 0),
            Err(CertificateError::Revoked)
        );
    }

    #[test]
    fn revoking_an_issuer_breaks_its_children_chain_verification() {
        let root_key = keypair(1);
        let mod_key = keypair(2);
        let op_key = keypair(3);
        let mut registry = CertificateRegistry::new();
        let root_hash = registry
            .install_root_certificate(root_cert(&root_key), &root_key.public_bytes())
            .unwrap();
        let mod_cert = child_cert(
            2,
            10,
            PrincipalKind::Mod,
            &mod_key,
            &root_key,
            root_hash,
            true,
            0,
            None,
        );
        let mod_hash = registry.issue(mod_cert, 0).unwrap();
        let op_cert = child_cert(
            3,
            20,
            PrincipalKind::Operator,
            &op_key,
            &mod_key,
            mod_hash,
            false,
            0,
            None,
        );
        registry.issue(op_cert.clone(), 0).unwrap();
        registry.revoke(mod_hash).unwrap();
        assert_eq!(
            registry.verify_chain(&op_cert, 0),
            Err(CertificateError::Revoked)
        );
    }

    #[test]
    fn authority_generation_before_start_is_rejected() {
        let root_key = keypair(1);
        let mod_key = keypair(2);
        let mut registry = CertificateRegistry::new();
        let root_hash = registry
            .install_root_certificate(root_cert(&root_key), &root_key.public_bytes())
            .unwrap();
        let mod_cert = child_cert(
            2,
            10,
            PrincipalKind::Mod,
            &mod_key,
            &root_key,
            root_hash,
            true,
            100, // not valid until generation 100
            None,
        );
        registry.issue(mod_cert.clone(), 100).unwrap();
        assert_eq!(
            registry.verify_chain(&mod_cert, 50),
            Err(CertificateError::AuthorityGenerationInvalid)
        );
        assert_eq!(
            registry.verify_chain(&mod_cert, 100),
            Ok(certificate_hash(&mod_cert))
        );
    }

    #[test]
    fn authority_generation_after_end_is_rejected() {
        let root_key = keypair(1);
        let mod_key = keypair(2);
        let mut registry = CertificateRegistry::new();
        let root_hash = registry
            .install_root_certificate(root_cert(&root_key), &root_key.public_bytes())
            .unwrap();
        let mod_cert = child_cert(
            2,
            10,
            PrincipalKind::Mod,
            &mod_key,
            &root_key,
            root_hash,
            true,
            0,
            Some(10),
        );
        registry.issue(mod_cert.clone(), 0).unwrap();
        assert_eq!(
            registry.verify_chain(&mod_cert, 10),
            Ok(certificate_hash(&mod_cert))
        );
        assert_eq!(
            registry.verify_chain(&mod_cert, 11),
            Err(CertificateError::AuthorityGenerationInvalid)
        );
    }

    #[test]
    fn chain_cycle_is_detected_not_looped_forever() {
        // `issue`'s own invariant (an issuer must already be registered
        // before a child can reference it) makes a real cycle impossible to
        // construct through the public API: two certificates cannot each be
        // the other's already-existing issuer, and a certificate cannot
        // reference its own not-yet-computed hash (that would require a
        // hash preimage of itself). This test therefore white-box-inserts a
        // structurally cyclic registry state directly (bypassing `issue`)
        // to prove `verify_chain`'s defensive cycle guard actually fires
        // rather than looping forever, even though the real issuance path
        // can never produce this state.
        let root_key = keypair(1);
        let a_key = keypair(2);
        let self_loop_key: StateHash = [42u8; 32];

        let mut looped = UserCertificateV1 {
            version: CERTIFICATE_VERSION_V1,
            serial: 2,
            subject: 10,
            subject_kind: PrincipalKind::Mod,
            subject_public_key: a_key.public_bytes(),
            issuer: 10,
            // Points at the exact map key it will be stored under below —
            // a genuine self-loop, chosen directly rather than solved for.
            issuer_certificate_hash: self_loop_key,
            policy_root: [0u8; 32],
            authority_generation_start: 0,
            authority_generation_end: None,
            allowed_scope: SCOPE_OBJECT,
            flags: CERT_FLAG_CAN_ISSUE,
            signature: [0u8; 64],
        };
        // Genuinely self-signed: `looped` really does verify against its
        // own declared `subject_public_key`, so the per-hop signature check
        // does not itself block the walk before the cycle guard can fire.
        looped.signature = sign_certificate(&looped, &a_key);

        let mut registry = CertificateRegistry::new();
        registry
            .install_root_certificate(root_cert(&root_key), &root_key.public_bytes())
            .unwrap();
        registry.certs_by_hash.insert(self_loop_key, looped.clone());

        assert_eq!(
            registry.verify_chain(&looped, 0),
            Err(CertificateError::ChainCycle)
        );
    }

    #[test]
    fn chain_deeper_than_max_depth_is_rejected() {
        let root_key = keypair(1);
        let mut registry = CertificateRegistry::new();
        let mut prev_hash = registry
            .install_root_certificate(root_cert(&root_key), &root_key.public_bytes())
            .unwrap();
        let mut prev_key = root_key;
        let mut leaf: Option<UserCertificateV1> = None;
        // Build a chain of MAX_CHAIN_DEPTH + 1 hops so it must exceed the bound.
        for i in 0..(MAX_CHAIN_DEPTH as u64 + 1) {
            let key = keypair(10 + i as u8);
            let cert = child_cert(
                100 + i,
                (200 + i) as PrincipalId,
                PrincipalKind::Mod,
                &key,
                &prev_key,
                prev_hash,
                true,
                0,
                None,
            );
            let hash = registry.issue(cert.clone(), 0).unwrap();
            prev_hash = hash;
            prev_key = key;
            leaf = Some(cert);
        }
        assert_eq!(
            registry.verify_chain(&leaf.unwrap(), 0),
            Err(CertificateError::ChainTooDeep)
        );
    }

    #[test]
    fn scope_mask_helpers_round_trip() {
        let mask = SCOPE_OBJECT | SCOPE_CLUSTER;
        assert!(scope_allowed(mask, AuthorityScope::Object));
        assert!(!scope_allowed(mask, AuthorityScope::Host));
        assert!(scope_allowed(mask, AuthorityScope::Cluster));
    }
}
