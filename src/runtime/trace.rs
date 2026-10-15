//! Runtime trace log — timer, schedule, VM, and topology events.

use crate::core::gphase_core::TopologyKind;
use crate::kernel::event_log::{ProcessId, TopologyEvent};
use crate::platform::Vec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceEvent {
    Boot { init_pid: ProcessId },
    TimerTick { tick: u64 },
    Preempt { pid: ProcessId },
    Schedule { pid: ProcessId },
    VmCycles { pid: ProcessId, cycles: usize },
    Topology(TopologyEvent),
}

#[derive(Debug, Clone, Default)]
pub struct TraceLog {
    pub events: Vec<TraceEvent>,
}

impl TraceLog {
    pub fn push(&mut self, event: TraceEvent) {
        self.events.push(event);
    }

    pub fn reflections(&self) -> Vec<&TopologyEvent> {
        self.events
            .iter()
            .filter_map(|e| match e {
                TraceEvent::Topology(t) if t.kind == TopologyKind::Reflection => Some(t),
                _ => None,
            })
            .collect()
    }
}
