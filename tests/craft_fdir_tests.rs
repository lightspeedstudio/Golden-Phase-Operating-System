//! Avenue 10 FDIR path tests.

use kairos::core::gphase_core::PhaseState;
use kairos::kernel::craft::{
    prop_stuck_scenario, CraftCommand, CraftController, CraftError, CraftKind, FdirController,
    PROP_FIRING_MAX_TICKS,
};

#[test]
fn fdir_four_state_cycle() {
    let mut c = CraftController::new();
    c.init_craft().unwrap();
    c.apply_fdir(CraftCommand::FdirDetect, 1).unwrap();
    assert_eq!(c.fdir_state, PhaseState::Phi1);
    c.apply_fdir(CraftCommand::FdirIsolate, 1).unwrap();
    assert_eq!(c.fdir_state, PhaseState::Phi2);
    c.apply_fdir(CraftCommand::FdirRecover, 1).unwrap();
    assert_eq!(c.fdir_state, PhaseState::Phi3);
    c.apply_fdir(CraftCommand::FdirProve, 1).unwrap();
    assert_eq!(c.fdir_state, PhaseState::Phi0);
}

#[test]
fn quarantine_refuses_ordinary_and_audits() {
    let mut c = CraftController::new();
    c.init_craft().unwrap();
    c.execute(CraftCommand::PropArm, None).unwrap();
    c.execute(CraftCommand::PropFire, None).unwrap();
    c.tick(PROP_FIRING_MAX_TICKS + 1).unwrap();
    let ep = FdirController::detect_propulsion_stuck(&mut c).unwrap();
    FdirController::isolate(&mut c, ep).unwrap();
    let before = c
        .ledger
        .chain(
            kairos::kernel::state_proof::ObjectKind::Craft,
            CraftKind::Propulsion.object_id(),
        )
        .unwrap()
        .events
        .len();
    let err = c.execute(CraftCommand::PropFire, Some(ep)).unwrap_err();
    assert!(matches!(err, CraftError::Quarantined));
    let after = c
        .ledger
        .chain(
            kairos::kernel::state_proof::ObjectKind::Craft,
            CraftKind::Propulsion.object_id(),
        )
        .unwrap()
        .events
        .len();
    assert!(after > before);
    assert!(c
        .ledger
        .chain(
            kairos::kernel::state_proof::ObjectKind::Craft,
            CraftKind::Propulsion.object_id(),
        )
        .unwrap()
        .events
        .iter()
        .any(|e| e.proof.operation_name == "rejected_quarantined_operation"));
}

#[test]
fn fdir_cannot_clear_before_proof() {
    let mut c = CraftController::new();
    c.init_craft().unwrap();
    c.execute(CraftCommand::PropArm, None).unwrap();
    c.execute(CraftCommand::PropFire, None).unwrap();
    c.tick(PROP_FIRING_MAX_TICKS + 1).unwrap();
    let ep = FdirController::detect_propulsion_stuck(&mut c).unwrap();
    FdirController::isolate(&mut c, ep).unwrap();
    assert!(FdirController::prove(&mut c, ep).is_err());
    assert!(c.obj(2).unwrap().quarantined);
}

#[test]
fn power_enters_protected_on_recovery() {
    let mut c = CraftController::new();
    c.init_craft().unwrap();
    c.execute(CraftCommand::PowerReady, None).unwrap();
    c.execute(CraftCommand::PowerLoad, None).unwrap();
    c.execute(CraftCommand::PropArm, None).unwrap();
    c.execute(CraftCommand::PropFire, None).unwrap();
    c.tick(PROP_FIRING_MAX_TICKS + 1).unwrap();
    let ep = FdirController::detect_propulsion_stuck(&mut c).unwrap();
    FdirController::isolate(&mut c, ep).unwrap();
    c.execute(CraftCommand::PropMarkFault, Some(ep)).unwrap();
    c.execute(CraftCommand::PowerProtect, Some(ep)).unwrap();
    assert_eq!(c.obj(3).unwrap().state, PhaseState::Phi3);
}

#[test]
fn complete_prop_stuck_prints_pass() {
    let mut c = CraftController::new();
    let report = prop_stuck_scenario(&mut c).unwrap();
    assert!(report.contains("CRAFT FDIR PASS"));
    assert_eq!(c.obj(1).unwrap().state, PhaseState::Phi3);
    assert_eq!(c.obj(2).unwrap().state, PhaseState::Phi0);
    assert_eq!(c.obj(3).unwrap().state, PhaseState::Phi3);
    assert_eq!(c.fdir_state, PhaseState::Phi0);
    assert!(!c.obj(2).unwrap().quarantined);
}
