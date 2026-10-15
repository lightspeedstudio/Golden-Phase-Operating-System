use kairos::core::gphase_core::{topology_status, PhaseState, TopologyKind};
use kairos::kernel::process::{Process, VALID_TRANSITIONS};

fn legal_edges() -> Vec<(PhaseState, PhaseState)> {
    let mut edges = Vec::new();
    for (from, targets) in VALID_TRANSITIONS {
        for &to in targets {
            edges.push((from, to));
        }
    }
    edges
}

fn illegal_edges() -> Vec<(PhaseState, PhaseState)> {
    let mut edges = Vec::new();
    for &from in &PhaseState::ALL {
        for &to in &PhaseState::ALL {
            let legal = VALID_TRANSITIONS
                .iter()
                .find(|(s, _)| *s == from)
                .map(|(_, t)| t.contains(&to))
                .unwrap_or(false);
            if !legal {
                edges.push((from, to));
            }
        }
    }
    edges
}

#[test]
fn legal_transitions_succeed() {
    for (from, to) in legal_edges() {
        let mut p = Process::new(0, from);
        assert!(p.transition(to), "legal {from:?} -> {to:?}");
        assert_eq!(p.state, to);
        assert_eq!(p.violations.len(), 0);
    }
}

#[test]
fn illegal_transitions_log_without_crash() {
    for (from, to) in illegal_edges() {
        let mut p = Process::new(7, from);
        assert!(!p.transition(to), "illegal {from:?} -> {to:?}");
        assert_eq!(p.state, from);
        assert_eq!(p.history, vec![from]);
        assert_eq!(p.violations.len(), 1);
        assert_eq!(p.violations[0].pid, 7);
        assert_eq!(p.violations[0].from, from);
        assert_eq!(p.violations[0].to, to);
    }
}

#[test]
fn reflection_event_logged_for_order_sensitive_triple() {
    let mut p = Process::new(9, PhaseState::Phi0);
    assert!(p.transition(PhaseState::Phi1));
    assert!(p.transition(PhaseState::Phi2));
    assert!(p.transition(PhaseState::Phi3));

    assert_eq!(p.topology_events.len(), 1);
    let event = &p.topology_events[0];
    assert_eq!(event.pid, 9);
    assert_eq!(
        event.triple,
        (PhaseState::Phi1, PhaseState::Phi2, PhaseState::Phi3)
    );
    assert_eq!(event.kind, TopologyKind::Reflection);
    let expected = topology_status(PhaseState::Phi1, PhaseState::Phi2, PhaseState::Phi3);
    assert_eq!(event.left, expected.left);
    assert_eq!(event.right, expected.right);
}

#[test]
fn no_topology_event_before_three_states() {
    let mut p = Process::new(1, PhaseState::Phi0);
    assert!(p.transition(PhaseState::Phi1));
    assert!(p.topology_events.is_empty());
}
