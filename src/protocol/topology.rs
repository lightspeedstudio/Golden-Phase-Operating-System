//! Stream topology scanning over phase-state sequences.

use crate::core::gphase_core::{topology_status, PhaseState, TopologyKind};
use crate::platform::Vec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamTopologyEvent {
    pub index: usize,
    pub triple: (PhaseState, PhaseState, PhaseState),
    pub kind: TopologyKind,
}

pub fn scan_non_overlapping_triples(states: &[PhaseState]) -> Vec<StreamTopologyEvent> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 3 <= states.len() {
        let triple = (states[i], states[i + 1], states[i + 2]);
        let result = topology_status(triple.0, triple.1, triple.2);
        out.push(StreamTopologyEvent {
            index: i,
            triple,
            kind: result.kind,
        });
        i += 3;
    }
    out
}

pub fn scan_sliding_triples(states: &[PhaseState]) -> Vec<StreamTopologyEvent> {
    let mut out = Vec::new();
    if states.len() < 3 {
        return out;
    }
    for i in 0..=states.len() - 3 {
        let triple = (states[i], states[i + 1], states[i + 2]);
        let result = topology_status(triple.0, triple.1, triple.2);
        out.push(StreamTopologyEvent {
            index: i,
            triple,
            kind: result.kind,
        });
    }
    out
}

pub fn reflection_count(states: &[PhaseState]) -> usize {
    scan_sliding_triples(states)
        .into_iter()
        .filter(|e| e.kind == TopologyKind::Reflection)
        .count()
}
