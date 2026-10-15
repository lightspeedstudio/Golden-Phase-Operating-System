//! `q_t = (s_t, m_t, v_t)` and projection `π_S(s,m,v) = s`.
//!
//! `s_t` is [`crate::core::gphase_core::PhaseState`], not
//! `fault_syndrome::PhaseState`. `v_t` is the full captable `Rights` `u32`
//! bitset, not `tensor_state::CapabilityClass`.

use crate::core::gphase_core::PhaseState;

/// Same width as `kernel::captable::Rights`. Defined here so PhaseSentinel
/// does not Cargo-depend on `captable`.
pub type Rights = u32;

pub const RIGHT_READ: Rights = 1 << 0;
pub const RIGHT_WRITE: Rights = 1 << 1;
pub const RIGHT_DELETE: Rights = 1 << 2;
pub const RIGHT_GRANT: Rights = 1 << 3;

/// `m_t`: `sched2.elapsed()` when that feature is on, else the Sentinel-local
/// epoch. Not a second `StateClock`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerSnapshot {
    pub epoch: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SentinelState {
    pub s_t: PhaseState,
    pub m_t: SchedulerSnapshot,
    pub v_t: Rights,
}

/// `π_S(s, m, v) = s`.
pub fn project_s(q: &SentinelState) -> PhaseState {
    q.s_t
}

pub fn rights_subset(requested: Rights, granted: Rights) -> bool {
    requested & granted == requested
}
