//! Process model: phase-state trajectory, legal transitions, topology events.

use crate::core::gphase_core::{topology_status, PhaseState, TopologyKind};
use crate::kernel::event_log::{ProcessId, TopologyEvent, TransitionViolation};
use crate::platform::{HashSet, String, Vec};

pub const VALID_TRANSITIONS: [(PhaseState, &[PhaseState]); 4] = [
    (PhaseState::Phi0, &[PhaseState::Phi1]),
    (PhaseState::Phi1, &[PhaseState::Phi0, PhaseState::Phi2]),
    (PhaseState::Phi2, &[PhaseState::Phi1, PhaseState::Phi3]),
    (PhaseState::Phi3, &[PhaseState::Phi0, PhaseState::Phi1]),
];

fn allowed_targets(from: PhaseState) -> &'static [PhaseState] {
    VALID_TRANSITIONS
        .iter()
        .find(|(s, _)| *s == from)
        .map(|(_, targets)| *targets)
        .expect("every state has a transition row")
}

pub struct Process {
    pub pid: ProcessId,
    pub name: String,
    pub state: PhaseState,
    pub history: Vec<PhaseState>,
    pub topology_events: Vec<TopologyEvent>,
    pub violations: Vec<TransitionViolation>,
}

impl Process {
    pub fn new(pid: ProcessId, state: PhaseState) -> Self {
        Self::with_name(pid, state, String::new())
    }

    pub fn with_name(pid: ProcessId, state: PhaseState, name: impl Into<String>) -> Self {
        let name = name.into();
        let name = if name.is_empty() {
            format!("proc{pid}")
        } else {
            name
        };
        Self {
            pid,
            name,
            state,
            history: vec![state],
            topology_events: Vec::new(),
            violations: Vec::new(),
        }
    }

    pub fn transition(&mut self, new_state: PhaseState) -> bool {
        let allowed: HashSet<PhaseState> = allowed_targets(self.state).iter().copied().collect();
        if !allowed.contains(&new_state) {
            self.violations.push(TransitionViolation {
                pid: self.pid,
                from: self.state,
                to: new_state,
            });
            return false;
        }

        self.state = new_state;
        self.history.push(new_state);
        self.check_topology();
        true
    }

    fn check_topology(&mut self) {
        if self.history.len() < 3 {
            return;
        }
        let n = self.history.len();
        let a = self.history[n - 3];
        let b = self.history[n - 2];
        let c = self.history[n - 1];
        let result = topology_status(a, b, c);
        if result.kind == TopologyKind::Reflection {
            self.topology_events.push(TopologyEvent {
                pid: self.pid,
                triple: (a, b, c),
                kind: result.kind,
                left: result.left,
                right: result.right,
            });
        }
    }
}
