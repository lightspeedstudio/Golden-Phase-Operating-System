//! StateDomain VM — virtual phase states encoded as Φ-words.

use kairos::core::gphase_core::PhaseState;
use kairos::kernel::state_domain::{
    build_chemistry_domain, build_state_change_proof, compute_domain_root,
    compute_state_change_proof_hash, decode_phase_word, encode_state_index, render_domain_map,
    render_domain_map_table, verify_state_change_proof, word_length_for_state_count, DomainId,
    StateChangeMode,
};
use kairos::kernel::state_proof::ObjectKind;
use kairos::kernel::state_root::compute_equation_root;
use kairos::Runtime;

fn eq_root() -> [u8; 32] {
    compute_equation_root().hash
}

#[test]
fn word_length_buckets() {
    assert_eq!(word_length_for_state_count(1), 1);
    assert_eq!(word_length_for_state_count(4), 1);
    assert_eq!(word_length_for_state_count(5), 2);
    assert_eq!(word_length_for_state_count(16), 2);
    assert_eq!(word_length_for_state_count(17), 3);
    assert_eq!(word_length_for_state_count(64), 3);
}

#[test]
fn encode_decode_round_trip() {
    for index in 0..16 {
        let word = encode_state_index(index, 2);
        assert_eq!(decode_phase_word(&word), index);
    }
}

#[test]
fn encode_examples_length_2() {
    assert_eq!(
        encode_state_index(0, 2),
        vec![PhaseState::Phi0, PhaseState::Phi0]
    );
    assert_eq!(
        encode_state_index(1, 2),
        vec![PhaseState::Phi0, PhaseState::Phi1]
    );
    assert_eq!(
        encode_state_index(4, 2),
        vec![PhaseState::Phi1, PhaseState::Phi0]
    );
}

#[test]
fn chemistry_domain_shape() {
    let domain = build_chemistry_domain(&eq_root());
    assert_eq!(domain.state_count, 10);
    assert_eq!(domain.word_length, 2);
    assert_eq!(domain.capacity, 16);
    assert_eq!(
        domain.state_by_label("positive_ion").unwrap().phase_word,
        encode_state_index(1, 2)
    );
}

#[test]
fn domain_root_changes_on_label() {
    let eq = eq_root();
    let a = build_chemistry_domain(&eq);
    let mut labels = kairos::kernel::state_domain::chemistry_labels();
    labels[0].0 = "neutral_renamed".into();
    let b = kairos::kernel::state_domain::build_domain_mapping(DomainId::Chemistry, &labels, &eq);
    assert_ne!(a.domain_root, b.domain_root);
}

#[test]
fn domain_root_changes_on_phase_word() {
    let eq = eq_root();
    let domain = build_chemistry_domain(&eq);
    let mut states = domain.states.clone();
    states[0].phase_word = encode_state_index(9, 2);
    let root = compute_domain_root(&eq, &DomainId::Chemistry, &states, domain.word_length);
    assert_ne!(domain.domain_root, root);
}

#[test]
fn state_change_proof_valid_and_tamper_fails() {
    let eq = eq_root();
    let domain = build_chemistry_domain(&eq);
    let old = domain.state_by_label("neutral").unwrap();
    let new = domain.state_by_label("positive_ion").unwrap();
    let snap = kairos::StateClock::new().snapshot();
    let proof = build_state_change_proof(
        &eq,
        &domain,
        12,
        &old.label,
        &old.phase_word,
        &new.label,
        &new.phase_word,
        "test",
        &snap,
        &[0u8; 32],
    );
    assert!(verify_state_change_proof(&proof, ObjectKind::Driver));

    let mut bad = proof.clone();
    bad.old_virtual_state = "tampered".into();
    assert!(!verify_state_change_proof(&bad, ObjectKind::Driver));

    let mut bad2 = proof.clone();
    bad2.new_virtual_state = "radical".into();
    assert!(!verify_state_change_proof(&bad2, ObjectKind::Driver));

    let mut bad3 = proof.clone();
    bad3.old_phase_word = encode_state_index(2, 2);
    assert_ne!(
        compute_state_change_proof_hash(
            &proof.equation_root,
            &proof.domain_root,
            ObjectKind::Driver,
            proof.object_id,
            &proof.old_virtual_state,
            &bad3.old_phase_word,
            &proof.operation,
            &proof.new_virtual_state,
            &proof.new_phase_word,
            &proof.stateclock_snapshot,
            &proof.previous_event_hash,
        ),
        proof.proof_hash
    );
}

#[test]
fn state_map_chemistry_prints_labels() {
    let domain = build_chemistry_domain(&eq_root());
    let text = render_domain_map(&domain);
    assert!(text.contains("positive_ion"));
    assert!(text.contains("Φ0 Φ1"));
    let table = render_domain_map_table(&domain);
    assert!(table.contains("neutral"));
}

#[test]
fn shell_state_domains_and_change() {
    let mut rt = Runtime::with_input([
        "state domains",
        "state mode expanded",
        "state map chemistry",
        "state change chem.node.12 positive_ion",
        "state view chem.node.12",
        "exit",
    ]);
    rt.boot().unwrap();
    let pid = rt.hosted.init_pid.unwrap();
    rt.hosted.kernel.syscall(pid).unwrap();
    rt.hosted.kernel.kreturn(pid).unwrap();
    rt.hosted.kernel.schedule(pid).unwrap();
    rt.run().unwrap();
    let out = rt.output_lines().join("\n");
    assert!(out.contains("chemistry states=10"));
    assert!(out.contains("expanded"));
    assert!(out.contains("positive_ion"));
    assert!(out.contains("chem.node.12"));
}

#[test]
fn statechain_records_domain_change() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    rt.hosted
        .kernel
        .change_domain_state("chem.node.1", &DomainId::Chemistry, "radical", "test")
        .unwrap();
    let chain_id = rt
        .hosted
        .kernel
        .state_domain
        .driver_chain_id_for("chem.node.1")
        .unwrap();
    let chain = rt
        .hosted
        .kernel
        .statechain
        .chain(ObjectKind::Driver, chain_id)
        .unwrap();
    assert!(chain
        .events
        .iter()
        .any(|e| e.proof.operation_name == "domain_state_change"));
}

#[test]
fn boots_root4_mode() {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    assert_eq!(rt.hosted.kernel.state_domain.mode, StateChangeMode::Root4);
}
