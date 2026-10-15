//! ANOM mode and status types.

use crate::kernel::authority::StateHash;
use crate::kernel::authority::{AuthorityMode, VisibilityMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnomStatus {
    Disabled,
    Available,
    Armed,
    Active,
    VerificationFailed,
    HostedOnly,
    BaremetalStub,
}

#[derive(Debug, Clone)]
pub struct AnomSession {
    pub session_id: u64,
    pub boot_session_root: StateHash,
    pub equation_root: StateHash,
    pub started_sequence: u64,
    pub authority: AuthorityMode,
    pub visibility: VisibilityMode,
    pub key_available: bool,
    pub current_anom_root: StateHash,
    pub enter_event_hash: StateHash,
    pub last_anom_event_hash: StateHash,
}
