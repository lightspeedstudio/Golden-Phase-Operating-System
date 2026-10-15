//! Bounded observation events. Not per-instruction.

use crate::core::gphase_core::PhaseState;
use crate::kernel::event_log::ProcessId;
use crate::kernel::hash::StateHash;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Spawn,
    Exit,
    Fs,
    Ipc,
    PhaseNet,
    Launch,
    Syscall,
}

impl EventKind {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Spawn => 0,
            Self::Exit => 1,
            Self::Fs => 2,
            Self::Ipc => 3,
            Self::PhaseNet => 4,
            Self::Launch => 5,
            Self::Syscall => 6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SentinelEvent {
    pub kind: EventKind,
    pub pid: ProcessId,
    pub nr: u32,
    pub from: Option<PhaseState>,
    pub to: Option<PhaseState>,
    pub content: Option<StateHash>,
}

impl SentinelEvent {
    pub fn syscall(pid: ProcessId, nr: u32) -> Self {
        Self {
            kind: EventKind::Syscall,
            pid,
            nr,
            from: None,
            to: None,
            content: None,
        }
    }
}
