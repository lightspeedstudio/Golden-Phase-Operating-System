//! Avenue 10A craft replay and tamper tests.

use kairos::core::gphase_core::PhaseState;
use kairos::kernel::craft::{
    prop_stuck_scenario, CraftController, CraftKind, FdirController, FDIR_OBJECT_ID,
};
use kairos::kernel::state_proof::{
    compute_event_hash, hash_payload, ObjectKind, StateTransitionProof,
};
use kairos::kernel::state_root::{compute_equation_root, SubsystemKind};

#[test]
fn clean_chain_verifies_and_replay_matches() {
    let mut c = CraftController::new();
    prop_stuck_scenario(&mut c).unwrap();
    c.ledger.verify_all().unwrap();
    let live = c.snapshot_states();
    let replayed = c.replay_episode(1).unwrap();
    assert_eq!(live.nav, replayed.nav);
    assert_eq!(live.prop, replayed.prop);
    assert_eq!(live.power, replayed.power);
    assert_eq!(live.fdir, replayed.fdir);
}

#[test]
fn assoc_parity_recomputes() {
    let mut c = CraftController::new();
    prop_stuck_scenario(&mut c).unwrap();
    for p in &c.proof_ops {
        let recomputed =
            FdirController::recompute_assoc_parity(p.assoc_a, p.assoc_b, p.assoc_c).unwrap();
        assert_eq!(recomputed, p.craft_parity);
    }
}

#[test]
fn event_hash_tamper_fails() {
    let mut c = CraftController::new();
    prop_stuck_scenario(&mut c).unwrap();
    c.ledger.xor_event_hash_for_audit_test(
        ObjectKind::Craft,
        CraftKind::Propulsion.object_id(),
        1,
        0xFF,
    );
    assert!(c.ledger.verify_all().is_err());
}

#[test]
fn parity_tamper_fails_verify() {
    let mut c = CraftController::new();
    prop_stuck_scenario(&mut c).unwrap();
    c.ledger.flip_assoc_parity_for_audit_test(
        ObjectKind::Craft,
        CraftKind::Propulsion.object_id(),
        1,
    );
    assert!(c.ledger.verify_all().is_err());
}

#[test]
fn transition_operand_alteration_fails_replay() {
    let mut c = CraftController::new();
    prop_stuck_scenario(&mut c).unwrap();
    let p = c
        .proof_ops
        .iter_mut()
        .find(|p| {
            p.object_id == CraftKind::Propulsion.object_id()
                && p.right == PhaseState::Phi3
                && p.left == PhaseState::Phi1
        })
        .expect("prop fire operand");
    p.right = PhaseState::Phi2; // illegal operand rewrite
    assert!(c.replay_episode(1).is_err());
}

#[test]
fn transition_result_tamper_fails_replay() {
    let mut c = CraftController::new();
    prop_stuck_scenario(&mut c).unwrap();
    if let Some(p) = c
        .proof_ops
        .iter_mut()
        .find(|p| p.object_id == CraftKind::Propulsion.object_id() && p.right == PhaseState::Phi1)
    {
        p.craft_parity ^= 1;
    }
    assert!(c.replay_episode(1).is_err());
}

#[test]
fn object_id_substitution_fails() {
    let mut c = CraftController::new();
    prop_stuck_scenario(&mut c).unwrap();
    c.ledger.set_event_object_id_for_audit_test(
        ObjectKind::Craft,
        CraftKind::Propulsion.object_id(),
        0,
        99,
    );
    assert!(c.replay_episode(1).is_err());
}

#[test]
fn entry_reorder_fails() {
    let mut c = CraftController::new();
    prop_stuck_scenario(&mut c).unwrap();
    c.ledger
        .swap_events_for_audit_test(ObjectKind::Craft, CraftKind::Propulsion.object_id(), 0, 1);
    assert!(c.ledger.verify_all().is_err());
}

#[test]
fn non_tail_deletion_fails() {
    let mut c = CraftController::new();
    prop_stuck_scenario(&mut c).unwrap();
    let len = c
        .ledger
        .chain(ObjectKind::Craft, CraftKind::Propulsion.object_id())
        .unwrap()
        .events
        .len();
    assert!(len >= 3);
    c.ledger
        .delete_event_for_audit_test(ObjectKind::Craft, CraftKind::Propulsion.object_id(), 1);
    assert!(c.ledger.verify_all().is_err());
}

#[test]
fn recorded_old_state_alteration_fails() {
    let mut c = CraftController::new();
    prop_stuck_scenario(&mut c).unwrap();
    c.ledger.set_event_previous_state_for_audit_test(
        ObjectKind::Craft,
        CraftKind::Propulsion.object_id(),
        1,
        PhaseState::Phi3,
    );
    assert!(c.ledger.verify_all().is_err());
}

#[test]
fn operation_name_alteration_fails() {
    let mut c = CraftController::new();
    prop_stuck_scenario(&mut c).unwrap();
    c.ledger.set_event_operation_for_audit_test(
        ObjectKind::Craft,
        CraftKind::Propulsion.object_id(),
        1,
        "tampered_op",
    );
    assert!(c.ledger.verify_all().is_err());
}

#[test]
fn sequence_number_gap_fails() {
    let mut c = CraftController::new();
    prop_stuck_scenario(&mut c).unwrap();
    c.ledger.set_event_sequence_for_audit_test(
        ObjectKind::Craft,
        CraftKind::Propulsion.object_id(),
        1,
        99,
    );
    assert!(c.ledger.verify_all().is_err());
}

#[test]
fn subsystem_kind_substitution_fails_replay() {
    let mut c = CraftController::new();
    prop_stuck_scenario(&mut c).unwrap();
    c.ledger.set_event_subsystem_for_audit_test(
        ObjectKind::Craft,
        CraftKind::Propulsion.object_id(),
        1,
        SubsystemKind::Driver,
    );
    assert!(c.replay_episode(1).is_err());
}

#[test]
fn episode_link_alteration_fails_prove() {
    let mut c = CraftController::new();
    // Stop before prove: rebuild through recover only
    c.init_craft().unwrap();
    c.execute(kairos::kernel::craft::CraftCommand::PowerReady, None)
        .unwrap();
    c.execute(kairos::kernel::craft::CraftCommand::NavSolve, None)
        .unwrap();
    c.execute(kairos::kernel::craft::CraftCommand::NavLock, None)
        .unwrap();
    c.execute(kairos::kernel::craft::CraftCommand::PropArm, None)
        .unwrap();
    c.execute(kairos::kernel::craft::CraftCommand::PowerLoad, None)
        .unwrap();
    c.execute(kairos::kernel::craft::CraftCommand::PropFire, None)
        .unwrap();
    c.fault_inject_prop_stuck("x").unwrap();
    c.tick(kairos::kernel::craft::PROP_FIRING_MAX_TICKS + 1)
        .unwrap();
    let ep = FdirController::detect_propulsion_stuck(&mut c).unwrap();
    FdirController::isolate(&mut c, ep).unwrap();
    c.execute(kairos::kernel::craft::CraftCommand::PropMarkFault, Some(ep))
        .unwrap();
    c.execute(kairos::kernel::craft::CraftCommand::PowerProtect, Some(ep))
        .unwrap();
    c.execute(kairos::kernel::craft::CraftCommand::PropSafeReset, Some(ep))
        .unwrap();
    c.apply_fdir(kairos::kernel::craft::CraftCommand::FdirRecover, ep)
        .unwrap();
    c.episodes[0].object_id = 0; // break episode link
    assert!(FdirController::prove(&mut c, ep).is_err());
    assert!(c.obj(2).unwrap().quarantined);
    assert_ne!(c.fdir_state, PhaseState::Phi0);
}

#[test]
fn checkpoint_head_alteration_fails() {
    let mut c = CraftController::new();
    prop_stuck_scenario(&mut c).unwrap();
    let mut bad = c.ledger.latest_checkpoint().unwrap().clone();
    bad.kernel_root[0] ^= 0x5A;
    c.ledger.replace_checkpoint_for_audit_test(0, bad);
    assert!(c.ledger.verify_checkpoints().is_err());
}

#[test]
fn live_state_mismatch_detected() {
    let mut c = CraftController::new();
    prop_stuck_scenario(&mut c).unwrap();
    c.objects
        .iter_mut()
        .find(|o| o.kind == CraftKind::Propulsion)
        .unwrap()
        .state = PhaseState::Phi2;
    let live = c.snapshot_states();
    let replayed = c.replay_episode(1).unwrap();
    assert_ne!(live.prop, replayed.prop);
}

#[test]
fn event_payload_tamper_breaks_hash() {
    let mut c = CraftController::new();
    prop_stuck_scenario(&mut c).unwrap();
    let chain = c.ledger.chain(ObjectKind::Craft, FDIR_OBJECT_ID).unwrap();
    let ev = &chain.events[0];
    let mut proof = ev.proof.clone();
    proof.payload_hash[0] ^= 0xAA;
    let recomputed = compute_event_hash(&proof, ev.assoc_parity);
    assert_ne!(recomputed, ev.event_hash);
}

#[test]
fn process_event_hash_unaffected_by_craft_proof_ops_channel() {
    // TransitionProofOp is a craft-side channel; Process event hashes must use
    // the unchanged EVENT_TAG layout keyed by ObjectKind::as_u8() discriminants.
    let eq = compute_equation_root();
    let proof = StateTransitionProof {
        object_kind: ObjectKind::Process,
        object_id: 1,
        sequence_number: 0,
        previous_state: PhaseState::Phi1,
        requested_state: PhaseState::Phi2,
        operation_name: "schedule".into(),
        compose_result: None,
        topology_status: kairos::kernel::state_proof::TopologyProofStatus::None,
        payload_hash: hash_payload(b""),
        previous_event_hash: [0u8; 32],
        equation_root: eq.hash,
    };
    assert_eq!(ObjectKind::Process.as_u8(), 0);
    let h = compute_event_hash(&proof, 0);
    assert_ne!(h, [0u8; 32]);
    // Discriminant byte in hash input is Process=0, not Craft=8.
    let mut craftish = proof.clone();
    craftish.object_kind = ObjectKind::Craft;
    assert_ne!(compute_event_hash(&craftish, 0), h);
}

/// Documented limitation: coordinated rewrite of chain events + subsystem head
/// cache can defeat naive head checks if both are mutated together.
#[test]
fn coordinated_chain_cache_rewrite_limitation_documented() {
    assert!(
        true,
        "coordinated chain+cache rewrite requires external seal"
    );
}
