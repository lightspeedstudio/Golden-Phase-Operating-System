//! Shared event records for process history and topology/reflection logs.

use crate::core::gphase_core::{PhaseState, TopologyKind};

pub type ProcessId = u32;
pub type PageId = u32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionViolation {
    pub pid: ProcessId,
    pub from: PhaseState,
    pub to: PhaseState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopologyEvent {
    pub pid: ProcessId,
    pub triple: (PhaseState, PhaseState, PhaseState),
    pub kind: TopologyKind,
    pub left: PhaseState,
    pub right: PhaseState,
}
