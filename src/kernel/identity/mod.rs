//! User identity, password, certificate, and authority subsystem
//! (`feature = "identity"`) — Gate 1: principal types and the
//! `AuthorityRoot` trust anchor. Independent, like `pmm`/`captable`/
//! `irqreg`/`sched2`/`extmap`/`txlog` before it: default sealed boot must be
//! unchanged when this feature is off (see `Cargo.toml`'s doc comment on the
//! `identity` feature).
//!
//! ## Intended full trust chain (built gate-by-gate; see the identity build
//! plan)
//! ```text
//! Golden Phase mathematics
//!         v
//! EquationRoot
//!         v
//! AuthorityRoot          <- Gate 1 (this gate)
//!         v
//! Principals and user certificates   <- Gate 1 (principals) / Gate 2 (certs)
//!         v
//! Password credentials               <- Gate 3
//!         v
//! Identity and credential Merkle roots  <- Gate 5 (this gate)
//!         v
//! Identity Statechain ledger          <- Gate 5 (this gate)
//!         v
//! BootSessionRoot                     <- already real (kernel::boot_session)
//!         v
//! Authenticated sessions              <- Gate 7
//!         v
//! Scoped kernel capabilities          <- Gate 8
//!         v
//! mod delegated commands              <- Gate 9
//! ```
//!
//! ## What this reuses / does not reuse (Gate 1)
//! - [`crate::kernel::state_root::compute_equation_root`] — the real
//!   `EquationRoot`, computed independently here exactly the way
//!   `StatechainLedger::new()` already does, not duplicated math.
//! - [`crate::kernel::crypto::phase_hash::ph_authority_root`] — new domain,
//!   same `tuple_hash` construction every other PhaseHash variant uses.
//! - [`crate::platform::BTreeMap`] for the bounded principal table — the
//!   repository's existing `std`/`alloc` platform shim, not a new
//!   collection type.
//!
//! ## Honest disclosure: installation identity (`I`)
//! The platform spec calls for "existing installation or device identity."
//! No hardware DNA/eFuse reader exists anywhere in this repository today
//! (confirmed by audit — see the identity build plan's Gate 0 section) and
//! this gate does not invent one. [`Identity::installation_id`] is an
//! explicit, operator/deployment-supplied 32-byte value (default
//! `[0u8; 32]` until set) — not a hardware-read unique ID. Any future gate
//! that wants real per-board uniqueness must add a real DNA/eFuse driver
//! first; this module will not silently claim uniqueness it cannot prove.
//!
//! ## What this gate adds (Gate 5)
//! - [`ledger::IdentityLedger`] — every principal/certificate/credential
//!   mutation performed through the [`Identity`] wrapper methods below
//!   ([`Identity::create_principal`], [`Identity::install_root_certificate`],
//!   [`Identity::provision_root_credential`], etc. — see each method) is
//!   recorded as a real, hash-chained [`ledger::IdentityEvent`], not a
//!   synthetic/example one. [`crate::userland::commands::passwd_cmd`]
//!   (Gate 4's one real interactive call site) was rewired in this gate to
//!   go through [`crate::kernel::Kernel::identity_provision_root_credential`]
//!   / `identity_change_root_password` / `identity_authenticate_root_credential`
//!   instead of touching [`RootCredential`] directly, so the ledger is fed
//!   by the subsystem's actual existing user-facing flow, not left unused.
//! - [`Identity::authority_state_root`] — `AuthorityStateRoot`, recomputed
//!   after every ledger event, binding `A_0` to the ledger's chain head and
//!   its five per-category Merkle roots. See [`ledger`] module docs for the
//!   full equation and the honest disclosure on its bounded retained
//!   window.
//! - [`crate::kernel::Kernel::identity_checkpoint_statechain`] checkpoints
//!   `AuthorityStateRoot` into the real `StatechainLedger`, the same
//!   pattern `kernel::txlog` already established (own internal chain,
//!   *also* checkpointed into the one real statechain — never a second,
//!   competing statechain).
//!
//! ## Non-goals (Gates 1-5)
//! - No sessions, `mod` tickets, or capabilities yet — Gates 7-8.
//!   [`ledger::IdentityEventKind::PolicyChanged`] exists (so the
//!   `PolicyRoot` category is real and ready) but has no real call site
//!   yet — no policy-mutation command exists in this repo before Gate 9;
//!   disclosed here rather than wired to a fabricated call site.
//! - No pinned Root public key installed on the *default* `Identity`
//!   instance yet: [`Identity::root_pubkey_hash`] stays `[0u8; 32]` until
//!   a later gate's provisioning flow sets a real one and calls
//!   [`certificate::CertificateRegistry::install_root_certificate`]
//!   (through [`Identity::install_root_certificate`]). Per the platform
//!   spec, `Unprovisioned` must never mean open access.
//! - Persistence is Gate 6 (`store`) — RAM is still the live copy; the
//!   store is a snapshot of it.
//! - No wiring into `kernel::syscall` enforcement yet — Gate 9.

pub mod authority_root;
pub mod capability;
pub mod certificate;
pub mod credential;
pub mod ledger;
pub mod mod_ticket;
pub mod phase_guard;
pub mod principal;
pub mod session;
pub mod store;

pub use capability::{IdentityCapError, IdentityCapId, IdentityCapability, IdentityCapTable};
pub use certificate::{CertificateError, CertificateRegistry, UserCertificateV1};
pub use credential::{
    CredentialError, PasswordPolicy, PasswordVerifier, RootCredential, RootCredentialState,
};
pub use ledger::{
    IdentityEvent, IdentityEventKind, IdentityLedger, LedgerCategory, LedgerError,
    IDENTITY_LEDGER_CHECKPOINT_OBJECT_ID, MAX_IDENTITY_LEDGER_EVENTS,
};
pub use mod_ticket::{ModOperation, ModTicket, TicketError, TicketTable};
pub use phase_guard::PhaseGuardState;
pub use principal::{
    AuthorityScope, NodeId, PrincipalError, PrincipalId, PrincipalKind, PrincipalRecord,
    PrincipalTable, SessionOrigin, ROOT_PRINCIPAL_ID,
};
pub use session::{
    AuthenticationMethod, SecurityContextId, SessionError, SessionId, SessionTable, UserSession,
};
pub use store::{IdentityStore, StoreError, PHASEFS_STORE_PATH};

use crate::kernel::hash::StateHash;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorityError {
    Session(SessionError),
    Ticket(TicketError),
    Capability(IdentityCapError),
    Principal(PrincipalError),
    PrincipalDisabled,
    KindNotPermitted,
    PhaseGuard,
    AuthenticationFailed,
}

/// Kernel-owned identity subsystem state. See module docs for full scope.
#[derive(Debug, Clone)]
pub struct Identity {
    principals: PrincipalTable,
    certificates: CertificateRegistry,
    root_credential: RootCredential,
    password_policy: PasswordPolicy,
    /// Installation/device identity (`I`). See module docs, "Honest
    /// disclosure".
    installation_id: StateHash,
    /// `H(PK_root)`. `[0u8; 32]` until Root is provisioned (later gate).
    root_pubkey_hash: StateHash,
    /// Hash of the current Root/authority policy record. `[0u8; 32]`
    /// (hash of the empty policy) until a later gate provisions a real
    /// policy record.
    policy_hash: StateHash,
    /// `A_0`, recomputed by [`Identity::recompute_authority_root`] whenever
    /// any of the three fields above changes. Computed once at
    /// construction with the defaults above so it is always a real,
    /// non-uninitialized value.
    authority_root: StateHash,
    /// Hash-chained event log for every mutation performed through this
    /// struct's wrapper methods — see [`ledger`] module docs (Gate 5).
    ledger: IdentityLedger,
    /// `AuthorityStateRoot`, recomputed by
    /// [`Identity::recompute_authority_state_root`] whenever
    /// `authority_root` changes or a ledger event is recorded.
    authority_state_root: StateHash,
    sessions: SessionTable,
    tickets: TicketTable,
    caps: IdentityCapTable,
    phase_guard: PhaseGuardState,
}

impl Default for Identity {
    fn default() -> Self {
        Self::new()
    }
}

impl Identity {
    pub fn new() -> Self {
        // Independent, matching `StatechainLedger::new()`'s own call to the
        // same function — not a second, divergent equation-root
        // computation.
        let equation_root = crate::kernel::state_root::compute_equation_root().hash;
        let installation_id = [0u8; 32];
        let root_pubkey_hash = [0u8; 32];
        let policy_hash = [0u8; 32];
        let authority_root = authority_root::compute_authority_root(
            equation_root,
            installation_id,
            root_pubkey_hash,
            policy_hash,
        );
        let ledger = IdentityLedger::new();
        let authority_state_root = ledger::compute_authority_state_root(authority_root, &ledger);
        Self {
            principals: PrincipalTable::new(),
            certificates: CertificateRegistry::new(),
            root_credential: RootCredential::new(),
            password_policy: PasswordPolicy::default(),
            installation_id,
            root_pubkey_hash,
            policy_hash,
            authority_root,
            ledger,
            authority_state_root,
            sessions: SessionTable::new(),
            tickets: TicketTable::new(),
            caps: IdentityCapTable::new(),
            phase_guard: PhaseGuardState::Valid,
        }
    }

    pub fn principals(&self) -> &PrincipalTable {
        &self.principals
    }

    pub fn principals_mut(&mut self) -> &mut PrincipalTable {
        &mut self.principals
    }

    pub fn certificates(&self) -> &CertificateRegistry {
        &self.certificates
    }

    pub fn certificates_mut(&mut self) -> &mut CertificateRegistry {
        &mut self.certificates
    }

    pub fn root_credential(&self) -> &RootCredential {
        &self.root_credential
    }

    pub fn root_credential_mut(&mut self) -> &mut RootCredential {
        &mut self.root_credential
    }

    pub fn password_policy(&self) -> &PasswordPolicy {
        &self.password_policy
    }

    pub fn set_password_policy(&mut self, policy: PasswordPolicy) {
        self.password_policy = policy;
    }

    pub fn authority_root(&self) -> StateHash {
        self.authority_root
    }

    pub fn installation_id(&self) -> StateHash {
        self.installation_id
    }

    pub fn root_pubkey_hash(&self) -> StateHash {
        self.root_pubkey_hash
    }

    pub fn policy_hash(&self) -> StateHash {
        self.policy_hash
    }

    /// Recompute `A_0` after `installation_id`/`root_pubkey_hash`/
    /// `policy_hash` changes (e.g. Root provisioning in a later gate).
    /// `A_0` is never mutated directly — only ever through this function —
    /// so it can never silently drift out of sync with its own inputs.
    /// Also recomputes `AuthorityStateRoot`, since it binds `A_0` too.
    fn recompute_authority_root(&mut self) {
        let equation_root = crate::kernel::state_root::compute_equation_root().hash;
        self.authority_root = authority_root::compute_authority_root(
            equation_root,
            self.installation_id,
            self.root_pubkey_hash,
            self.policy_hash,
        );
        self.recompute_authority_state_root();
    }

    /// Set the operator/deployment-supplied installation identity and
    /// recompute `A_0`. See module docs, "Honest disclosure".
    pub fn set_installation_id(&mut self, installation_id: StateHash) {
        self.installation_id = installation_id;
        self.recompute_authority_root();
    }

    /// Recompute `AuthorityStateRoot` from the current `A_0` and the
    /// ledger's current chain head/category roots. Called after every
    /// ledger-recording wrapper method below, and by
    /// [`Self::recompute_authority_root`] — never mutated directly, so it
    /// can never silently drift out of sync with either input (Gate 5).
    fn recompute_authority_state_root(&mut self) {
        self.authority_state_root =
            ledger::compute_authority_state_root(self.authority_root, &self.ledger);
    }

    /// `AuthorityStateRoot` — see [`ledger`] module docs for the full
    /// equation (Gate 5).
    pub fn authority_state_root(&self) -> StateHash {
        self.authority_state_root
    }

    /// The identity ledger — see [`ledger`] module docs (Gate 5).
    pub fn ledger(&self) -> &IdentityLedger {
        &self.ledger
    }

    /// Create a new non-Root principal, recording a real
    /// [`IdentityEventKind::PrincipalCreated`] event. See
    /// [`PrincipalTable::create`] for the underlying mechanism.
    pub fn create_principal(
        &mut self,
        kind: PrincipalKind,
        tick: u64,
    ) -> Result<PrincipalId, PrincipalError> {
        let id = self.principals.create(kind)?;
        self.ledger
            .record(IdentityEventKind::PrincipalCreated, id, tick, &[kind.as_u8()]);
        self.recompute_authority_state_root();
        Ok(id)
    }

    /// Disable a principal, recording a real
    /// [`IdentityEventKind::PrincipalDisabled`] event. See
    /// [`PrincipalTable::disable`] for the underlying mechanism.
    pub fn disable_principal(&mut self, id: PrincipalId, tick: u64) -> Result<(), PrincipalError> {
        self.principals.disable(id)?;
        self.revoke_live_grants_for_principal(id, tick);
        self.sessions.revoke_principal(id);
        self.ledger
            .record(IdentityEventKind::PrincipalDisabled, id, tick, &[]);
        self.recompute_authority_state_root();
        Ok(())
    }

    /// Enable a principal, recording a real
    /// [`IdentityEventKind::PrincipalEnabled`] event. See
    /// [`PrincipalTable::enable`] for the underlying mechanism.
    pub fn enable_principal(&mut self, id: PrincipalId, tick: u64) -> Result<(), PrincipalError> {
        self.principals.enable(id)?;
        self.ledger
            .record(IdentityEventKind::PrincipalEnabled, id, tick, &[]);
        self.recompute_authority_state_root();
        Ok(())
    }

    /// Install the pinned Root certificate, recording a real
    /// [`IdentityEventKind::RootCertificateInstalled`] event. See
    /// [`CertificateRegistry::install_root_certificate`] for the underlying
    /// mechanism.
    pub fn install_root_certificate(
        &mut self,
        cert: UserCertificateV1,
        root_public_key: &[u8; 32],
        tick: u64,
    ) -> Result<StateHash, CertificateError> {
        let subject = cert.subject;
        let hash = self
            .certificates
            .install_root_certificate(cert, root_public_key)?;
        self.ledger.record(
            IdentityEventKind::RootCertificateInstalled,
            subject,
            tick,
            &hash[..],
        );
        self.recompute_authority_state_root();
        Ok(hash)
    }

    /// Issue a new non-root certificate, recording a real
    /// [`IdentityEventKind::CertificateIssued`] event. See
    /// [`CertificateRegistry::issue`] for the underlying mechanism.
    pub fn issue_certificate(
        &mut self,
        cert: UserCertificateV1,
        now_generation: u64,
        tick: u64,
    ) -> Result<StateHash, CertificateError> {
        let subject = cert.subject;
        let hash = self.certificates.issue(cert, now_generation)?;
        self.ledger
            .record(IdentityEventKind::CertificateIssued, subject, tick, &hash[..]);
        self.recompute_authority_state_root();
        Ok(hash)
    }

    /// Revoke a certificate, recording a real
    /// [`IdentityEventKind::CertificateRevoked`] event. See
    /// [`CertificateRegistry::revoke`] for the underlying mechanism.
    pub fn revoke_certificate(
        &mut self,
        hash: StateHash,
        principal_id: PrincipalId,
        tick: u64,
    ) -> Result<(), CertificateError> {
        self.certificates.revoke(hash)?;
        self.ledger.record(
            IdentityEventKind::CertificateRevoked,
            principal_id,
            tick,
            &hash[..],
        );
        self.recompute_authority_state_root();
        Ok(())
    }

    /// Provision the Root credential (`Unprovisioned -> Active`), recording
    /// a real [`IdentityEventKind::CredentialProvisioned`] event. See
    /// [`RootCredential::provision`] for the underlying mechanism.
    pub fn provision_root_credential(
        &mut self,
        verifier: PasswordVerifier,
        tick: u64,
    ) -> Result<(), CredentialError> {
        self.root_credential.provision(verifier)?;
        self.ledger.record(
            IdentityEventKind::CredentialProvisioned,
            ROOT_PRINCIPAL_ID,
            tick,
            &[],
        );
        self.recompute_authority_state_root();
        Ok(())
    }

    /// Change the Root credential's password, recording a real
    /// [`IdentityEventKind::CredentialChanged`] event. See
    /// [`RootCredential::change_password`] for the underlying mechanism.
    pub fn change_root_password(
        &mut self,
        policy: &PasswordPolicy,
        verifier: PasswordVerifier,
        tick: u64,
    ) -> Result<(), CredentialError> {
        self.root_credential.change_password(policy, verifier)?;
        self.ledger.record(
            IdentityEventKind::CredentialChanged,
            ROOT_PRINCIPAL_ID,
            tick,
            &[],
        );
        self.recompute_authority_state_root();
        Ok(())
    }

    /// Attempt Root authentication. Unlike the other wrapper methods, a
    /// mere failed attempt is not itself recorded as a ledger event (that
    /// would let an attacker grow the ledger without bound purely by
    /// guessing) — but if this specific attempt causes
    /// [`RootCredential::authenticate`]'s own internal auto-lock
    /// (`state: Active -> Locked`) to fire, that real state transition is
    /// recorded as [`IdentityEventKind::CredentialLocked`].
    pub fn authenticate_root_credential(
        &mut self,
        policy: &PasswordPolicy,
        password: &[u8],
        now_chronos: u64,
        tick: u64,
    ) -> Result<bool, CredentialError> {
        let before = self.root_credential.state();
        let result = self.root_credential.authenticate(policy, password, now_chronos);
        if before != RootCredentialState::Locked
            && self.root_credential.state() == RootCredentialState::Locked
        {
            self.ledger.record(
                IdentityEventKind::CredentialLocked,
                ROOT_PRINCIPAL_ID,
                tick,
                &[],
            );
            self.recompute_authority_state_root();
        }
        result
    }

    /// Explicitly lock the Root credential, recording a real
    /// [`IdentityEventKind::CredentialLocked`] event. See
    /// [`RootCredential::lock`] for the underlying mechanism.
    pub fn lock_root_credential(&mut self, tick: u64) {
        let before = self.root_credential.state();
        self.root_credential.lock();
        if before != self.root_credential.state() {
            self.ledger.record(
                IdentityEventKind::CredentialLocked,
                ROOT_PRINCIPAL_ID,
                tick,
                &[],
            );
            self.recompute_authority_state_root();
        }
    }

    /// Unlock the Root credential, recording a real
    /// [`IdentityEventKind::CredentialUnlocked`] event. See
    /// [`RootCredential::unlock`] for the underlying mechanism.
    pub fn unlock_root_credential(&mut self, tick: u64) -> Result<(), CredentialError> {
        self.root_credential.unlock()?;
        self.ledger.record(
            IdentityEventKind::CredentialUnlocked,
            ROOT_PRINCIPAL_ID,
            tick,
            &[],
        );
        self.recompute_authority_state_root();
        Ok(())
    }

    /// Require recovery for the Root credential, recording a real
    /// [`IdentityEventKind::CredentialRecoveryRequired`] event. See
    /// [`RootCredential::require_recovery`] for the underlying mechanism.
    pub fn require_root_recovery(&mut self, tick: u64) {
        self.root_credential.require_recovery();
        self.ledger.record(
            IdentityEventKind::CredentialRecoveryRequired,
            ROOT_PRINCIPAL_ID,
            tick,
            &[],
        );
        self.recompute_authority_state_root();
    }

    pub fn sessions(&self) -> &SessionTable {
        &self.sessions
    }

    pub fn tickets(&self) -> &TicketTable {
        &self.tickets
    }

    pub fn caps(&self) -> &IdentityCapTable {
        &self.caps
    }

    pub fn caps_mut(&mut self) -> &mut IdentityCapTable {
        &mut self.caps
    }

    pub fn phase_guard(&self) -> PhaseGuardState {
        self.phase_guard
    }

    pub fn set_phase_guard(&mut self, state: PhaseGuardState) {
        self.phase_guard = state;
    }

    /// Open an authenticated session bound to `boot_session_root` and attach
    /// it as `process_id`'s security context (Gate 7).
    pub fn open_session(
        &mut self,
        principal_id: PrincipalId,
        origin: SessionOrigin,
        method: AuthenticationMethod,
        boot_session_root: StateHash,
        tick: u64,
        process_id: u32,
    ) -> Result<UserSession, SessionError> {
        let rec = self
            .principals
            .get(principal_id)
            .map_err(|_| SessionError::NotFound)?;
        if !rec.enabled {
            return Err(SessionError::PrincipalDisabled);
        }
        if rec.kind == PrincipalKind::Root && matches!(origin, SessionOrigin::Remote(_)) {
            return Err(SessionError::RemoteRootDenied);
        }
        let session = self.sessions.open(
            principal_id,
            origin,
            method,
            boot_session_root,
            tick,
            process_id,
        )?;
        self.ledger.record(
            IdentityEventKind::SessionOpened,
            principal_id,
            tick,
            &session.session_hash[..],
        );
        self.recompute_authority_state_root();
        Ok(session)
    }

    pub fn close_session(&mut self, id: SessionId, tick: u64) -> Result<UserSession, SessionError> {
        let session = self.sessions.close(id)?;
        self.revoke_live_grants_for_session(id, session.principal_id, tick);
        self.ledger.record(
            IdentityEventKind::SessionClosed,
            session.principal_id,
            tick,
            &session.session_hash[..],
        );
        self.recompute_authority_state_root();
        Ok(session)
    }

    /// Issue a capability + ticket for `op` on an already-authenticated
    /// session (Gate 8).
    pub fn issue_mod_ticket(
        &mut self,
        session_id: SessionId,
        op: ModOperation,
        boot_session_root: StateHash,
        now_tick: u64,
        lease_ticks: u64,
    ) -> Result<ModTicket, AuthorityError> {
        let session = self
            .sessions
            .require_bound(session_id, boot_session_root)
            .map_err(AuthorityError::Session)?;
        let rec = self
            .principals
            .get(session.principal_id)
            .map_err(|_| AuthorityError::Principal(PrincipalError::NotFound))?;
        if !rec.enabled {
            return Err(AuthorityError::PrincipalDisabled);
        }
        let recovery_only =
            matches!(op, ModOperation::CredentialUnlock | ModOperation::SessionClose);
        if recovery_only {
            if !self.phase_guard.allows_root_recovery() {
                return Err(AuthorityError::PhaseGuard);
            }
        } else if !self.phase_guard.allows_mod_elevation() {
            return Err(AuthorityError::PhaseGuard);
        }
        if rec.kind != PrincipalKind::Root && rec.kind != PrincipalKind::Mod {
            return Err(AuthorityError::KindNotPermitted);
        }
        let cap = self
            .caps
            .grant(session.principal_id, session_id, op, now_tick)
            .map_err(AuthorityError::Capability)?;
        self.ledger.record(
            IdentityEventKind::CapabilityGranted,
            session.principal_id,
            now_tick,
            &cap.proof[..],
        );
        let expires = now_tick.saturating_add(lease_ticks);
        let ticket = self
            .tickets
            .issue(session.principal_id, session_id, cap.cap_id, op, expires)
            .map_err(AuthorityError::Ticket)?;
        self.ledger.record(
            IdentityEventKind::ModTicketIssued,
            session.principal_id,
            now_tick,
            &[],
        );
        self.recompute_authority_state_root();
        Ok(ticket)
    }

    /// Section-12 AND equation (Gate 9): principal enabled ∧ session bound
    /// to this boot ∧ ticket live ∧ capability authorizes `op` ∧ PhaseGuard
    /// permits the class of operation.
    pub fn require(
        &self,
        ticket_id: crate::kernel::identity::mod_ticket::TicketId,
        op: ModOperation,
        boot_session_root: StateHash,
        now_tick: u64,
    ) -> Result<(), AuthorityError> {
        let ticket = self
            .tickets
            .require(ticket_id, op, now_tick)
            .map_err(AuthorityError::Ticket)?;
        let session = self
            .sessions
            .require_bound(ticket.session_id, boot_session_root)
            .map_err(AuthorityError::Session)?;
        if session.principal_id != ticket.principal_id {
            return Err(AuthorityError::Session(SessionError::Inactive));
        }
        let rec = self
            .principals
            .get(session.principal_id)
            .map_err(|_| AuthorityError::Principal(PrincipalError::NotFound))?;
        if !rec.enabled {
            return Err(AuthorityError::PrincipalDisabled);
        }
        self.caps
            .authorize(ticket.cap_id, op)
            .map_err(AuthorityError::Capability)?;
        let recovery_only =
            matches!(op, ModOperation::CredentialUnlock | ModOperation::SessionClose);
        if recovery_only {
            if !self.phase_guard.allows_root_recovery() {
                return Err(AuthorityError::PhaseGuard);
            }
        } else if !self.phase_guard.allows_mod_elevation() {
            return Err(AuthorityError::PhaseGuard);
        }
        Ok(())
    }

    /// Revoke one live ticket and its capability (Gate 8 `mod -k`).
    pub fn revoke_ticket(
        &mut self,
        ticket_id: crate::kernel::identity::mod_ticket::TicketId,
        tick: u64,
    ) -> Result<(), TicketError> {
        let ticket = self.tickets.get(ticket_id)?;
        if !ticket.active {
            return Err(TicketError::Inactive);
        }
        self.tickets.revoke(ticket_id)?;
        let _ = self.caps.revoke(ticket.cap_id);
        self.ledger.record(
            IdentityEventKind::ModTicketRevoked,
            ticket.principal_id,
            tick,
            &[],
        );
        self.ledger.record(
            IdentityEventKind::CapabilityRevoked,
            ticket.principal_id,
            tick,
            &ticket.cap_id.to_le_bytes(),
        );
        self.recompute_authority_state_root();
        Ok(())
    }

    fn revoke_live_grants_for_principal(&mut self, principal_id: PrincipalId, tick: u64) {
        let tickets: crate::platform::Vec<ModTicket> = self
            .tickets
            .active()
            .into_iter()
            .filter(|t| t.principal_id == principal_id)
            .collect();
        self.tickets.revoke_principal(principal_id);
        for t in tickets {
            self.ledger.record(
                IdentityEventKind::ModTicketRevoked,
                principal_id,
                tick,
                &[],
            );
            self.ledger.record(
                IdentityEventKind::CapabilityRevoked,
                principal_id,
                tick,
                &t.cap_id.to_le_bytes(),
            );
            let _ = self.caps.revoke(t.cap_id);
        }
        self.caps.revoke_principal(principal_id);
    }

    fn revoke_live_grants_for_session(
        &mut self,
        session_id: SessionId,
        principal_id: PrincipalId,
        tick: u64,
    ) {
        let tickets: crate::platform::Vec<ModTicket> = self
            .tickets
            .active()
            .into_iter()
            .filter(|t| t.session_id == session_id)
            .collect();
        self.tickets.revoke_session(session_id);
        for t in tickets {
            self.ledger.record(
                IdentityEventKind::ModTicketRevoked,
                principal_id,
                tick,
                &[],
            );
            self.ledger.record(
                IdentityEventKind::CapabilityRevoked,
                principal_id,
                tick,
                &t.cap_id.to_le_bytes(),
            );
            let _ = self.caps.revoke(t.cap_id);
        }
        self.caps.revoke_session(session_id);
    }

    /// Canonical snapshot of live identity state (Gate 6). Never includes
    /// plaintext passwords — verifiers are Argon2id output only.
    pub fn encode(&self) -> Result<crate::platform::Vec<u8>, StoreError> {
        let snap = store::EncodedIdentity {
            installation_id: self.installation_id,
            root_pubkey_hash: self.root_pubkey_hash,
            policy_hash: self.policy_hash,
            password_policy: self.password_policy,
            root_state: self.root_credential.state(),
            root_verifier: self.root_credential.verifier().copied(),
            root_history: self.root_credential.history().to_vec(),
            principals: self.principals.snapshot(),
            next_principal_id: self.principals.next_id(),
            certificates: self
                .certificates
                .snapshot()
                .into_iter()
                .map(|(_, c)| c)
                .collect(),
            revoked: self.certificates.revoked_snapshot(),
            root_certificate_hash: self.certificates.root_certificate_hash(),
            ledger_sequence: self.ledger.sequence(),
            ledger_chain_head: self.ledger.chain_head(),
            ledger_events: self.ledger.events().to_vec(),
            principal_leaves: self.ledger.principal_leaves().to_vec(),
            certificate_leaves: self.ledger.certificate_leaves().to_vec(),
            credential_leaves: self.ledger.credential_leaves().to_vec(),
            revocation_leaves: self.ledger.revocation_leaves().to_vec(),
            policy_leaves: self.ledger.policy_leaves().to_vec(),
        };
        store::encode_identity(&snap)
    }

    /// Replace live state with a previously encoded snapshot. Recomputes
    /// `A_0` and `AuthorityStateRoot` from the restored inputs rather than
    /// trusting a stored copy of those hashes.
    pub fn restore_from_bytes(&mut self, bytes: &[u8]) -> Result<(), StoreError> {
        let snap = store::decode_identity(bytes)?;
        self.principals
            .restore(snap.principals, snap.next_principal_id)
            .map_err(|_| StoreError::Principal)?;
        self.certificates
            .restore(
                snap.certificates,
                snap.revoked,
                snap.root_certificate_hash,
            )
            .map_err(StoreError::Decode)?;
        self.root_credential
            .restore(snap.root_state, snap.root_verifier, snap.root_history);
        self.password_policy = snap.password_policy;
        self.installation_id = snap.installation_id;
        self.root_pubkey_hash = snap.root_pubkey_hash;
        self.policy_hash = snap.policy_hash;
        self.ledger.restore(
            snap.ledger_sequence,
            snap.ledger_chain_head,
            snap.ledger_events,
            snap.principal_leaves,
            snap.certificate_leaves,
            snap.credential_leaves,
            snap.revocation_leaves,
            snap.policy_leaves,
        );
        self.ledger.verify().map_err(|_| StoreError::Credential)?;
        self.recompute_authority_root();
        Ok(())
    }

    pub fn persist_to(&self, backend: &mut dyn IdentityStore) -> Result<(), StoreError> {
        let bytes = self.encode()?;
        backend.save(&bytes)
    }

    pub fn load_from(&mut self, backend: &mut dyn IdentityStore) -> Result<(), StoreError> {
        match backend.load()? {
            Some(bytes) => self.restore_from_bytes(&bytes),
            None => Err(StoreError::NotFound),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_identity_has_root_principal_and_a_real_authority_root() {
        let identity = Identity::new();
        assert_eq!(identity.principals().len(), 1);
        assert_eq!(
            identity.principals().get(ROOT_PRINCIPAL_ID).unwrap().kind,
            PrincipalKind::Root
        );
        // Not all-zero: it is a real hash over (equation_root, zeros, zeros,
        // zeros), not an uninitialized/default StateHash.
        assert_ne!(identity.authority_root(), [0u8; 32]);
    }

    #[test]
    fn authority_root_is_deterministic_across_independent_instances() {
        let a = Identity::new();
        let b = Identity::new();
        assert_eq!(a.authority_root(), b.authority_root());
    }

    #[test]
    fn setting_installation_id_changes_the_authority_root() {
        let mut identity = Identity::new();
        let before = identity.authority_root();
        identity.set_installation_id([42u8; 32]);
        let after = identity.authority_root();
        assert_ne!(before, after);
        assert_eq!(identity.installation_id(), [42u8; 32]);
    }

    #[test]
    fn authority_root_matches_direct_computation() {
        let identity = Identity::new();
        let equation_root = crate::kernel::state_root::compute_equation_root().hash;
        let expected =
            authority_root::compute_authority_root(equation_root, [0u8; 32], [0u8; 32], [0u8; 32]);
        assert_eq!(identity.authority_root(), expected);
    }

    #[test]
    fn new_identity_has_a_real_genesis_authority_state_root() {
        let identity = Identity::new();
        assert_eq!(identity.ledger().chain_head(), [0u8; 32]);
        assert_ne!(identity.authority_state_root(), [0u8; 32]);
        assert_eq!(
            identity.authority_state_root(),
            ledger::compute_authority_state_root(identity.authority_root(), identity.ledger())
        );
    }

    #[test]
    fn create_principal_records_a_ledger_event_and_changes_authority_state_root() {
        let mut identity = Identity::new();
        let before_state_root = identity.authority_state_root();
        let before_head = identity.ledger().chain_head();

        let id = identity.create_principal(PrincipalKind::Operator, 1).unwrap();

        assert_ne!(id, ROOT_PRINCIPAL_ID);
        assert_eq!(identity.ledger().sequence(), 1);
        assert_ne!(identity.ledger().chain_head(), before_head);
        assert_ne!(identity.authority_state_root(), before_state_root);
        assert_eq!(
            identity.ledger().events()[0].kind,
            IdentityEventKind::PrincipalCreated
        );
        assert_eq!(identity.ledger().events()[0].principal_id, id);
    }

    #[test]
    fn disable_and_enable_principal_each_record_their_own_event() {
        let mut identity = Identity::new();
        let id = identity.create_principal(PrincipalKind::Guest, 0).unwrap();
        identity.disable_principal(id, 1).unwrap();
        assert_eq!(
            identity.ledger().events()[1].kind,
            IdentityEventKind::PrincipalDisabled
        );
        identity.enable_principal(id, 2).unwrap();
        assert_eq!(
            identity.ledger().events()[2].kind,
            IdentityEventKind::PrincipalEnabled
        );
        assert_eq!(identity.ledger().sequence(), 3);
    }

    #[test]
    fn provision_and_change_root_password_each_record_their_own_event() {
        let mut identity = Identity::new();
        let verifier = credential::create_verifier(
            b"a genuinely long enough root password",
            credential::DEFAULT_ARGON2_MEMORY_KIB,
            credential::DEFAULT_ARGON2_ITERATIONS,
            credential::DEFAULT_ARGON2_LANES,
            0,
        )
        .unwrap();
        identity.provision_root_credential(verifier, 0).unwrap();
        assert_eq!(
            identity.ledger().events()[0].kind,
            IdentityEventKind::CredentialProvisioned
        );
        assert_eq!(
            identity.ledger().events()[0].principal_id,
            ROOT_PRINCIPAL_ID
        );

        let policy = PasswordPolicy::default();
        let new_verifier = credential::create_verifier(
            b"a brand new root password value here",
            credential::DEFAULT_ARGON2_MEMORY_KIB,
            credential::DEFAULT_ARGON2_ITERATIONS,
            credential::DEFAULT_ARGON2_LANES,
            1,
        )
        .unwrap();
        identity
            .change_root_password(&policy, new_verifier, 1)
            .unwrap();
        assert_eq!(
            identity.ledger().events()[1].kind,
            IdentityEventKind::CredentialChanged
        );
        assert_eq!(identity.ledger().sequence(), 2);
    }

    #[test]
    fn authenticate_root_credential_records_a_lock_event_on_transition_to_locked() {
        let mut identity = Identity::new();
        let policy = PasswordPolicy {
            maximum_failures: 2,
            initial_delay_ticks: 0,
            maximum_delay_ticks: 0,
            ..PasswordPolicy::default()
        };
        let verifier = credential::create_verifier(
            b"a genuinely long enough root password",
            credential::DEFAULT_ARGON2_MEMORY_KIB,
            credential::DEFAULT_ARGON2_ITERATIONS,
            credential::DEFAULT_ARGON2_LANES,
            0,
        )
        .unwrap();
        identity.provision_root_credential(verifier, 0).unwrap();
        assert_eq!(identity.ledger().sequence(), 1);

        // A merely-wrong password (not yet reaching the lockout threshold)
        // must not add a ledger event.
        let _ = identity.authenticate_root_credential(&policy, b"wrong once", 0, 1);
        assert_eq!(identity.ledger().sequence(), 1);

        // The attempt that actually crosses `maximum_failures` and flips
        // Active -> Locked must record exactly one CredentialLocked event.
        let _ = identity.authenticate_root_credential(&policy, b"wrong twice", 1, 2);
        assert_eq!(identity.ledger().sequence(), 2);
        assert_eq!(
            identity.ledger().events()[1].kind,
            IdentityEventKind::CredentialLocked
        );
        assert_eq!(
            identity.root_credential().state(),
            RootCredentialState::Locked
        );
    }

    #[test]
    fn unlock_and_require_recovery_each_record_their_own_event() {
        let mut identity = Identity::new();
        let verifier = credential::create_verifier(
            b"a genuinely long enough root password",
            credential::DEFAULT_ARGON2_MEMORY_KIB,
            credential::DEFAULT_ARGON2_ITERATIONS,
            credential::DEFAULT_ARGON2_LANES,
            0,
        )
        .unwrap();
        identity.provision_root_credential(verifier, 0).unwrap();
        identity.lock_root_credential(1);
        assert_eq!(
            identity.ledger().events()[1].kind,
            IdentityEventKind::CredentialLocked
        );
        identity.unlock_root_credential(2).unwrap();
        assert_eq!(
            identity.ledger().events()[2].kind,
            IdentityEventKind::CredentialUnlocked
        );
        identity.require_root_recovery(3);
        assert_eq!(
            identity.ledger().events()[3].kind,
            IdentityEventKind::CredentialRecoveryRequired
        );
        assert_eq!(identity.ledger().sequence(), 4);
    }

    #[test]
    fn install_issue_and_revoke_certificate_each_record_their_own_event() {
        use crate::kernel::crypto::sign::SignKeypair;
        use certificate::{
            CERTIFICATE_VERSION_V1, CERT_FLAG_CAN_ISSUE, CERT_FLAG_ROOT, NO_ISSUER, SCOPE_HOST,
        };

        let mut identity = Identity::new();
        let root_key = SignKeypair::from_seed(&[1u8; 32]);
        let mut root_cert = UserCertificateV1 {
            version: CERTIFICATE_VERSION_V1,
            serial: 1,
            subject: ROOT_PRINCIPAL_ID,
            subject_kind: PrincipalKind::Root,
            subject_public_key: root_key.public_bytes(),
            issuer: ROOT_PRINCIPAL_ID,
            issuer_certificate_hash: NO_ISSUER,
            policy_root: [0u8; 32],
            authority_generation_start: 0,
            authority_generation_end: None,
            allowed_scope: SCOPE_HOST,
            flags: CERT_FLAG_ROOT | CERT_FLAG_CAN_ISSUE,
            signature: [0u8; 64],
        };
        root_cert.signature = certificate::sign_certificate(&root_cert, &root_key);

        let root_hash = identity
            .install_root_certificate(root_cert, &root_key.public_bytes(), 0)
            .unwrap();
        assert_eq!(
            identity.ledger().events()[0].kind,
            IdentityEventKind::RootCertificateInstalled
        );

        let mod_key = SignKeypair::from_seed(&[2u8; 32]);
        let mut mod_cert = UserCertificateV1 {
            version: CERTIFICATE_VERSION_V1,
            serial: 2,
            subject: 10,
            subject_kind: PrincipalKind::Mod,
            subject_public_key: mod_key.public_bytes(),
            issuer: ROOT_PRINCIPAL_ID,
            issuer_certificate_hash: root_hash,
            policy_root: [0u8; 32],
            authority_generation_start: 0,
            authority_generation_end: None,
            allowed_scope: SCOPE_HOST,
            flags: 0,
            signature: [0u8; 64],
        };
        mod_cert.signature = certificate::sign_certificate(&mod_cert, &root_key);
        let mod_hash = identity.issue_certificate(mod_cert, 0, 1).unwrap();
        assert_eq!(
            identity.ledger().events()[1].kind,
            IdentityEventKind::CertificateIssued
        );

        identity.revoke_certificate(mod_hash, 10, 2).unwrap();
        assert_eq!(
            identity.ledger().events()[2].kind,
            IdentityEventKind::CertificateRevoked
        );
        assert_ne!(
            identity.ledger().revocation_root(),
            crate::kernel::state_root::merkle_root(&[])
        );
        assert_eq!(identity.ledger().sequence(), 3);
    }

    #[test]
    fn remote_root_session_is_denied() {
        let mut identity = Identity::new();
        assert_eq!(
            identity.open_session(
                ROOT_PRINCIPAL_ID,
                SessionOrigin::Remote(1),
                AuthenticationMethod::Password,
                [1u8; 32],
                0,
                1,
            ),
            Err(SessionError::RemoteRootDenied)
        );
    }

    #[test]
    fn guest_cannot_issue_a_mod_ticket() {
        let mut identity = Identity::new();
        let id = identity.create_principal(PrincipalKind::Guest, 0).unwrap();
        let boot = [3u8; 32];
        let session = identity
            .open_session(
                id,
                SessionOrigin::Local,
                AuthenticationMethod::Password,
                boot,
                1,
                7,
            )
            .unwrap();
        assert_eq!(
            identity.issue_mod_ticket(session.session_id, ModOperation::FsRemove, boot, 1, 64),
            Err(AuthorityError::KindNotPermitted)
        );
    }

    #[test]
    fn require_rejects_a_boot_session_mismatch() {
        let mut identity = Identity::new();
        let boot = [9u8; 32];
        let session = identity
            .open_session(
                ROOT_PRINCIPAL_ID,
                SessionOrigin::Local,
                AuthenticationMethod::Password,
                boot,
                0,
                1,
            )
            .unwrap();
        let ticket = identity
            .issue_mod_ticket(session.session_id, ModOperation::FsRemove, boot, 0, 64)
            .unwrap();
        assert_eq!(
            identity.require(ticket.ticket_id, ModOperation::FsRemove, [8u8; 32], 0),
            Err(AuthorityError::Session(SessionError::BootSessionMismatch))
        );
    }

    #[test]
    fn logout_records_ticket_and_capability_revocation() {
        let mut identity = Identity::new();
        let boot = [4u8; 32];
        let session = identity
            .open_session(
                ROOT_PRINCIPAL_ID,
                SessionOrigin::Local,
                AuthenticationMethod::Password,
                boot,
                0,
                1,
            )
            .unwrap();
        identity
            .issue_mod_ticket(session.session_id, ModOperation::FsRemove, boot, 0, 64)
            .unwrap();
        identity.close_session(session.session_id, 1).unwrap();
        let kinds: crate::platform::Vec<_> =
            identity.ledger().events().iter().map(|e| e.kind).collect();
        assert!(kinds.contains(&IdentityEventKind::ModTicketRevoked));
        assert!(kinds.contains(&IdentityEventKind::CapabilityRevoked));
        assert!(kinds.contains(&IdentityEventKind::SessionClosed));
        assert!(identity.tickets().active().is_empty());
    }
}
