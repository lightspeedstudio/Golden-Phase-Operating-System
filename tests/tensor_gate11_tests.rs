//! T11: `PH_tensor-proof` uses the real PhaseHash v1 tuple construction.
//! Changing M or V changes the proof without changing π_S. Replay of
//! sequence or BootSessionRoot is a different digest. Not authorization.
#![cfg(feature = "tensor")]

use kairos::core::gphase_core::PhaseState;
use kairos::kernel::crypto::phase_hash::{ph_content, ph_tensor_proof, TensorProofInput};
use kairos::kernel::tensor_state::{CapabilityClass, SchedulerPhase, TensorState};

fn proof(tensor: TensorState, sequence: u64, boot: [u8; 32]) -> [u8; 32] {
    ph_tensor_proof(&TensorProofInput {
        tensor: tensor.as_u8(),
        sequence,
        previous_root: [1u8; 32],
        boot_session_root: boot,
        authority_generation: 3,
        chronos: 10,
    })
}

#[test]
fn m_or_v_change_changes_proof_not_projected_s() {
    let a = TensorState::new(
        PhaseState::Phi1,
        SchedulerPhase::M0,
        CapabilityClass::from_u8(0).unwrap(),
    );
    let b = a.t_m();
    let c = a.t_v(0x0F);
    assert_eq!(a.project_root(), b.project_root());
    assert_eq!(a.project_root(), c.project_root());
    let boot = [2u8; 32];
    assert_ne!(proof(a, 1, boot), proof(b, 1, boot));
    assert_ne!(proof(a, 1, boot), proof(c, 1, boot));
}

#[test]
fn s_change_changes_projection_and_proof() {
    let a = TensorState::snapshot(PhaseState::Phi0, 0, 0);
    let b = a.t_s_compose(PhaseState::Phi1);
    assert_ne!(a.project_root(), b.project_root());
    let boot = [2u8; 32];
    assert_ne!(proof(a, 1, boot), proof(b, 1, boot));
}

#[test]
fn replay_sequence_or_boot_session_root_is_a_different_digest() {
    let t = TensorState::snapshot(PhaseState::Phi2, 1, 1);
    let boot = [7u8; 32];
    let p0 = proof(t, 1, boot);
    assert_ne!(p0, proof(t, 0, boot));
    assert_ne!(p0, proof(t, 1, [8u8; 32]));
}

#[test]
fn tensor_proof_domain_separates_from_ph_content() {
    let t = TensorState::snapshot(PhaseState::Phi0, 0, 0);
    let p = proof(t, 0, [0u8; 32]);
    assert_ne!(p, ph_content(&[t.as_u8()]));
}
