//! Host lock-step tests. MockG1pBus is a test double of the G1P V2 map, not a board.

use kairos::core::gphase_core::{compose, topology_status, PhaseState, TopologyKind};
use kairos::kernel::phase_consensus::{
    evaluate_lockstep, ConsensusError, ConsensusMode, EmulatedAccelerator, MockG1p,
};
use kairos::Kernel;

#[test]
fn sixty_four_triples_mock_matches_frozen_census() {
    let mock = MockG1p::matching();
    let mut same = 0;
    let mut reflection = 0;
    for a in PhaseState::ALL {
        for b in PhaseState::ALL {
            for c in PhaseState::ALL {
                let ok = evaluate_lockstep(&mock, a, b, c).expect("mock must match gphase_core");
                assert_eq!(ok.mode, ConsensusMode::Accelerated);
                assert_eq!(ok.sw.compose, compose(a, b));
                match topology_status(a, b, c).kind {
                    TopologyKind::Same => same += 1,
                    TopologyKind::Reflection => reflection += 1,
                }
            }
        }
    }
    assert_eq!(same, 52);
    assert_eq!(reflection, 12);
}

#[test]
fn timeout_holds_busy_and_does_not_commit() {
    let mock = MockG1p::matching().with_timeout(4).hold_busy(64);
    let err = evaluate_lockstep(&mock, PhaseState::Phi0, PhaseState::Phi1, PhaseState::Phi0)
        .unwrap_err();
    assert_eq!(err, ConsensusError::Timeout);
}

#[test]
fn compose_mismatch_is_rejected() {
    let mock = MockG1p::matching().xor_compose(1);
    let err = evaluate_lockstep(&mock, PhaseState::Phi0, PhaseState::Phi1, PhaseState::Phi0)
        .unwrap_err();
    assert_eq!(err, ConsensusError::Mismatch);
}

#[test]
fn opcode_error_is_hardware_fault() {
    let mock = MockG1p::matching().force_error();
    let err = evaluate_lockstep(&mock, PhaseState::Phi0, PhaseState::Phi1, PhaseState::Phi0)
        .unwrap_err();
    assert_eq!(err, ConsensusError::HardwareFault);
}

#[test]
fn legacy_zero_trace_accepts_matching_results() {
    let mock = MockG1p::matching().legacy_no_trace();
    let ok = evaluate_lockstep(&mock, PhaseState::Phi0, PhaseState::Phi1, PhaseState::Phi2).unwrap();
    assert_eq!(ok.hw.hash, 0);
    assert_eq!(ok.hw.cycle, 0);
    assert_eq!(ok.sw.compose, compose(PhaseState::Phi0, PhaseState::Phi1));
}

#[test]
fn emulated_mode_is_labeled() {
    let accel = EmulatedAccelerator::new();
    let ok = evaluate_lockstep(&accel, PhaseState::Phi1, PhaseState::Phi2, PhaseState::Phi3)
        .unwrap();
    assert_eq!(ok.mode, ConsensusMode::Emulated);
}

#[test]
fn probe_failure_rejects() {
    let mock = MockG1p::matching().fail_probe();
    assert_eq!(
        evaluate_lockstep(&mock, PhaseState::Phi0, PhaseState::Phi0, PhaseState::Phi0),
        Err(ConsensusError::ProbeFailed)
    );
}

#[test]
fn kernel_contention_default_is_emulated() {
    let mut k = Kernel::new();
    let a = k.spawn();
    let b = k.spawn();
    for pid in [a, b] {
        assert!(k.wake(pid).unwrap());
        assert!(k.schedule(pid).unwrap());
    }
    assert_eq!(k.consensus.mode(), ConsensusMode::Emulated);
    assert_eq!(k.contention_check(a, b).unwrap(), PhaseState::Phi3);
}

#[test]
fn kernel_contention_mismatch_mock_rejects() {
    let mut k = Kernel::new();
    k.attach_phase_accelerator(Box::new(MockG1p::matching().xor_compose(1)));
    let a = k.spawn();
    let b = k.spawn();
    for pid in [a, b] {
        assert!(k.wake(pid).unwrap());
        assert!(k.schedule(pid).unwrap());
    }
    assert_eq!(k.contention_check(a, b), Err("phase consensus rejected"));
}

#[test]
fn kernel_arbitrate_mismatch_mock_rejects() {
    let mut k = Kernel::new();
    k.attach_phase_accelerator(Box::new(MockG1p::matching().xor_compose(1)));
    let user = k.alloc_page();
    k.map_user(user).unwrap();
    let kern = k.alloc_page();
    k.map_kernel(kern).unwrap();
    assert!(k.memory_arbitrate(user, kern).is_err());
}
