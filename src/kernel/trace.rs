//! Kernel event trace — boot, commands, spawn/kill, errors, phase changes.

use crate::core::gphase_core::PhaseState;
use crate::kernel::event_log::ProcessId;
use crate::platform::{String, Vec};

pub const TRACE_CAPACITY: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceRecord {
    Boot {
        init_pid: ProcessId,
    },
    Command {
        line: String,
    },
    Spawn {
        pid: ProcessId,
        name: String,
    },
    Kill {
        pid: ProcessId,
        ok: bool,
    },
    Error {
        message: String,
    },
    PhaseChange {
        pid: ProcessId,
        from: PhaseState,
        to: PhaseState,
    },
}

#[derive(Debug, Clone, Default)]
pub struct KernelTrace {
    pub events: Vec<TraceRecord>,
}

impl KernelTrace {
    pub fn record(&mut self, event: TraceRecord) {
        if self.events.len() >= TRACE_CAPACITY {
            self.events.remove(0);
        }
        self.events.push(event);
    }

    pub fn recent(&self) -> &[TraceRecord] {
        &self.events
    }
}
