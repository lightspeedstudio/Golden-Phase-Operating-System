//! Identity-scoped capabilities (`kernel::identity`, Gate 8) — distinct
//! from `kernel::captable` object handles. These gate `ModOperation` verbs
//! bound to `(PrincipalId, SessionId)`, not raw filesystem objects.

use crate::kernel::crypto::phase_hash::ph_identity_capability;
use crate::kernel::hash::StateHash;
use crate::kernel::identity::mod_ticket::ModOperation;
use crate::kernel::identity::principal::PrincipalId;
use crate::kernel::identity::session::SessionId;
use crate::platform::BTreeMap;

pub const MAX_IDENTITY_CAPS: usize = 128;

pub type IdentityCapId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentityCapability {
    pub cap_id: IdentityCapId,
    pub principal_id: PrincipalId,
    pub session_id: SessionId,
    pub op: ModOperation,
    pub generation: u64,
    pub proof: StateHash,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityCapError {
    TableFull,
    NotFound,
    Inactive,
    OperationMismatch,
}

impl IdentityCapability {
    pub fn issue(
        cap_id: IdentityCapId,
        principal_id: PrincipalId,
        session_id: SessionId,
        op: ModOperation,
        generation: u64,
    ) -> Self {
        let proof = ph_identity_capability(principal_id, session_id, op.as_u8(), generation);
        Self {
            cap_id,
            principal_id,
            session_id,
            op,
            generation,
            proof,
            active: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct IdentityCapTable {
    caps: BTreeMap<IdentityCapId, IdentityCapability>,
    next_id: IdentityCapId,
}

impl IdentityCapTable {
    pub fn new() -> Self {
        Self {
            caps: BTreeMap::new(),
            next_id: 1,
        }
    }

    pub fn grant(
        &mut self,
        principal_id: PrincipalId,
        session_id: SessionId,
        op: ModOperation,
        generation: u64,
    ) -> Result<IdentityCapability, IdentityCapError> {
        if self.caps.len() >= MAX_IDENTITY_CAPS {
            return Err(IdentityCapError::TableFull);
        }
        let cap_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let cap = IdentityCapability::issue(cap_id, principal_id, session_id, op, generation);
        self.caps.insert(cap_id, cap);
        Ok(cap)
    }

    pub fn get(&self, id: IdentityCapId) -> Result<IdentityCapability, IdentityCapError> {
        self.caps.get(&id).copied().ok_or(IdentityCapError::NotFound)
    }

    pub fn authorize(
        &self,
        id: IdentityCapId,
        op: ModOperation,
    ) -> Result<IdentityCapability, IdentityCapError> {
        let cap = self.get(id)?;
        if !cap.active {
            return Err(IdentityCapError::Inactive);
        }
        if cap.op != op {
            return Err(IdentityCapError::OperationMismatch);
        }
        Ok(cap)
    }

    pub fn revoke(&mut self, id: IdentityCapId) -> Result<(), IdentityCapError> {
        let cap = self.caps.get_mut(&id).ok_or(IdentityCapError::NotFound)?;
        cap.active = false;
        Ok(())
    }

    pub fn revoke_session(&mut self, session_id: SessionId) {
        for cap in self.caps.values_mut() {
            if cap.session_id == session_id {
                cap.active = false;
            }
        }
    }

    pub fn revoke_principal(&mut self, principal_id: PrincipalId) {
        for cap in self.caps.values_mut() {
            if cap.principal_id == principal_id {
                cap.active = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_and_authorize_round_trip() {
        let mut table = IdentityCapTable::new();
        let cap = table
            .grant(0, 1, ModOperation::PrincipalDisable, 0)
            .unwrap();
        assert_ne!(cap.proof, [0u8; 32]);
        assert!(table.authorize(cap.cap_id, ModOperation::PrincipalDisable).is_ok());
        assert_eq!(
            table.authorize(cap.cap_id, ModOperation::FsRemove),
            Err(IdentityCapError::OperationMismatch)
        );
        table.revoke(cap.cap_id).unwrap();
        assert_eq!(
            table.authorize(cap.cap_id, ModOperation::PrincipalDisable),
            Err(IdentityCapError::Inactive)
        );
    }
}
