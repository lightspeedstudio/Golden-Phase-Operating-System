//! Avenue 10 craft fixture — cycle and guard tests.

use kairos::core::gphase_core::{compose, PhaseState};
use kairos::kernel::craft::{
    CraftCommand, CraftController, CraftError, CraftKind, PROP_FIRING_MAX_TICKS,
};
use kairos::kernel::state_proof::ObjectKind;
use kairos::kernel::state_root::SubsystemKind;

#[test]
fn enum_numeric_compatibility() {
    assert_eq!(ObjectKind::Process.as_u8(), 0);
    assert_eq!(ObjectKind::Driver.as_u8(), 7);
    assert_eq!(ObjectKind::Craft.as_u8(), 8);
    assert_eq!(ObjectKind::from_u8(8), Some(ObjectKind::Craft));
    assert_eq!(ObjectKind::from_u8(99), None);
    assert_eq!(SubsystemKind::Driver.as_u8(), 5);
    assert_eq!(SubsystemKind::Craft.as_u8(), 6);
}

#[test]
fn canonical_compose_reused_mission_cycle() {
    assert_eq!(
        compose(PhaseState::Phi0, PhaseState::Phi1),
        PhaseState::Phi1
    );
    assert_eq!(
        compose(PhaseState::Phi1, PhaseState::Phi3),
        PhaseState::Phi2
    );
    assert_eq!(
        compose(PhaseState::Phi2, PhaseState::Phi1),
        PhaseState::Phi3
    );
    assert_eq!(
        compose(PhaseState::Phi3, PhaseState::Phi3),
        PhaseState::Phi0
    );
}

#[test]
fn navigation_four_state_cycle() {
    let mut c = CraftController::new();
    c.init_craft().unwrap();
    // init leaves nav stale (Φ1); reset to absent then cycle
    c.objects[0].state = PhaseState::Phi0;
    c.execute(CraftCommand::NavStale, None).unwrap();
    assert_eq!(c.obj(1).unwrap().state, PhaseState::Phi1);
    c.execute(CraftCommand::NavSolve, None).unwrap();
    assert_eq!(c.obj(1).unwrap().state, PhaseState::Phi2);
    c.execute(CraftCommand::NavLock, None).unwrap();
    assert_eq!(c.obj(1).unwrap().state, PhaseState::Phi3);
    c.execute(CraftCommand::NavReset, None).unwrap();
    assert_eq!(c.obj(1).unwrap().state, PhaseState::Phi0);
}

#[test]
fn propulsion_four_state_cycle() {
    let mut c = CraftController::new();
    c.init_craft().unwrap();
    c.execute(CraftCommand::PropArm, None).unwrap();
    assert_eq!(c.obj(2).unwrap().state, PhaseState::Phi1);
    c.execute(CraftCommand::PropFire, None).unwrap();
    assert_eq!(c.obj(2).unwrap().state, PhaseState::Phi2);
    c.execute(CraftCommand::PropMarkFault, None).unwrap();
    assert_eq!(c.obj(2).unwrap().state, PhaseState::Phi3);
    c.execute(CraftCommand::PropSafeReset, None).unwrap();
    assert_eq!(c.obj(2).unwrap().state, PhaseState::Phi0);
}

#[test]
fn power_four_state_cycle() {
    let mut c = CraftController::new();
    c.init_craft().unwrap();
    c.execute(CraftCommand::PowerReady, None).unwrap();
    c.execute(CraftCommand::PowerLoad, None).unwrap();
    c.execute(CraftCommand::PowerProtect, None).unwrap();
    c.execute(CraftCommand::PowerOff, None).unwrap();
    assert_eq!(c.obj(3).unwrap().state, PhaseState::Phi0);
}

#[test]
fn idle_arm_fire_succeeds() {
    let mut c = CraftController::new();
    c.init_craft().unwrap();
    c.execute(CraftCommand::PropArm, None).unwrap();
    c.execute(CraftCommand::PropFire, None).unwrap();
    assert_eq!(c.obj(2).unwrap().state, PhaseState::Phi2);
}

#[test]
fn fire_while_idle_rejected() {
    let mut c = CraftController::new();
    c.init_craft().unwrap();
    let err = c.execute(CraftCommand::PropFire, None).unwrap_err();
    assert!(matches!(err, CraftError::GuardRejected(_)));
}

#[test]
fn firing_to_fault_uses_phi1() {
    assert_eq!(CraftCommand::PropMarkFault.operand(), PhaseState::Phi1);
    assert_eq!(
        compose(PhaseState::Phi2, PhaseState::Phi1),
        PhaseState::Phi3
    );
}

#[test]
fn fault_to_idle_uses_phi3() {
    assert_eq!(CraftCommand::PropSafeReset.operand(), PhaseState::Phi3);
    assert_eq!(
        compose(PhaseState::Phi3, PhaseState::Phi3),
        PhaseState::Phi0
    );
}

#[test]
fn dwell_detector_boundary() {
    let mut c = CraftController::new();
    c.init_craft().unwrap();
    c.execute(CraftCommand::PropArm, None).unwrap();
    c.execute(CraftCommand::PropFire, None).unwrap();
    c.tick(PROP_FIRING_MAX_TICKS).unwrap();
    assert_eq!(c.obj(2).unwrap().ticks_in_state, PROP_FIRING_MAX_TICKS);
    assert!(kairos::kernel::craft::fdir::FdirController::anomaly(
        CraftKind::Propulsion,
        PhaseState::Phi2,
        PROP_FIRING_MAX_TICKS,
        0
    )
    .is_none());
    c.tick(1).unwrap();
    assert!(kairos::kernel::craft::fdir::FdirController::anomaly(
        CraftKind::Propulsion,
        PhaseState::Phi2,
        c.obj(2).unwrap().ticks_in_state,
        0
    )
    .is_some());
}
