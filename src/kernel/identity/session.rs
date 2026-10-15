//! User sessions (`kernel::identity`, Gate 7) — `UserSession`,
//! `SecurityContextId` per process, `AuthenticationMethod`, bound to the
//! current `BootSessionRoot`.

use crate::kernel::crypto::phase_hash::ph_identity_session;
use crate::kernel::hash::StateHash;
use crate::kernel::identity::principal::{PrincipalId, SessionOrigin};
use crate::platform::BTreeMap;

pub const MAX_SESSIONS: usize = 64;

pub type SessionId = u32;
pub type SecurityContextId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthenticationMethod {
    Password,
    Certificate,
}

impl AuthenticationMethod {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Password => 0,
            Self::Certificate => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserSession {
    pub session_id: SessionId,
    pub principal_id: PrincipalId,
    pub origin: SessionOrigin,
    pub method: AuthenticationMethod,
    pub boot_session_root: StateHash,
    pub opened_tick: u64,
    pub session_hash: StateHash,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    TableFull,
    NotFound,
    Inactive,
    BootSessionMismatch,
    PrincipalDisabled,
    RemoteRootDenied,
}

fn origin_byte(origin: SessionOrigin) -> u8 {
    match origin {
        SessionOrigin::Local => 0,
        SessionOrigin::Remote(_) => 1,
    }
}

impl UserSession {
    pub fn bind(
        session_id: SessionId,
        principal_id: PrincipalId,
        origin: SessionOrigin,
        method: AuthenticationMethod,
        boot_session_root: StateHash,
        opened_tick: u64,
    ) -> Self {
        let session_hash = ph_identity_session(
            principal_id,
            origin_byte(origin),
            method.as_u8(),
            &boot_session_root,
            opened_tick,
        );
        Self {
            session_id,
            principal_id,
            origin,
            method,
            boot_session_root,
            opened_tick,
            session_hash,
            active: true,
        }
    }
}

/// Bounded session table plus per-process security-context map.
#[derive(Debug, Clone, Default)]
pub struct SessionTable {
    sessions: BTreeMap<SessionId, UserSession>,
    next_id: SessionId,
    /// Process id → live session id (the process's `SecurityContextId`).
    contexts: BTreeMap<u32, SessionId>,
}

impl SessionTable {
    pub fn new() -> Self {
        Self {
            sessions: BTreeMap::new(),
            next_id: 1,
            contexts: BTreeMap::new(),
        }
    }

    pub fn get(&self, id: SessionId) -> Result<UserSession, SessionError> {
        self.sessions.get(&id).copied().ok_or(SessionError::NotFound)
    }

    pub fn context_of(&self, process_id: u32) -> Option<SecurityContextId> {
        self.contexts.get(&process_id).copied()
    }

    pub fn session_for_process(&self, process_id: u32) -> Option<UserSession> {
        let sid = self.context_of(process_id)?;
        self.sessions.get(&sid).copied().filter(|s| s.active)
    }

    pub fn open(
        &mut self,
        principal_id: PrincipalId,
        origin: SessionOrigin,
        method: AuthenticationMethod,
        boot_session_root: StateHash,
        opened_tick: u64,
        process_id: u32,
    ) -> Result<UserSession, SessionError> {
        if self.sessions.len() >= MAX_SESSIONS {
            return Err(SessionError::TableFull);
        }
        let session_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let session = UserSession::bind(
            session_id,
            principal_id,
            origin,
            method,
            boot_session_root,
            opened_tick,
        );
        self.sessions.insert(session_id, session);
        self.contexts.insert(process_id, session_id);
        Ok(session)
    }

    pub fn close(&mut self, id: SessionId) -> Result<UserSession, SessionError> {
        let session = self
            .sessions
            .get_mut(&id)
            .ok_or(SessionError::NotFound)?;
        session.active = false;
        self.contexts.retain(|_, sid| *sid != id);
        Ok(*session)
    }

    /// Confirm a live session is still bound to `expected_boot_session_root`.
    pub fn require_bound(
        &self,
        id: SessionId,
        expected_boot_session_root: StateHash,
    ) -> Result<UserSession, SessionError> {
        let session = self.get(id)?;
        if !session.active {
            return Err(SessionError::Inactive);
        }
        if session.boot_session_root != expected_boot_session_root {
            return Err(SessionError::BootSessionMismatch);
        }
        Ok(session)
    }

    pub fn revoke_principal(&mut self, principal_id: PrincipalId) {
        let ids: crate::platform::Vec<SessionId> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.principal_id == principal_id && s.active)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let _ = self.close(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_binds_session_hash_to_boot_session_root() {
        let mut table = SessionTable::new();
        let boot = [7u8; 32];
        let s = table
            .open(0, SessionOrigin::Local, AuthenticationMethod::Password, boot, 10, 1)
            .unwrap();
        assert!(s.active);
        assert_eq!(s.boot_session_root, boot);
        assert_ne!(s.session_hash, [0u8; 32]);
        assert_eq!(table.context_of(1), Some(s.session_id));
        assert_eq!(table.require_bound(s.session_id, boot).unwrap().session_id, s.session_id);
        assert_eq!(
            table.require_bound(s.session_id, [8u8; 32]),
            Err(SessionError::BootSessionMismatch)
        );
    }

    #[test]
    fn close_deactivates_and_drops_process_context() {
        let mut table = SessionTable::new();
        let s = table
            .open(0, SessionOrigin::Local, AuthenticationMethod::Password, [1u8; 32], 0, 1)
            .unwrap();
        table.close(s.session_id).unwrap();
        assert!(!table.get(s.session_id).unwrap().active);
        assert_eq!(table.context_of(1), None);
        assert_eq!(
            table.require_bound(s.session_id, [1u8; 32]),
            Err(SessionError::Inactive)
        );
    }
}
