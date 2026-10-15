//! Avenue 10A partial-recovery failure invariants.

use kairos::core::gphase_core::PhaseState;
use kairos::kernel::craft::{
    CraftCommand, CraftController, CraftError, CraftKind, FdirController, PROP_FIRING_MAX_TICKS,
};

fn setup_quarantined_firing() -> (CraftController, u64) {
    let mut c = CraftController::new();
    c.init_craft().unwrap();
    c.execute(CraftCommand::PowerReady, None).unwrap();
    c.execute(CraftCommand::NavSolve, None).unwrap();
    c.execute(CraftCommand::NavLock, None).unwrap();
    c.execute(CraftCommand::PropArm, None).unwrap();
    c.execute(CraftCommand::PowerLoad, None).unwrap();
    c.execute(CraftCommand::PropFire, None).unwrap();
    c.fault_inject_prop_stuck("stuck").unwrap();
    c.tick(PROP_FIRING_MAX_TICKS + 1).unwrap();
    let ep = FdirController::detect_propulsion_stuck(&mut c).unwrap();
    FdirController::isolate(&mut c, ep).unwrap();
    assert!(
        c.obj(CraftKind::Propulsion.object_id())
            .unwrap()
            .quarantined
    );
    assert_eq!(c.fdir_state, PhaseState::Phi2);
    (c, ep)
}

#[test]
fn power_protection_failure_keeps_quarantine() {
    let (mut c, ep) = setup_quarantined_firing();
    c.execute(CraftCommand::PropMarkFault, Some(ep)).unwrap();
    // Force power out of load so POWER_PROTECT guard fails.
    c.obj_mut(CraftKind::Power.object_id()).unwrap().state = PhaseState::Phi0;
    let err = c.execute(CraftCommand::PowerProtect, Some(ep)).unwrap_err();
    assert!(matches!(err, CraftError::GuardRejected(_)));
    assert!(
        c.obj(CraftKind::Propulsion.object_id())
            .unwrap()
            .quarantined
    );
    assert_eq!(c.fdir_state, PhaseState::Phi2); // not recovered, not normal
    assert!(FdirController::prove(&mut c, ep).is_err());
}

#[test]
fn propulsion_reset_failure_keeps_quarantine() {
    let (mut c, ep) = setup_quarantined_firing();
    c.execute(CraftCommand::PropMarkFault, Some(ep)).unwrap();
    c.execute(CraftCommand::PowerProtect, Some(ep)).unwrap();
    // Force prop out of fault so SAFE_RESET guard fails.
    c.obj_mut(CraftKind::Propulsion.object_id()).unwrap().state = PhaseState::Phi2;
    let err = c
        .execute(CraftCommand::PropSafeReset, Some(ep))
        .unwrap_err();
    assert!(matches!(err, CraftError::GuardRejected(_)));
    assert!(
        c.obj(CraftKind::Propulsion.object_id())
            .unwrap()
            .quarantined
    );
    assert_ne!(c.fdir_state, PhaseState::Phi0);
    assert_ne!(c.fdir_state, PhaseState::Phi3); // never reached recovered
    assert!(FdirController::prove(&mut c, ep).is_err());
}

#[test]
fn recover_propulsion_aborts_before_fdir_recover_on_mid_failure() {
    let (mut c, ep) = setup_quarantined_firing();
    c.obj_mut(CraftKind::Power.object_id()).unwrap().state = PhaseState::Phi1; // not load
    let err = FdirController::recover_propulsion(&mut c, ep).unwrap_err();
    assert!(matches!(
        err,
        CraftError::GuardRejected(_) | CraftError::IllegalTransition(_)
    ));
    assert!(
        c.obj(CraftKind::Propulsion.object_id())
            .unwrap()
            .quarantined
    );
    assert_eq!(c.fdir_state, PhaseState::Phi2);
}

#[test]
fn replay_failure_blocks_prove_and_keeps_quarantine() {
    let (mut c, ep) = setup_quarantined_firing();
    FdirController::recover_propulsion(&mut c, ep).unwrap();
    assert_eq!(c.fdir_state, PhaseState::Phi3);
    // Corrupt craft parity so replay fails inside prove.
    for p in c.proof_ops.iter_mut() {
        p.craft_parity ^= 1;
    }
    assert!(FdirController::prove(&mut c, ep).is_err());
    assert!(
        c.obj(CraftKind::Propulsion.object_id())
            .unwrap()
            .quarantined
    );
    assert_ne!(c.fdir_state, PhaseState::Phi0);
}

#[test]
fn checkpoint_failure_blocks_prove_and_keeps_quarantine() {
    let (mut c, ep) = setup_quarantined_firing();
    FdirController::recover_propulsion(&mut c, ep).unwrap();
    // Poison equation/signing path by replacing a bogus prior checkpoint that
    // verify_checkpoints will reject after seal — instead break verify_all first
    // via event hash so prove aborts before FdirProve.
    c.ledger.xor_event_hash_for_audit_test(
        kairos::kernel::state_proof::ObjectKind::Craft,
        CraftKind::Propulsion.object_id(),
        0,
        0xFF,
    );
    assert!(FdirController::prove(&mut c, ep).is_err());
    assert!(
        c.obj(CraftKind::Propulsion.object_id())
            .unwrap()
            .quarantined
    );
    assert_eq!(c.fdir_state, PhaseState::Phi3); // recovered pending, not cleared
}
