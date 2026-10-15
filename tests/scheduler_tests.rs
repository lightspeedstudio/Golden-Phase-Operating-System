use kairos::core::gphase_core::TopologyKind;
use kairos::core::gphase_core::{compose, PhaseState};
use kairos::Kernel;

#[test]
fn spawn_starts_sleeping() {
    let mut k = Kernel::new();
    let pid = k.spawn();
    assert_eq!(k.get_process(pid).unwrap().state, PhaseState::Phi0);
}

#[test]
fn scheduler_primitives_follow_legal_graph() {
    let mut k = Kernel::new();
    let pid = k.spawn();
    assert!(k.wake(pid).unwrap());
    assert!(k.schedule(pid).unwrap());
    assert!(k.preempt(pid).unwrap());
    assert!(k.schedule(pid).unwrap());
    assert!(k.syscall(pid).unwrap());
    assert!(k.kreturn(pid).unwrap());
    assert!(k.schedule(pid).unwrap());
    assert!(k.syscall(pid).unwrap());
    assert!(k.exit(pid).unwrap());
    assert_eq!(k.get_process(pid).unwrap().state, PhaseState::Phi0);
}

#[test]
fn wrong_state_transition_fails_and_logs() {
    let mut k = Kernel::new();
    let pid = k.spawn();
    assert!(!k.schedule(pid).unwrap());
    assert_eq!(k.get_process(pid).unwrap().violations.len(), 1);
}

#[test]
fn contention_escalates_two_running_to_phi3() {
    let mut k = Kernel::new();
    let a = k.spawn();
    let b = k.spawn();
    for pid in [a, b] {
        assert!(k.wake(pid).unwrap());
        assert!(k.schedule(pid).unwrap());
    }
    assert_eq!(k.contention_check(a, b).unwrap(), PhaseState::Phi3);
    assert_eq!(
        k.contention_check(a, b).unwrap(),
        compose(PhaseState::Phi2, PhaseState::Phi2)
    );
}

#[test]
fn topology_report_aggregates_reflection_events() {
    let mut k = Kernel::new();
    for _ in 0..2 {
        let pid = k.spawn();
        assert!(k.wake(pid).unwrap());
        assert!(k.schedule(pid).unwrap());
        assert!(k.syscall(pid).unwrap());
    }
    let report = k.topology_report();
    assert_eq!(report.len(), 2);
    for event in &report {
        assert_eq!(event.kind, TopologyKind::Reflection);
        assert_eq!(
            event.triple,
            (PhaseState::Phi1, PhaseState::Phi2, PhaseState::Phi3)
        );
    }
}
