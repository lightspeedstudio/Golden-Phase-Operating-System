//! Equation-rooted Merkle statechain tests (from KairOS security design note).

use kairos::core::gphase_core::{compose, PhaseState, ALPHA};
use kairos::kernel::state_proof::{
    compute_event_hash, hash_payload, verify_compose_result, verify_proof, ObjectKind,
    StateTransitionProof, TopologyProofStatus,
};
use kairos::kernel::state_root::{
    compose_table_bytes, compute_equation_root, compute_kernel_state_root,
    compute_prime_log_commitment, merkle_root, SubsystemKind,
};
use kairos::kernel::statechain::{ChainError, StatechainLedger};
use kairos::kernel::statechain_watchdog::StatechainWatchdog;
use kairos::kernel::Kernel;

#[test]
fn equation_root_is_deterministic() {
    let a = compute_equation_root();
    let b = compute_equation_root();
    assert_eq!(a, b);
    assert_ne!(a.hash, [0u8; 32]);
}

#[test]
fn prime_log_commitment_is_deterministic() {
    let eq = compute_equation_root();
    let a = compute_prime_log_commitment(&eq);
    let b = compute_prime_log_commitment(&eq);
    assert_eq!(a, b);
}

#[test]
fn compose_table_matches_core_in_equation_root() {
    let root = compute_equation_root();
    let table = compose_table_bytes();
    for a in PhaseState::ALL {
        for b in PhaseState::ALL {
            let idx = (a.as_u8() as usize) << 2 | b.as_u8() as usize;
            assert_eq!(table[idx], compose(a, b).as_u8());
        }
    }
    assert_ne!(root.hash, [0u8; 32]);
}

#[test]
fn valid_transition_proof_verifies() {
    let eq = compute_equation_root();
    let proof = StateTransitionProof {
        object_kind: ObjectKind::Process,
        object_id: 1,
        sequence_number: 0,
        previous_state: PhaseState::Phi1,
        requested_state: PhaseState::Phi2,
        operation_name: "schedule".into(),
        compose_result: None,
        topology_status: TopologyProofStatus::None,
        payload_hash: hash_payload(b""),
        previous_event_hash: [0u8; 32],
        equation_root: eq.hash,
    };
    verify_proof(&proof, &[0u8; 32]).unwrap();
}

#[test]
fn invalid_compose_result_fails_verification() {
    let eq = compute_equation_root();
    let proof = StateTransitionProof {
        object_kind: ObjectKind::Process,
        object_id: 1,
        sequence_number: 0,
        previous_state: PhaseState::Phi1,
        requested_state: PhaseState::Phi2,
        operation_name: "contention".into(),
        compose_result: Some(PhaseState::Phi0),
        topology_status: TopologyProofStatus::None,
        payload_hash: hash_payload(b""),
        previous_event_hash: [0u8; 32],
        equation_root: eq.hash,
    };
    assert!(verify_compose_result(&proof, PhaseState::Phi1, PhaseState::Phi2).is_err());
    let expected = compose(PhaseState::Phi1, PhaseState::Phi2);
    let mut good = proof.clone();
    good.compose_result = Some(expected);
    verify_compose_result(&good, PhaseState::Phi1, PhaseState::Phi2).unwrap();
}

#[test]
fn event_hash_changes_with_payload() {
    let eq = compute_equation_root();
    let mut proof = StateTransitionProof {
        object_kind: ObjectKind::MemoryPage,
        object_id: 0,
        sequence_number: 0,
        previous_state: PhaseState::Phi0,
        requested_state: PhaseState::Phi1,
        operation_name: "map_user".into(),
        compose_result: None,
        topology_status: TopologyProofStatus::None,
        payload_hash: hash_payload(b"a"),
        previous_event_hash: [0u8; 32],
        equation_root: eq.hash,
    };
    let h1 = compute_event_hash(&proof, 0);
    proof.payload_hash = hash_payload(b"b");
    let h2 = compute_event_hash(&proof, 0);
    assert_ne!(h1, h2);
}

#[test]
fn event_hash_changes_with_assoc_parity() {
    let eq = compute_equation_root();
    let proof = StateTransitionProof {
        object_kind: ObjectKind::Process,
        object_id: 0,
        sequence_number: 0,
        previous_state: PhaseState::Phi0,
        requested_state: PhaseState::Phi1,
        operation_name: "wake".into(),
        compose_result: None,
        topology_status: TopologyProofStatus::None,
        payload_hash: hash_payload(b""),
        previous_event_hash: [0u8; 32],
        equation_root: eq.hash,
    };
    assert_ne!(compute_event_hash(&proof, 0), compute_event_hash(&proof, 1));
}

#[test]
fn deleting_event_breaks_chain() {
    let mut chain = kairos::kernel::statechain::ObjectStatechain::new(ObjectKind::Process, 0);
    let eq = compute_equation_root();
    let proof = |op: &str, prev: kairos::kernel::hash::StateHash| StateTransitionProof {
        object_kind: ObjectKind::Process,
        object_id: 0,
        sequence_number: 0,
        previous_state: PhaseState::Phi0,
        requested_state: PhaseState::Phi1,
        operation_name: op.into(),
        compose_result: None,
        topology_status: TopologyProofStatus::None,
        payload_hash: hash_payload(op.as_bytes()),
        previous_event_hash: prev,
        equation_root: eq.hash,
    };
    chain
        .append(proof("wake", [0u8; 32]), 1, SubsystemKind::Process, 0)
        .unwrap();
    let head = chain.head().unwrap();
    chain
        .append(proof("schedule", head), 2, SubsystemKind::Process, 0)
        .unwrap();
    chain.events.remove(0);
    assert!(matches!(
        chain.verify(),
        Err(ChainError::Fork | ChainError::Reorder)
    ));
}

#[test]
fn syscall_write_records_statechain() {
    use kairos::kernel::state_proof::ObjectKind;
    use kairos::kernel::syscall::{dispatch, SyscallArgs, WRITE};

    let mut kernel = Kernel::new();
    let pid = kernel.spawn();
    kernel.wake(pid).unwrap();
    kernel.schedule(pid).unwrap();
    let mut io = kairos::kernel::syscall::IoBuffer::default();
    let args = SyscallArgs {
        text: "audit".into(),
        ..Default::default()
    };
    dispatch(&mut kernel, pid, &mut io, WRITE, &args).unwrap();
    let chain = kernel
        .statechain
        .chain(ObjectKind::Syscall, pid)
        .expect("syscall chain");
    assert_eq!(chain.events.len(), 1);
    assert_eq!(chain.events[0].proof.operation_name, "write");
}

#[test]
fn signed_checkpoint_seals_and_verifies() {
    let mut ledger = StatechainLedger::new();
    ledger
        .record_transition(
            ObjectKind::Process,
            0,
            SubsystemKind::Process,
            PhaseState::Phi0,
            PhaseState::Phi1,
            "wake",
            None,
            b"",
        )
        .unwrap();
    let checkpoint = ledger.seal_checkpoint().unwrap();
    assert_eq!(checkpoint.sequence, 0);
    assert_eq!(checkpoint.kernel_root, ledger.kernel_root());
    ledger.verify_checkpoints().unwrap();
    let dog = StatechainWatchdog::new();
    let report = dog.audit(&ledger);
    assert!(report.signed_checkpoints_ok);
    assert!(!report.tamper_detected);
}

#[test]
fn tampered_checkpoint_signature_fails_audit() {
    let mut ledger = StatechainLedger::new();
    ledger
        .record_transition(
            ObjectKind::Process,
            0,
            SubsystemKind::Process,
            PhaseState::Phi0,
            PhaseState::Phi1,
            "wake",
            None,
            b"",
        )
        .unwrap();
    ledger.seal_checkpoint().unwrap();
    let mut bad = ledger.latest_checkpoint().unwrap().clone();
    bad.signature = [9u8; 32];
    ledger.replace_checkpoint_for_audit_test(0, bad);
    assert!(ledger.verify_checkpoints().is_err());
    let dog = StatechainWatchdog::new();
    let report = dog.audit(&ledger);
    assert!(report.tamper_detected);
    assert!(!report.signed_checkpoints_ok);
}

#[test]
fn kernel_integration_audit_passes() {
    let mut kernel = Kernel::new();
    let pid = kernel.spawn();
    kernel.wake(pid).unwrap();
    kernel.schedule(pid).unwrap();
    let page = kernel.alloc_page();
    kernel.map_user(page).unwrap();

    let dog = StatechainWatchdog::new();
    let report = dog.audit(&kernel.statechain);
    assert!(!report.tamper_detected, "{}", report.message);
    assert!(report.equation_ok);
    assert!(report.chains_ok);
    assert!(report.kernel_root_ok);
}

#[test]
fn simulated_tamper_fails_audit() {
    let mut kernel = Kernel::new();
    let pid = kernel.spawn();
    kernel.wake(pid).unwrap();
    let dog = StatechainWatchdog::new();
    kernel.statechain.set_kernel_root_for_audit_test([1u8; 32]);
    let report = dog.audit(&kernel.statechain);
    assert!(report.tamper_detected);
}

#[test]
fn assoc_parity_flips_on_odd_defects_and_holds_on_even() {
    let mut kernel = Kernel::new();
    let pid = kernel.spawn();
    kernel.wake(pid).unwrap();
    kernel.schedule(pid).unwrap(); // (Phi0,Phi1,Phi2) is Same
    assert_eq!(kernel.statechain.reflection_defects(), 0);
    assert_eq!(kernel.statechain.assoc_parity(), 0);
    kernel.syscall(pid).unwrap(); // (Phi1,Phi2,Phi3) is Reflection
    assert_eq!(kernel.statechain.reflection_defects(), 1);
    assert_eq!(kernel.statechain.assoc_parity(), 1);
    let chain = kernel.statechain.chain(ObjectKind::Process, pid).unwrap();
    assert_eq!(chain.events.last().unwrap().assoc_parity, 1);
    kernel.kreturn(pid).unwrap(); // (Phi2,Phi3,Phi1) is Same
    assert_eq!(kernel.statechain.assoc_parity(), 1);
    kernel.schedule(pid).unwrap(); // (Phi3,Phi1,Phi2) is Reflection
    assert_eq!(kernel.statechain.reflection_defects(), 2);
    assert_eq!(kernel.statechain.assoc_parity(), 0);
    let chain = kernel.statechain.chain(ObjectKind::Process, pid).unwrap();
    assert_eq!(chain.events.last().unwrap().assoc_parity, 0);
    kernel.statechain.verify_all().unwrap();
}

#[test]
fn same_triples_never_change_assoc_parity() {
    let mut kernel = Kernel::new();
    let pid = kernel.spawn();
    kernel.wake(pid).unwrap();
    kernel.exit(pid).unwrap(); // (Phi0,Phi1,Phi0) is Same
    kernel.wake(pid).unwrap(); // (Phi1,Phi0,Phi1) is Same
    kernel.exit(pid).unwrap(); // (Phi0,Phi1,Phi0) is Same
    assert_eq!(kernel.statechain.reflection_defects(), 0);
    assert_eq!(kernel.statechain.assoc_parity(), 0);
    let chain = kernel.statechain.chain(ObjectKind::Process, pid).unwrap();
    assert!(chain.events.iter().all(|e| e.assoc_parity == 0));
}

#[test]
fn tampered_assoc_parity_fails_audit() {
    let mut kernel = Kernel::new();
    let pid = kernel.spawn();
    kernel.wake(pid).unwrap();
    let dog = StatechainWatchdog::new();
    assert!(!dog.audit(&kernel.statechain).tamper_detected);
    kernel
        .statechain
        .flip_assoc_parity_for_audit_test(ObjectKind::Process, pid, 0);
    let report = dog.audit(&kernel.statechain);
    assert!(report.tamper_detected);
    assert!(!report.chains_ok);
}

#[test]
fn merkle_odd_leaf_pairs_with_self() {
    let a = [1u8; 32];
    let b = [2u8; 32];
    let c = [3u8; 32];
    let d = [4u8; 32];
    let root_three = merkle_root(&[a, b, c]);
    let root_four = merkle_root(&[a, b, c, d]);
    assert_ne!(root_three, root_four);
}

#[test]
fn subsystem_root_changes_on_process_transition() {
    let mut ledger = StatechainLedger::new();
    ledger
        .record_transition(
            ObjectKind::Process,
            0,
            SubsystemKind::Process,
            PhaseState::Phi0,
            PhaseState::Phi1,
            "wake",
            None,
            b"",
        )
        .unwrap();
    let h1 = ledger
        .chain(ObjectKind::Process, 0)
        .and_then(|c| c.head())
        .unwrap();
    ledger
        .record_transition(
            ObjectKind::Process,
            0,
            SubsystemKind::Process,
            PhaseState::Phi1,
            PhaseState::Phi2,
            "schedule",
            None,
            b"",
        )
        .unwrap();
    let h2 = ledger
        .chain(ObjectKind::Process, 0)
        .and_then(|c| c.head())
        .unwrap();
    assert_ne!(h1, h2);
}

#[test]
fn kernel_state_root_changes_with_subsystem() {
    let eq = compute_equation_root();
    let h1 = [9u8; 32];
    let root1 = compute_kernel_state_root(&eq.hash, &[h1], &[], &[], &[], &[], &[], &[]);
    let h2 = [8u8; 32];
    let root2 = compute_kernel_state_root(&eq.hash, &[h2], &[], &[], &[], &[], &[], &[]);
    assert_ne!(root1.hash, root2.hash);
}

// Public constants are part of equation root — changing ALPHA would change root in a rebuild.
#[test]
fn alpha_constant_is_fixed_in_build() {
    assert!((ALPHA - 7.2973525693e-3).abs() < 1e-12);
}
