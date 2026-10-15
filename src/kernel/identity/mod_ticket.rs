//! `ModTicket` / `ModOperation` (`kernel::identity`, Gate 8).
//!
//! A ticket is a time-bounded elevation for one principal+session to
//! perform a structured `mod` verb. There is no `mod shell`.

use crate::kernel::identity::principal::PrincipalId;
use crate::kernel::identity::session::SessionId;
use crate::platform::BTreeMap;

pub const MAX_TICKETS: usize = 64;

pub type TicketId = u32;

/// Structured `mod` verbs. New variants are appended; never renumbered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModOperation {
    PrincipalCreate,
    PrincipalDisable,
    PrincipalEnable,
    CertificateIssue,
    CertificateRevoke,
    CredentialLock,
    CredentialUnlock,
    FsRemove,
    SessionClose,
    #[cfg(feature = "phasestack")]
    AppInstall,
    #[cfg(feature = "phasestack")]
    DeveloperCertRevoke,
    #[cfg(feature = "phasesentinel")]
    SentinelCertInstall,
    #[cfg(feature = "phasesentinel")]
    SentinelCertRevoke,
    #[cfg(feature = "phasesentinel")]
    SentinelPolicySet,
    #[cfg(feature = "phasesentinel")]
    SentinelRecover,
}

impl ModOperation {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::PrincipalCreate => 0,
            Self::PrincipalDisable => 1,
            Self::PrincipalEnable => 2,
            Self::CertificateIssue => 3,
            Self::CertificateRevoke => 4,
            Self::CredentialLock => 5,
            Self::CredentialUnlock => 6,
            Self::FsRemove => 7,
            Self::SessionClose => 8,
            #[cfg(feature = "phasestack")]
            Self::AppInstall => 9,
            #[cfg(feature = "phasestack")]
            Self::DeveloperCertRevoke => 10,
            #[cfg(feature = "phasesentinel")]
            Self::SentinelCertInstall => 11,
            #[cfg(feature = "phasesentinel")]
            Self::SentinelCertRevoke => 12,
            #[cfg(feature = "phasesentinel")]
            Self::SentinelPolicySet => 13,
            #[cfg(feature = "phasesentinel")]
            Self::SentinelRecover => 14,
        }
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::PrincipalCreate),
            1 => Some(Self::PrincipalDisable),
            2 => Some(Self::PrincipalEnable),
            3 => Some(Self::CertificateIssue),
            4 => Some(Self::CertificateRevoke),
            5 => Some(Self::CredentialLock),
            6 => Some(Self::CredentialUnlock),
            7 => Some(Self::FsRemove),
            8 => Some(Self::SessionClose),
            #[cfg(feature = "phasestack")]
            9 => Some(Self::AppInstall),
            #[cfg(feature = "phasestack")]
            10 => Some(Self::DeveloperCertRevoke),
            #[cfg(feature = "phasesentinel")]
            11 => Some(Self::SentinelCertInstall),
            #[cfg(feature = "phasesentinel")]
            12 => Some(Self::SentinelCertRevoke),
            #[cfg(feature = "phasesentinel")]
            13 => Some(Self::SentinelPolicySet),
            #[cfg(feature = "phasesentinel")]
            14 => Some(Self::SentinelRecover),
            _ => None,
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "principal.create" => Some(Self::PrincipalCreate),
            "principal.disable" => Some(Self::PrincipalDisable),
            "principal.enable" => Some(Self::PrincipalEnable),
            "certificate.issue" => Some(Self::CertificateIssue),
            "certificate.revoke" => Some(Self::CertificateRevoke),
            "credential.lock" => Some(Self::CredentialLock),
            "credential.unlock" => Some(Self::CredentialUnlock),
            "fs.rm" => Some(Self::FsRemove),
            "session.close" => Some(Self::SessionClose),
            #[cfg(feature = "phasestack")]
            "app.install" => Some(Self::AppInstall),
            #[cfg(feature = "phasestack")]
            "cert.developer-revoke" => Some(Self::DeveloperCertRevoke),
            #[cfg(feature = "phasesentinel")]
            "sentinel.cert.install" => Some(Self::SentinelCertInstall),
            #[cfg(feature = "phasesentinel")]
            "sentinel.cert.revoke" => Some(Self::SentinelCertRevoke),
            #[cfg(feature = "phasesentinel")]
            "sentinel.policy.set" => Some(Self::SentinelPolicySet),
            #[cfg(feature = "phasesentinel")]
            "sentinel.recover" => Some(Self::SentinelRecover),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::PrincipalCreate => "principal.create",
            Self::PrincipalDisable => "principal.disable",
            Self::PrincipalEnable => "principal.enable",
            Self::CertificateIssue => "certificate.issue",
            Self::CertificateRevoke => "certificate.revoke",
            Self::CredentialLock => "credential.lock",
            Self::CredentialUnlock => "credential.unlock",
            Self::FsRemove => "fs.rm",
            Self::SessionClose => "session.close",
            #[cfg(feature = "phasestack")]
            Self::AppInstall => "app.install",
            #[cfg(feature = "phasestack")]
            Self::DeveloperCertRevoke => "cert.developer-revoke",
            #[cfg(feature = "phasesentinel")]
            Self::SentinelCertInstall => "sentinel.cert.install",
            #[cfg(feature = "phasesentinel")]
            Self::SentinelCertRevoke => "sentinel.cert.revoke",
            #[cfg(feature = "phasesentinel")]
            Self::SentinelPolicySet => "sentinel.policy.set",
            #[cfg(feature = "phasesentinel")]
            Self::SentinelRecover => "sentinel.recover",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModTicket {
    pub ticket_id: TicketId,
    pub principal_id: PrincipalId,
    pub session_id: SessionId,
    pub cap_id: u32,
    pub op: ModOperation,
    pub expires_tick: u64,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TicketError {
    TableFull,
    NotFound,
    Inactive,
    Expired,
    OperationMismatch,
}

#[derive(Debug, Clone, Default)]
pub struct TicketTable {
    tickets: BTreeMap<TicketId, ModTicket>,
    next_id: TicketId,
}

impl TicketTable {
    pub fn new() -> Self {
        Self {
            tickets: BTreeMap::new(),
            next_id: 1,
        }
    }

    pub fn issue(
        &mut self,
        principal_id: PrincipalId,
        session_id: SessionId,
        cap_id: u32,
        op: ModOperation,
        expires_tick: u64,
    ) -> Result<ModTicket, TicketError> {
        if self.tickets.len() >= MAX_TICKETS {
            return Err(TicketError::TableFull);
        }
        let ticket_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let ticket = ModTicket {
            ticket_id,
            principal_id,
            session_id,
            cap_id,
            op,
            expires_tick,
            active: true,
        };
        self.tickets.insert(ticket_id, ticket);
        Ok(ticket)
    }

    pub fn get(&self, id: TicketId) -> Result<ModTicket, TicketError> {
        self.tickets.get(&id).copied().ok_or(TicketError::NotFound)
    }

    pub fn require(
        &self,
        id: TicketId,
        op: ModOperation,
        now_tick: u64,
    ) -> Result<ModTicket, TicketError> {
        let ticket = self.get(id)?;
        if !ticket.active {
            return Err(TicketError::Inactive);
        }
        if now_tick > ticket.expires_tick {
            return Err(TicketError::Expired);
        }
        if ticket.op != op {
            return Err(TicketError::OperationMismatch);
        }
        Ok(ticket)
    }

    pub fn revoke(&mut self, id: TicketId) -> Result<(), TicketError> {
        let ticket = self.tickets.get_mut(&id).ok_or(TicketError::NotFound)?;
        ticket.active = false;
        Ok(())
    }

    pub fn revoke_session(&mut self, session_id: SessionId) {
        for t in self.tickets.values_mut() {
            if t.session_id == session_id {
                t.active = false;
            }
        }
    }

    pub fn revoke_principal(&mut self, principal_id: PrincipalId) {
        for t in self.tickets.values_mut() {
            if t.principal_id == principal_id {
                t.active = false;
            }
        }
    }

    pub fn active(&self) -> crate::platform::Vec<ModTicket> {
        self.tickets
            .values()
            .copied()
            .filter(|t| t.active)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rejects_mod_shell_and_unknown_verbs() {
        assert_eq!(ModOperation::parse("shell"), None);
        assert_eq!(ModOperation::parse("mod shell"), None);
        assert_eq!(
            ModOperation::parse("principal.disable"),
            Some(ModOperation::PrincipalDisable)
        );
    }

    #[test]
    fn expired_ticket_is_rejected() {
        let mut table = TicketTable::new();
        let t = table.issue(0, 1, 1, ModOperation::FsRemove, 10).unwrap();
        assert_eq!(
            table.require(t.ticket_id, ModOperation::FsRemove, 11),
            Err(TicketError::Expired)
        );
        assert!(table.require(t.ticket_id, ModOperation::FsRemove, 10).is_ok());
    }
}
