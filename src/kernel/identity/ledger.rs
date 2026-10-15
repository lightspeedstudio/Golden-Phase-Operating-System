//! Identity ledger (`kernel::identity`, Gate 5) — `IdentityEventKind`,
//! the hash-chained event log every identity mutation is recorded into,
//! per-category Merkle roots, and the `AuthorityStateRoot` equation that
//! binds them together with `A_0`.
//!
//! ## What this reuses
//! - [`crate::kernel::crypto::phase_hash::ph_identity_chain`] /
//!   `ph_authority_state_root` — new domains added alongside this file,
//!   same `tuple_hash` construction as every other PhaseHash variant.
//! - [`crate::kernel::state_root::merkle_root`] — the same Merkle
//!   construction [`crate::kernel::identity::credential::Blocklist::root`]
//!   and `StatechainLedger::subsystem_merkle_root` already use.
//! - [`crate::kernel::crypto::phase_hash::ph_content`] — payloads are
//!   hashed before being bound into a chain link; the ledger never stores
//!   plaintext secrets (verifiers are already one-way Argon2id output, and
//!   even those are only ever committed as a further hash, never raw).
//!
//! ## Relationship to the real `StatechainLedger` (same pattern as `txlog`)
//! Exactly like `kernel::txlog` (see that module's own docs and
//! `append_ledger_event`), this ledger keeps **its own** internal hash
//! chain (`L_n = PH_identity-chain(L_{n-1}, ...)`) and is *also*
//! checkpointed into the real `StatechainLedger` — not a second, competing
//! statechain. [`crate::kernel::Kernel::identity_checkpoint_statechain`]
//! performs that checkpoint (`ObjectKind::Syscall`, a single fixed object
//! id [`IDENTITY_LEDGER_CHECKPOINT_OBJECT_ID`] (`31`),
//! `SubsystemKind::Syscall`). `31` is a reserved small id: `StatechainLedger`
//! indexes subsystem heads by `object_id`, so a large sentinel (e.g.
//! `u32::MAX`) would try to allocate a multi-gigabyte head vector. `31`
//! sits above typical init/shell `ProcessId` values used by
//! `record_syscall_event` on the same `(ObjectKind::Syscall, _)` key space.
//! Tests still match on `operation_name ==
//! "identity_authority_state_checkpoint"` rather than assuming the chain
//! contains only identity events.
//!
//! ## Honest disclosure: the retained event window is bounded
//! [`MAX_IDENTITY_LEDGER_EVENTS`] bounds the `events` list actually kept in
//! RAM (a ring, oldest evicted first — the same bounded-ring model
//! `kernel::txlog::TXLOG_MAX_RECORDS` already uses). This is an
//! **operational journal** for audit/stats/[`IdentityLedger::verify`]
//! purposes, exactly like `txlog`'s own `TxRecord` ring is *not* the real
//! chain of record. The real, unbounded chain of record is
//! [`IdentityLedger::chain_head`] (`L_n`) itself: every new event
//! cryptographically extends it regardless of whether earlier events are
//! still retained in RAM, the same way `StatechainLedger`'s own
//! `kernel_root` does not require retaining every historical event either.
//! [`IdentityLedger::verify`] can therefore only re-derive the chain across
//! whatever window is still retained, not from genesis once eviction has
//! occurred — disclosed here rather than overclaimed.
//!
//! ## Non-goals (Gate 5)
//! - No persistence (Gate 6): the ledger is RAM-only here, like every
//!   other subsystem before its own persistence gate.
//! - No sessions/`mod` tickets/capabilities event kinds yet (Gates 7-8);
//!   [`IdentityEventKind`] is appended to by later gates the same way
//!   [`crate::kernel::state_proof::ObjectKind`]/[`crate::kernel::state_root::SubsystemKind`]
//!   were appended to for Craft Mainframe — existing discriminants are
//!   never renumbered.

use crate::kernel::crypto::phase_hash::{
    ph_authority_state_root, ph_content, ph_identity_chain, AuthorityStateRootInput,
    IdentityChainInput,
};
use crate::kernel::hash::StateHash;
use crate::kernel::identity::principal::PrincipalId;
use crate::kernel::state_root::merkle_root;
use crate::platform::Vec;

/// Hard bound on the number of events retained in [`IdentityLedger::events`]
/// (a ring; oldest evicted first) and on each per-category leaf list. See
/// module docs, "Honest disclosure: the retained event window is bounded".
pub const MAX_IDENTITY_LEDGER_EVENTS: usize = 1024;

/// Statechain object id used by
/// [`crate::kernel::Kernel::identity_checkpoint_statechain`] for this
/// ledger's one global checkpoint stream. Must stay small:
/// `StatechainLedger` indexes subsystem heads by `object_id`, so a large
/// sentinel would allocate a multi-gigabyte vector. `31` is reserved for
/// this stream (above typical init/shell process ids).
pub const IDENTITY_LEDGER_CHECKPOINT_OBJECT_ID: u32 = 31;

/// The identity-subsystem event kinds this gate recognizes. Every variant
/// is a real, currently-wired state-changing operation (principal
/// lifecycle, certificate issuance/revocation, Root credential lifecycle) —
/// no speculative/unused variants. New variants are always appended, never
/// inserted (`as_u8`/`from_u8` are the wire-stable encoding once persisted
/// in Gate 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityEventKind {
    PrincipalCreated,
    PrincipalDisabled,
    PrincipalEnabled,
    RootCertificateInstalled,
    CertificateIssued,
    CertificateRevoked,
    CredentialProvisioned,
    CredentialChanged,
    CredentialLocked,
    CredentialUnlocked,
    CredentialRecoveryRequired,
    PolicyChanged,
    SessionOpened,
    SessionClosed,
    ModTicketIssued,
    ModTicketRevoked,
    CapabilityGranted,
    CapabilityRevoked,
}

/// Which per-category Merkle root ([`IdentityLedger::principal_root`] etc.)
/// an [`IdentityEventKind`] contributes its event hash to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerCategory {
    Principal,
    Certificate,
    Credential,
    Revocation,
    Policy,
}

impl IdentityEventKind {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::PrincipalCreated => 0,
            Self::PrincipalDisabled => 1,
            Self::PrincipalEnabled => 2,
            Self::RootCertificateInstalled => 3,
            Self::CertificateIssued => 4,
            Self::CertificateRevoked => 5,
            Self::CredentialProvisioned => 6,
            Self::CredentialChanged => 7,
            Self::CredentialLocked => 8,
            Self::CredentialUnlocked => 9,
            Self::CredentialRecoveryRequired => 10,
            Self::PolicyChanged => 11,
            Self::SessionOpened => 12,
            Self::SessionClosed => 13,
            Self::ModTicketIssued => 14,
            Self::ModTicketRevoked => 15,
            Self::CapabilityGranted => 16,
            Self::CapabilityRevoked => 17,
        }
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::PrincipalCreated),
            1 => Some(Self::PrincipalDisabled),
            2 => Some(Self::PrincipalEnabled),
            3 => Some(Self::RootCertificateInstalled),
            4 => Some(Self::CertificateIssued),
            5 => Some(Self::CertificateRevoked),
            6 => Some(Self::CredentialProvisioned),
            7 => Some(Self::CredentialChanged),
            8 => Some(Self::CredentialLocked),
            9 => Some(Self::CredentialUnlocked),
            10 => Some(Self::CredentialRecoveryRequired),
            11 => Some(Self::PolicyChanged),
            12 => Some(Self::SessionOpened),
            13 => Some(Self::SessionClosed),
            14 => Some(Self::ModTicketIssued),
            15 => Some(Self::ModTicketRevoked),
            16 => Some(Self::CapabilityGranted),
            17 => Some(Self::CapabilityRevoked),
            _ => None,
        }
    }

    /// See [`LedgerCategory`].
    pub fn category(self) -> LedgerCategory {
        match self {
            Self::PrincipalCreated | Self::PrincipalDisabled | Self::PrincipalEnabled
            | Self::SessionOpened | Self::SessionClosed => LedgerCategory::Principal,
            Self::RootCertificateInstalled | Self::CertificateIssued => {
                LedgerCategory::Certificate
            }
            Self::CredentialProvisioned
            | Self::CredentialChanged
            | Self::CredentialLocked
            | Self::CredentialUnlocked
            | Self::CredentialRecoveryRequired => LedgerCategory::Credential,
            Self::CertificateRevoked
            | Self::ModTicketRevoked
            | Self::CapabilityRevoked => LedgerCategory::Revocation,
            Self::PolicyChanged
            | Self::ModTicketIssued
            | Self::CapabilityGranted => LedgerCategory::Policy,
        }
    }
}

/// One recorded, hash-chained identity event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentityEvent {
    pub sequence: u64,
    pub kind: IdentityEventKind,
    pub principal_id: PrincipalId,
    pub tick: u64,
    pub payload_hash: StateHash,
    pub previous_hash: StateHash,
    pub event_hash: StateHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerError {
    /// A retained event's own `event_hash` does not recompute correctly
    /// from its recorded fields.
    EventHash,
    /// Two adjacent retained events do not actually link
    /// (`events[i].event_hash != events[i+1].previous_hash`).
    Fork,
    /// The last retained event's `event_hash` does not match
    /// [`IdentityLedger::chain_head`].
    HeadMismatch,
}

fn push_bounded<T>(list: &mut Vec<T>, item: T, max: usize) {
    if list.len() >= max {
        list.remove(0);
    }
    list.push(item);
}

/// Hash-chained identity event log plus the five per-category Merkle leaf
/// lists. See module docs for the full contract.
#[derive(Debug, Clone)]
pub struct IdentityLedger {
    sequence: u64,
    chain_head: StateHash,
    events: Vec<IdentityEvent>,
    principal_leaves: Vec<StateHash>,
    certificate_leaves: Vec<StateHash>,
    credential_leaves: Vec<StateHash>,
    revocation_leaves: Vec<StateHash>,
    policy_leaves: Vec<StateHash>,
}

impl Default for IdentityLedger {
    fn default() -> Self {
        Self::new()
    }
}

impl IdentityLedger {
    pub fn new() -> Self {
        Self {
            sequence: 0,
            chain_head: [0u8; 32],
            events: Vec::new(),
            principal_leaves: Vec::new(),
            certificate_leaves: Vec::new(),
            credential_leaves: Vec::new(),
            revocation_leaves: Vec::new(),
            policy_leaves: Vec::new(),
        }
    }

    pub fn chain_head(&self) -> StateHash {
        self.chain_head
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn events(&self) -> &[IdentityEvent] {
        &self.events
    }

    pub fn principal_root(&self) -> StateHash {
        merkle_root(&self.principal_leaves)
    }

    pub fn certificate_root(&self) -> StateHash {
        merkle_root(&self.certificate_leaves)
    }

    pub fn credential_root(&self) -> StateHash {
        merkle_root(&self.credential_leaves)
    }

    pub fn revocation_root(&self) -> StateHash {
        merkle_root(&self.revocation_leaves)
    }

    pub fn policy_root(&self) -> StateHash {
        merkle_root(&self.policy_leaves)
    }

    fn category_leaves_mut(&mut self, category: LedgerCategory) -> &mut Vec<StateHash> {
        match category {
            LedgerCategory::Principal => &mut self.principal_leaves,
            LedgerCategory::Certificate => &mut self.certificate_leaves,
            LedgerCategory::Credential => &mut self.credential_leaves,
            LedgerCategory::Revocation => &mut self.revocation_leaves,
            LedgerCategory::Policy => &mut self.policy_leaves,
        }
    }

    /// Append one event: hashes `payload` (never stored as plaintext),
    /// extends the hash chain (`chain_head` becomes the new event's
    /// `event_hash`), and pushes that same `event_hash` onto the event
    /// kind's category leaf list (see [`IdentityEventKind::category`]).
    /// Bounded per module docs; returns the appended event (its
    /// `event_hash` is the new [`Self::chain_head`]).
    pub fn record(
        &mut self,
        kind: IdentityEventKind,
        principal_id: PrincipalId,
        tick: u64,
        payload: &[u8],
    ) -> IdentityEvent {
        let payload_hash = ph_content(payload);
        let previous_hash = self.chain_head;
        let sequence = self.sequence;
        let event_hash = ph_identity_chain(&IdentityChainInput {
            previous: previous_hash,
            sequence,
            kind: kind.as_u8(),
            principal_id,
            tick,
            payload_hash,
        });
        let event = IdentityEvent {
            sequence,
            kind,
            principal_id,
            tick,
            payload_hash,
            previous_hash,
            event_hash,
        };
        self.sequence = self.sequence.saturating_add(1);
        self.chain_head = event_hash;
        push_bounded(&mut self.events, event, MAX_IDENTITY_LEDGER_EVENTS);
        let leaves = self.category_leaves_mut(kind.category());
        push_bounded(leaves, event_hash, MAX_IDENTITY_LEDGER_EVENTS);
        event
    }

    /// Re-derive the chain across the retained event window and confirm it
    /// ends at [`Self::chain_head`]. See module docs, "Honest disclosure:
    /// the retained event window is bounded" — this is a real check over
    /// whatever window survives eviction, not a from-genesis audit once
    /// eviction has occurred.
    pub fn verify(&self) -> Result<(), LedgerError> {
        let mut expected_prev: Option<StateHash> = None;
        let mut last_hash: Option<StateHash> = None;
        for event in &self.events {
            if let Some(expected) = expected_prev {
                if event.previous_hash != expected {
                    return Err(LedgerError::Fork);
                }
            }
            let recomputed = ph_identity_chain(&IdentityChainInput {
                previous: event.previous_hash,
                sequence: event.sequence,
                kind: event.kind.as_u8(),
                principal_id: event.principal_id,
                tick: event.tick,
                payload_hash: event.payload_hash,
            });
            if recomputed != event.event_hash {
                return Err(LedgerError::EventHash);
            }
            expected_prev = Some(event.event_hash);
            last_hash = Some(event.event_hash);
        }
        if let Some(last) = last_hash {
            if last != self.chain_head {
                return Err(LedgerError::HeadMismatch);
            }
        }
        Ok(())
    }

    /// Replace this ledger with a previously persisted snapshot. Does not
    /// re-hash events — callers must have stored the exact hashes (Gate 6).
    /// [`IdentityLedger::verify`] still re-derives them after restore.
    pub fn restore(
        &mut self,
        sequence: u64,
        chain_head: StateHash,
        events: Vec<IdentityEvent>,
        principal_leaves: Vec<StateHash>,
        certificate_leaves: Vec<StateHash>,
        credential_leaves: Vec<StateHash>,
        revocation_leaves: Vec<StateHash>,
        policy_leaves: Vec<StateHash>,
    ) {
        self.sequence = sequence;
        self.chain_head = chain_head;
        self.events = events;
        self.principal_leaves = principal_leaves;
        self.certificate_leaves = certificate_leaves;
        self.credential_leaves = credential_leaves;
        self.revocation_leaves = revocation_leaves;
        self.policy_leaves = policy_leaves;
    }

    pub fn principal_leaves(&self) -> &[StateHash] {
        &self.principal_leaves
    }

    pub fn certificate_leaves(&self) -> &[StateHash] {
        &self.certificate_leaves
    }

    pub fn credential_leaves(&self) -> &[StateHash] {
        &self.credential_leaves
    }

    pub fn revocation_leaves(&self) -> &[StateHash] {
        &self.revocation_leaves
    }

    pub fn policy_leaves(&self) -> &[StateHash] {
        &self.policy_leaves
    }
}

/// `AuthorityStateRoot = PH_authority-state(A_0, L_n, PrincipalRoot,
/// CertificateRoot, CredentialRoot, RevocationRoot, PolicyRoot)`. See
/// `kernel::identity::Identity::authority_state_root`.
pub fn compute_authority_state_root(authority_root: StateHash, ledger: &IdentityLedger) -> StateHash {
    ph_authority_state_root(&AuthorityStateRootInput {
        authority_root,
        chain_head: ledger.chain_head(),
        principal_root: ledger.principal_root(),
        certificate_root: ledger.certificate_root(),
        credential_root: ledger.credential_root(),
        revocation_root: ledger.revocation_root(),
        policy_root: ledger.policy_root(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_kind_byte_encoding_round_trips() {
        for kind in [
            IdentityEventKind::PrincipalCreated,
            IdentityEventKind::PrincipalDisabled,
            IdentityEventKind::PrincipalEnabled,
            IdentityEventKind::RootCertificateInstalled,
            IdentityEventKind::CertificateIssued,
            IdentityEventKind::CertificateRevoked,
            IdentityEventKind::CredentialProvisioned,
            IdentityEventKind::CredentialChanged,
            IdentityEventKind::CredentialLocked,
            IdentityEventKind::CredentialUnlocked,
            IdentityEventKind::CredentialRecoveryRequired,
            IdentityEventKind::PolicyChanged,
            IdentityEventKind::SessionOpened,
            IdentityEventKind::SessionClosed,
            IdentityEventKind::ModTicketIssued,
            IdentityEventKind::ModTicketRevoked,
            IdentityEventKind::CapabilityGranted,
            IdentityEventKind::CapabilityRevoked,
        ] {
            assert_eq!(IdentityEventKind::from_u8(kind.as_u8()), Some(kind));
        }
        assert_eq!(IdentityEventKind::from_u8(200), None);
    }

    #[test]
    fn new_ledger_starts_at_genesis() {
        let ledger = IdentityLedger::new();
        assert_eq!(ledger.chain_head(), [0u8; 32]);
        assert_eq!(ledger.sequence(), 0);
        assert_eq!(ledger.principal_root(), merkle_root(&[]));
    }

    #[test]
    fn record_extends_the_chain_head_and_sequence() {
        let mut ledger = IdentityLedger::new();
        let e0 = ledger.record(IdentityEventKind::PrincipalCreated, 5, 10, b"op-payload");
        assert_eq!(e0.sequence, 0);
        assert_eq!(e0.previous_hash, [0u8; 32]);
        assert_eq!(ledger.chain_head(), e0.event_hash);
        assert_eq!(ledger.sequence(), 1);

        let e1 = ledger.record(IdentityEventKind::PrincipalDisabled, 5, 11, b"");
        assert_eq!(e1.sequence, 1);
        assert_eq!(e1.previous_hash, e0.event_hash);
        assert_eq!(ledger.chain_head(), e1.event_hash);
        assert_ne!(e0.event_hash, e1.event_hash);
    }

    #[test]
    fn record_is_deterministic_given_identical_state_and_inputs() {
        let mut a = IdentityLedger::new();
        let mut b = IdentityLedger::new();
        let ea = a.record(IdentityEventKind::PrincipalCreated, 1, 0, b"x");
        let eb = b.record(IdentityEventKind::PrincipalCreated, 1, 0, b"x");
        assert_eq!(ea.event_hash, eb.event_hash);
    }

    #[test]
    fn each_category_only_receives_its_own_events() {
        let mut ledger = IdentityLedger::new();
        ledger.record(IdentityEventKind::PrincipalCreated, 1, 0, b"");
        let empty = merkle_root(&[]);
        assert_ne!(ledger.principal_root(), empty);
        assert_eq!(ledger.certificate_root(), empty);
        assert_eq!(ledger.credential_root(), empty);
        assert_eq!(ledger.revocation_root(), empty);
        assert_eq!(ledger.policy_root(), empty);

        ledger.record(IdentityEventKind::CertificateIssued, 2, 1, b"");
        assert_ne!(ledger.certificate_root(), empty);
        assert_eq!(ledger.credential_root(), empty);

        ledger.record(IdentityEventKind::CredentialProvisioned, 0, 2, b"");
        assert_ne!(ledger.credential_root(), empty);
        assert_eq!(ledger.revocation_root(), empty);

        ledger.record(IdentityEventKind::CertificateRevoked, 2, 3, b"");
        assert_ne!(ledger.revocation_root(), empty);
        assert_eq!(ledger.policy_root(), empty);

        ledger.record(IdentityEventKind::PolicyChanged, 0, 4, b"");
        assert_ne!(ledger.policy_root(), empty);
    }

    #[test]
    fn verify_accepts_a_real_untampered_chain() {
        let mut ledger = IdentityLedger::new();
        for i in 0..10u64 {
            ledger.record(IdentityEventKind::PrincipalCreated, i as u32, i, b"op");
        }
        assert_eq!(ledger.verify(), Ok(()));
    }

    #[test]
    fn verify_detects_a_tampered_event_hash() {
        let mut ledger = IdentityLedger::new();
        ledger.record(IdentityEventKind::PrincipalCreated, 1, 0, b"op");
        ledger.record(IdentityEventKind::PrincipalCreated, 2, 1, b"op");
        ledger.events[0].event_hash[0] ^= 0xFF;
        assert_eq!(ledger.verify(), Err(LedgerError::EventHash));
    }

    #[test]
    fn verify_detects_a_broken_link_between_events() {
        let mut ledger = IdentityLedger::new();
        ledger.record(IdentityEventKind::PrincipalCreated, 1, 0, b"op");
        ledger.record(IdentityEventKind::PrincipalCreated, 2, 1, b"op");
        ledger.events[1].previous_hash[0] ^= 0xFF;
        assert_eq!(ledger.verify(), Err(LedgerError::Fork));
    }

    #[test]
    fn verify_detects_head_mismatch() {
        let mut ledger = IdentityLedger::new();
        ledger.record(IdentityEventKind::PrincipalCreated, 1, 0, b"op");
        ledger.chain_head[0] ^= 0xFF;
        assert_eq!(ledger.verify(), Err(LedgerError::HeadMismatch));
    }

    #[test]
    fn bounded_ring_evicts_oldest_events() {
        let mut ledger = IdentityLedger::new();
        for i in 0..(MAX_IDENTITY_LEDGER_EVENTS as u64 + 5) {
            ledger.record(IdentityEventKind::PrincipalCreated, i as u32, i, b"op");
        }
        assert_eq!(ledger.events().len(), MAX_IDENTITY_LEDGER_EVENTS);
        // The chain head still reflects the very latest event even though
        // earlier ones were evicted.
        assert_eq!(
            ledger.events().last().unwrap().event_hash,
            ledger.chain_head()
        );
        // A from-genesis verify is not claimed once eviction has occurred
        // (see module docs); the retained window itself must still be
        // internally consistent though.
        assert_eq!(ledger.verify(), Ok(()));
    }

    #[test]
    fn compute_authority_state_root_is_sensitive_to_authority_root_and_ledger() {
        let ledger_a = IdentityLedger::new();
        let mut ledger_b = IdentityLedger::new();
        ledger_b.record(IdentityEventKind::PrincipalCreated, 1, 0, b"op");

        let root = [7u8; 32];
        let a = compute_authority_state_root(root, &ledger_a);
        let b = compute_authority_state_root(root, &ledger_b);
        assert_ne!(a, b, "different ledger state must change AuthorityStateRoot");

        let other_root = [8u8; 32];
        let c = compute_authority_state_root(other_root, &ledger_a);
        assert_ne!(a, c, "different A_0 must change AuthorityStateRoot");
    }

    #[test]
    fn compute_authority_state_root_is_deterministic() {
        let mut ledger = IdentityLedger::new();
        ledger.record(IdentityEventKind::PrincipalCreated, 1, 0, b"op");
        let root = [3u8; 32];
        assert_eq!(
            compute_authority_state_root(root, &ledger),
            compute_authority_state_root(root, &ledger)
        );
    }
}
