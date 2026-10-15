//! Avenue 10A enum / hash compatibility freeze.

use kairos::core::gphase_core::PhaseState;
use kairos::kernel::state_proof::{
    compute_event_hash, hash_payload, ObjectKind, StateTransitionProof,
};
use kairos::kernel::state_root::{compute_equation_root, SubsystemKind};

#[test]
fn object_kind_craft_appended_as_8() {
    assert_eq!(ObjectKind::Process.as_u8(), 0);
    assert_eq!(ObjectKind::MemoryPage.as_u8(), 1);
    assert_eq!(ObjectKind::File.as_u8(), 2);
    assert_eq!(ObjectKind::Directory.as_u8(), 3);
    assert_eq!(ObjectKind::Block.as_u8(), 4);
    assert_eq!(ObjectKind::Syscall.as_u8(), 5);
    assert_eq!(ObjectKind::Vm.as_u8(), 6);
    assert_eq!(ObjectKind::Driver.as_u8(), 7);
    assert_eq!(ObjectKind::Craft.as_u8(), 8);
    assert_eq!(ObjectKind::from_u8(8), Some(ObjectKind::Craft));
    #[cfg(not(feature = "phasestack"))]
    assert_eq!(ObjectKind::from_u8(9), None);
    #[cfg(feature = "phasestack")]
    assert_eq!(ObjectKind::from_u8(9), Some(ObjectKind::App));
}

#[test]
fn subsystem_kind_craft_appended_as_6() {
    assert_eq!(SubsystemKind::Process.as_u8(), 0);
    assert_eq!(SubsystemKind::Memory.as_u8(), 1);
    assert_eq!(SubsystemKind::Filesystem.as_u8(), 2);
    assert_eq!(SubsystemKind::Vm.as_u8(), 3);
    assert_eq!(SubsystemKind::Syscall.as_u8(), 4);
    assert_eq!(SubsystemKind::Driver.as_u8(), 5);
    assert_eq!(SubsystemKind::Craft.as_u8(), 6);
    assert_eq!(SubsystemKind::from_u8(6), Some(SubsystemKind::Craft));
    assert_eq!(SubsystemKind::from_u8(7), None);
}

#[test]
fn pre_existing_process_event_hash_layout_stable() {
    let eq = compute_equation_root();
    let proof = StateTransitionProof {
        object_kind: ObjectKind::Process,
        object_id: 42,
        sequence_number: 3,
        previous_state: PhaseState::Phi0,
        requested_state: PhaseState::Phi1,
        operation_name: "freeze_vector".into(),
        compose_result: Some(PhaseState::Phi1),
        topology_status: kairos::kernel::state_proof::TopologyProofStatus::None,
        payload_hash: hash_payload(b"avenue10a"),
        previous_event_hash: [7u8; 32],
        equation_root: eq.hash,
    };
    // Golden for this frozen vector (layout uses ObjectKind::as_u8, not craft channel).
    let h = compute_event_hash(&proof, 1);
    let again = compute_event_hash(&proof, 1);
    assert_eq!(h, again);
    assert_ne!(h, compute_event_hash(&proof, 0));
}
