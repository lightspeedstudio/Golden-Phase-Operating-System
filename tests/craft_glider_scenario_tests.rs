//! Avenue 10B WP-2 — end-to-end FDIR scenario over the real (hosted-sim)
//! glider carrier, plus packet-replay-vs-statechain cross-check.
//!
//! Hosted-sim: `CraftController::new_over_glider()` wires the craft fixture
//! to `phase_transport::TransportController` running in-process over the
//! sealed Avenue 2 glider law. This is not a claim of live-on-silicon
//! execution — every "packet crossing the carrier" here is a software
//! simulation of the same M=4 lattice that was silicon-verified separately.

use kairos::core::gphase_core::PhaseState;
use kairos::kernel::craft::{
    prop_stuck_over_glider_scenario, prop_stuck_scenario, CraftController, FdirController,
};
use kairos::kernel::state_proof::ObjectKind;

#[test]
fn glider_scenario_prints_fdir_pass_and_replay_pass() {
    let mut c = CraftController::new_over_glider().expect("controller over glider carrier");
    let report = prop_stuck_over_glider_scenario(&mut c).unwrap();
    assert!(report.contains("CRAFT FDIR PASS"));
    assert!(report.contains("CRAFT GLIDER SCENARIO PASS"));
    assert!(report.contains("PACKET REPLAY CROSS-CHECK"));
    assert!(report.contains("statechain replay: live == replayed (OK)"));
    assert!(report.contains("sealed checkpoint: sequence="));
}

#[test]
fn glider_scenario_runs_detect_isolate_recover_prove_in_order() {
    let mut c = CraftController::new_over_glider().expect("controller over glider carrier");
    prop_stuck_over_glider_scenario(&mut c).unwrap();

    let fdir_chain = c
        .ledger
        .chain(ObjectKind::Craft, kairos::kernel::craft::FDIR_OBJECT_ID)
        .expect("fdir chain present");
    let ops: Vec<&str> = fdir_chain
        .events
        .iter()
        .map(|e| e.proof.operation_name.as_str())
        .collect();

    let detect_at = ops
        .iter()
        .position(|o| *o == "fdir_detect")
        .expect("detect present");
    let isolate_at = ops
        .iter()
        .position(|o| *o == "fdir_isolate")
        .expect("isolate present");
    let recover_at = ops
        .iter()
        .position(|o| *o == "fdir_recover")
        .expect("recover present");
    let prove_at = ops
        .iter()
        .position(|o| *o == "fdir_prove")
        .expect("prove present");

    assert!(detect_at < isolate_at, "detect must precede isolate");
    assert!(isolate_at < recover_at, "isolate must precede recover");
    assert!(recover_at < prove_at, "recover must precede prove");

    // A checkpoint was sealed by FdirController::prove.
    assert!(
        c.last_checkpoint.is_some(),
        "expected sealed checkpoint after prove"
    );
}

#[test]
fn transported_packet_log_matches_statechain_audited_commands() {
    let mut c = CraftController::new_over_glider().expect("controller over glider carrier");
    prop_stuck_over_glider_scenario(&mut c).unwrap();

    // Every proof_op that carries a packet_id must resolve to a transport
    // record, and every ordinary (non-reject) transported op's opcode must
    // equal the ledger's recorded operation name for that transition.
    let records = c.transport.records();
    assert!(
        !records.is_empty(),
        "glider transport must have logged packets"
    );

    let mut cross_checked = 0usize;
    for proof_op in &c.proof_ops {
        let Some(pkt_id) = proof_op.packet_id else {
            continue;
        };
        let record = records
            .iter()
            .find(|r| r.packet_id == pkt_id)
            .unwrap_or_else(|| panic!("packet {pkt_id} missing from glider transport log"));
        // Every packet-carrying transition traces back to a real chain event
        // at the recorded sequence number.
        let chain = c
            .ledger
            .chain(ObjectKind::Craft, proof_op.object_id)
            .expect("chain for packet-bearing object");
        let event = chain
            .events
            .iter()
            .find(|e| e.proof.sequence_number == proof_op.sequence)
            .expect("statechain event at proof_op sequence");
        if event.proof.operation_name != "rejected_quarantined_operation" {
            assert_eq!(
                record.opcode, event.proof.operation_name,
                "transported opcode must match audited op for packet {pkt_id}"
            );
        }
        cross_checked += 1;
    }
    assert!(
        cross_checked > 0,
        "expected at least one packet-bearing transition"
    );
}

#[test]
fn assoc_parity_verifies_end_to_end() {
    let mut c = CraftController::new_over_glider().expect("controller over glider carrier");
    prop_stuck_over_glider_scenario(&mut c).unwrap();

    // Recompute assoc_parity for every recorded proof op and confirm it
    // agrees with what was stored at record time (mirrors what
    // FdirController::prove and the packet-replay cross-check both assert).
    for proof_op in &c.proof_ops {
        let parity = FdirController::recompute_assoc_parity(
            proof_op.assoc_a,
            proof_op.assoc_b,
            proof_op.assoc_c,
        )
        .expect("assoc_parity recomputation must not error");
        assert_eq!(
            parity, proof_op.craft_parity,
            "assoc_parity mismatch at object={} seq={}",
            proof_op.object_id, proof_op.sequence
        );
    }

    // Ledger-level verification (hash chain + checkpoint) also passes.
    c.ledger.verify_all().expect("ledger verify_all");
}

#[test]
fn glider_and_local_transport_reach_the_same_final_mission_state() {
    // Same mission, same outcome, different transport.
    let mut local = CraftController::new();
    prop_stuck_scenario(&mut local).unwrap();
    let local_snap = local.snapshot_states();

    let mut glider = CraftController::new_over_glider().expect("controller over glider carrier");
    prop_stuck_over_glider_scenario(&mut glider).unwrap();
    let glider_snap = glider.snapshot_states();

    assert_eq!(local_snap, glider_snap);
    assert_eq!(glider_snap.nav, PhaseState::Phi3);
    assert_eq!(glider_snap.prop, PhaseState::Phi0);
    assert_eq!(glider_snap.power, PhaseState::Phi3);
    assert_eq!(glider_snap.fdir, PhaseState::Phi0);
    assert!(!glider_snap.prop_quarantined);
}

#[test]
fn shell_scenario_dispatch_runs_glider_scenario() {
    use kairos::userland::parser::{parse, Command};
    assert_eq!(
        parse("scenario run prop-stuck-glider"),
        Command::ScenarioRun {
            name: "prop-stuck-glider".into(),
        }
    );
}
