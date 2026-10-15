//! StateClock event kinds and records.

use crate::platform::String;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockEventKind {
    ChronosTick,
    KairosEvent,
    SectorCrossing,
    DilationChanged,
    SkewDetected,
    Resync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KairosEventKind {
    Boot,
    PhaseChange,
    Command,
    Spawn,
    Kill,
    Syscall,
    Operator,
    Other,
}

impl KairosEventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            KairosEventKind::Boot => "boot",
            KairosEventKind::PhaseChange => "phase_change",
            KairosEventKind::Command => "command",
            KairosEventKind::Spawn => "spawn",
            KairosEventKind::Kill => "kill",
            KairosEventKind::Syscall => "syscall",
            KairosEventKind::Operator => "operator",
            KairosEventKind::Other => "other",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClockEvent {
    pub kind: ClockEventKind,
    pub previous_chronos_tick: u64,
    pub new_chronos_tick: u64,
    pub previous_kairos_tick: u64,
    pub new_kairos_tick: u64,
    pub proper_tick: u64,
    pub state_tick: u64,
    pub phi: f64,
    pub sector_id: u8,
    pub epsilon: f64,
    pub x: f64,
    pub dilation_factor: f64,
    pub skew: i64,
    pub reason: String,
}
