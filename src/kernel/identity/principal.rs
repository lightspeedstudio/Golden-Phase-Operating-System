//! Principal types (`kernel::identity`, Gate 1) — the "who" the rest of the
//! identity subsystem (certificates, credentials, sessions, `mod` tickets)
//! is bound to. See module docs on `kernel::identity` for the full trust
//! chain and per-gate scope.
//!
//! ## Non-goals (Gate 1)
//! - No certificates, passwords, sessions, or capabilities live here yet —
//!   those are Gates 2-8. A [`PrincipalRecord`] is only ever `(kind,
//!   enabled)`; it proves nothing about credentials on its own.
//! - No revocation cascade: disabling a principal here does not yet revoke
//!   any session/capability (there are none to revoke until later gates).

use crate::platform::BTreeMap;

/// Hard bound on the number of principals the kernel will track — matches
/// the "keep all tables ... bounded" platform requirement. Generous enough
/// for a real deployment's Root/Mod/Operator/Agent/Service/Node/Guest
/// roster without unbounded growth.
pub const MAX_PRINCIPALS: usize = 256;

/// Stable numeric identity of a principal. `0` is reserved for the Root
/// principal (see [`ROOT_PRINCIPAL_ID`]); all other IDs are assigned
/// monotonically by [`PrincipalTable::create`].
pub type PrincipalId = u32;

/// The Root principal's fixed, well-known ID. Root is created exactly once,
/// implicitly, the first time a [`PrincipalTable`] is constructed — it is
/// never allocated via [`PrincipalTable::create`] and never disabled via
/// [`PrincipalTable::disable`] (see that function's doc).
pub const ROOT_PRINCIPAL_ID: PrincipalId = 0;

/// A PhaseNet-authenticated remote machine identity, used by
/// [`SessionOrigin::Remote`]. Distinct from [`PrincipalId`]: a `NodeId`
/// names the network peer a session arrived over, not the principal
/// authenticating within it.
pub type NodeId = u32;

/// The seven principal kinds this subsystem recognizes. See the platform
/// spec (`docs/...` / plan) for the authoritative meaning of each; summary:
/// - [`Root`](PrincipalKind::Root): highest-authority local account, bound
///   to the mathematical/kernel [`crate::kernel::identity::authority_root`].
/// - [`Mod`](PrincipalKind::Mod): administrator who elevates via `mod`.
/// - [`Operator`](PrincipalKind::Operator): ordinary human user.
/// - [`Agent`](PrincipalKind::Agent): AI/autonomous software identity.
/// - [`Service`](PrincipalKind::Service): background system service.
/// - [`Node`](PrincipalKind::Node): authenticated PhaseNet machine.
/// - [`Guest`](PrincipalKind::Guest): restricted temporary account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrincipalKind {
    Root,
    Mod,
    Operator,
    Agent,
    Service,
    Node,
    Guest,
}

impl PrincipalKind {
    /// Stable, hash-relevant byte encoding — used by later gates' canonical
    /// certificate/ledger encodings. Values are part of the wire format
    /// once persisted (Gate 6); never renumber an existing variant.
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Root => 0,
            Self::Mod => 1,
            Self::Operator => 2,
            Self::Agent => 3,
            Self::Service => 4,
            Self::Node => 5,
            Self::Guest => 6,
        }
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Root),
            1 => Some(Self::Mod),
            2 => Some(Self::Operator),
            3 => Some(Self::Agent),
            4 => Some(Self::Service),
            5 => Some(Self::Node),
            6 => Some(Self::Guest),
            _ => None,
        }
    }

    /// Whether this kind is a human principal that may hold an interactive
    /// password credential per the platform spec (`Root`, `Mod`,
    /// `Operator`, optionally `Guest`). `Agent`/`Service`/`Node` must use
    /// certificates or other machine credentials instead (enforced in
    /// Gate 3, not here — this is the classification the later enforcement
    /// reads).
    pub fn may_hold_password(self) -> bool {
        matches!(self, Self::Root | Self::Mod | Self::Operator | Self::Guest)
    }
}

/// Where an authenticated session's connection originated. `Host` and
/// `Local` are deliberately not conflated: [`SessionOrigin`] tracks
/// connection origin; [`AuthorityScope`] (below) tracks authority scope —
/// the platform spec requires these stay separate concepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionOrigin {
    Local,
    Remote(NodeId),
}

/// The scope an authority decision or capability applies to. Distinct from
/// [`SessionOrigin`]: `Host`/`Local` are not user roles and must never be
/// conflated with a [`PrincipalKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthorityScope {
    Object,
    Host,
    Cluster,
}

/// A tracked principal's persistent record. Gate 1 scope only: certificate
/// and credential state are added by later gates on top of this same
/// `PrincipalId`, not folded into this struct, so this table stays a single
/// source of truth for "does this principal exist and is it enabled"
/// without growing into a duplicate of the certificate/credential tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrincipalRecord {
    pub kind: PrincipalKind,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrincipalError {
    /// [`PrincipalTable::create`] was called with every slot already used
    /// (see [`MAX_PRINCIPALS`]).
    TableFull,
    /// No principal exists with the given [`PrincipalId`].
    NotFound,
    /// [`PrincipalTable::disable`] or `enable` was called on
    /// [`ROOT_PRINCIPAL_ID`] — Root cannot be disabled through this table
    /// (see that function's doc for the rationale).
    RootCannotBeDisabled,
}

/// Bounded principal directory. Root ([`ROOT_PRINCIPAL_ID`]) always exists
/// from construction; every other principal is created explicitly through
/// [`PrincipalTable::create`].
#[derive(Debug, Clone)]
pub struct PrincipalTable {
    principals: BTreeMap<PrincipalId, PrincipalRecord>,
    next_id: PrincipalId,
}

impl Default for PrincipalTable {
    fn default() -> Self {
        Self::new()
    }
}

impl PrincipalTable {
    pub fn new() -> Self {
        let mut principals = BTreeMap::new();
        // Root always exists structurally (the principal *record*), even
        // though its *credential* remains `Unprovisioned` until a later
        // gate's provisioning flow runs — the platform spec's own
        // distinction between "Root the principal" and "Root the
        // authenticated account" (see `RootCredentialState`, Gate 3).
        principals.insert(
            ROOT_PRINCIPAL_ID,
            PrincipalRecord {
                kind: PrincipalKind::Root,
                enabled: true,
            },
        );
        Self {
            principals,
            next_id: ROOT_PRINCIPAL_ID + 1,
        }
    }

    pub fn len(&self) -> usize {
        self.principals.len()
    }

    pub fn is_empty(&self) -> bool {
        false // Root always exists.
    }

    pub fn get(&self, id: PrincipalId) -> Result<PrincipalRecord, PrincipalError> {
        self.principals
            .get(&id)
            .copied()
            .ok_or(PrincipalError::NotFound)
    }

    /// Create a new non-Root principal. Bounded by [`MAX_PRINCIPALS`].
    pub fn create(&mut self, kind: PrincipalKind) -> Result<PrincipalId, PrincipalError> {
        if self.principals.len() >= MAX_PRINCIPALS {
            return Err(PrincipalError::TableFull);
        }
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).unwrap_or(self.next_id);
        self.principals.insert(
            id,
            PrincipalRecord {
                kind,
                enabled: true,
            },
        );
        Ok(id)
    }

    /// Disable a principal. Root is exempt: the platform spec requires Root
    /// remain a real, always-present highest-authority principal — its
    /// *credential* can be locked/require-recovery (Gate 3's
    /// `RootCredentialState`), but the principal record itself is never
    /// disabled through this generic path.
    pub fn disable(&mut self, id: PrincipalId) -> Result<(), PrincipalError> {
        if id == ROOT_PRINCIPAL_ID {
            return Err(PrincipalError::RootCannotBeDisabled);
        }
        let record = self
            .principals
            .get_mut(&id)
            .ok_or(PrincipalError::NotFound)?;
        record.enabled = false;
        Ok(())
    }

    pub fn enable(&mut self, id: PrincipalId) -> Result<(), PrincipalError> {
        if id == ROOT_PRINCIPAL_ID {
            return Err(PrincipalError::RootCannotBeDisabled);
        }
        let record = self
            .principals
            .get_mut(&id)
            .ok_or(PrincipalError::NotFound)?;
        record.enabled = true;
        Ok(())
    }

    pub fn next_id(&self) -> PrincipalId {
        self.next_id
    }

    /// Snapshot of every principal record, ordered by id. Used by Gate 6
    /// persistence; not a live iterator over interior mutability.
    pub fn snapshot(&self) -> crate::platform::Vec<(PrincipalId, PrincipalRecord)> {
        self.principals.iter().map(|(id, rec)| (*id, *rec)).collect()
    }

    /// Replace the table with a previously persisted snapshot. Root must be
    /// present. Does not record ledger events — this is restore, not a
    /// mutation (Gate 6).
    pub fn restore(
        &mut self,
        entries: crate::platform::Vec<(PrincipalId, PrincipalRecord)>,
        next_id: PrincipalId,
    ) -> Result<(), PrincipalError> {
        if !entries
            .iter()
            .any(|(id, rec)| *id == ROOT_PRINCIPAL_ID && rec.kind == PrincipalKind::Root)
        {
            return Err(PrincipalError::NotFound);
        }
        if entries.len() > MAX_PRINCIPALS {
            return Err(PrincipalError::TableFull);
        }
        let mut principals = crate::platform::BTreeMap::new();
        for (id, rec) in entries {
            principals.insert(id, rec);
        }
        self.principals = principals;
        self.next_id = next_id.max(ROOT_PRINCIPAL_ID + 1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_principal_exists_from_construction() {
        let table = PrincipalTable::new();
        let root = table.get(ROOT_PRINCIPAL_ID).unwrap();
        assert_eq!(root.kind, PrincipalKind::Root);
        assert!(root.enabled);
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn create_assigns_distinct_ids_starting_after_root() {
        let mut table = PrincipalTable::new();
        let a = table.create(PrincipalKind::Operator).unwrap();
        let b = table.create(PrincipalKind::Mod).unwrap();
        assert_ne!(a, b);
        assert_ne!(a, ROOT_PRINCIPAL_ID);
        assert_ne!(b, ROOT_PRINCIPAL_ID);
        assert_eq!(table.len(), 3);
    }

    #[test]
    fn table_full_is_a_hard_bound() {
        let mut table = PrincipalTable::new();
        // Root already occupies one slot.
        for _ in 0..(MAX_PRINCIPALS - 1) {
            table.create(PrincipalKind::Guest).unwrap();
        }
        assert_eq!(table.len(), MAX_PRINCIPALS);
        assert_eq!(
            table.create(PrincipalKind::Guest),
            Err(PrincipalError::TableFull)
        );
    }

    #[test]
    fn disable_and_enable_round_trip_for_a_non_root_principal() {
        let mut table = PrincipalTable::new();
        let op = table.create(PrincipalKind::Operator).unwrap();
        table.disable(op).unwrap();
        assert!(!table.get(op).unwrap().enabled);
        table.enable(op).unwrap();
        assert!(table.get(op).unwrap().enabled);
    }

    #[test]
    fn root_cannot_be_disabled_through_the_generic_path() {
        let mut table = PrincipalTable::new();
        assert_eq!(
            table.disable(ROOT_PRINCIPAL_ID),
            Err(PrincipalError::RootCannotBeDisabled)
        );
        assert!(table.get(ROOT_PRINCIPAL_ID).unwrap().enabled);
    }

    #[test]
    fn unknown_principal_is_not_found() {
        let table = PrincipalTable::new();
        assert_eq!(table.get(999), Err(PrincipalError::NotFound));
    }

    #[test]
    fn principal_kind_byte_encoding_round_trips() {
        for kind in [
            PrincipalKind::Root,
            PrincipalKind::Mod,
            PrincipalKind::Operator,
            PrincipalKind::Agent,
            PrincipalKind::Service,
            PrincipalKind::Node,
            PrincipalKind::Guest,
        ] {
            assert_eq!(PrincipalKind::from_u8(kind.as_u8()), Some(kind));
        }
        assert_eq!(PrincipalKind::from_u8(200), None);
    }

    #[test]
    fn only_human_kinds_may_hold_a_password() {
        assert!(PrincipalKind::Root.may_hold_password());
        assert!(PrincipalKind::Mod.may_hold_password());
        assert!(PrincipalKind::Operator.may_hold_password());
        assert!(PrincipalKind::Guest.may_hold_password());
        assert!(!PrincipalKind::Agent.may_hold_password());
        assert!(!PrincipalKind::Service.may_hold_password());
        assert!(!PrincipalKind::Node.may_hold_password());
    }
}
