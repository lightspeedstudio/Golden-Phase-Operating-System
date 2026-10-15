use kairos::core::gphase_core::{reflect, PhaseState};
use kairos::kernel::crypto::phase_hash::{
    ph_content, ph_sentinel_manifest, tuple_hash,
};
use kairos::kernel::process::VALID_TRANSITIONS;
use kairos::kernel::sentinel::decision::{i1_compose_ok, i2_orbit_ok, phase_transition_legal};
use kairos::kernel::sentinel::types::{
    project_s, rights_subset, RIGHT_GRANT, RIGHT_READ, RIGHT_WRITE, SchedulerSnapshot,
    SentinelState,
};

#[test]
fn projection_is_s() {
    let q = SentinelState {
        s_t: PhaseState::Phi2,
        m_t: SchedulerSnapshot { epoch: 9 },
        v_t: RIGHT_READ | RIGHT_WRITE | RIGHT_GRANT,
    };
    assert_eq!(project_s(&q), PhaseState::Phi2);
    assert_eq!(project_s(&q), q.s_t);
}

#[test]
fn v_t_is_full_u32_rights_not_four_bit_label() {
    let v: u32 = RIGHT_READ | RIGHT_WRITE | RIGHT_GRANT;
    assert_eq!(v, 0b1011);
    assert_ne!(v & 0x0F, v >> 4);
    assert!(rights_subset(RIGHT_READ, v));
    assert!(!rights_subset(1 << 16, v));
}

#[test]
fn delta_s_uses_valid_transitions_and_i1_i2() {
    assert!(phase_transition_legal(PhaseState::Phi0, PhaseState::Phi1));
    assert!(!phase_transition_legal(PhaseState::Phi0, PhaseState::Phi2));
    assert_eq!(VALID_TRANSITIONS.len(), 4);
    assert!(i1_compose_ok(
        PhaseState::Phi0,
        PhaseState::Phi0,
        kairos::core::gphase_core::compose(PhaseState::Phi0, PhaseState::Phi0)
    ));
    let predicted = PhaseState::Phi1;
    assert!(i2_orbit_ok(predicted, predicted));
    assert!(i2_orbit_ok(predicted, reflect(predicted)));
    assert!(!i2_orbit_ok(PhaseState::Phi0, PhaseState::Phi1));
}

#[test]
fn tuple_hash_part_boundaries_on_sentinel_domain() {
    let a: &[&[u8]] = &[b"ab", b"c"];
    let b: &[&[u8]] = &[b"a", b"bc"];
    assert_ne!(
        tuple_hash("GPOS/PhaseSentinel/manifest/v1", a),
        tuple_hash("GPOS/PhaseSentinel/manifest/v1", b)
    );
    assert_ne!(ph_sentinel_manifest(b"ab"), ph_content(b"ab"));
}

#[test]
fn phasesentinel_is_not_in_default_features() {
    let cargo = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
    let default_line = cargo
        .lines()
        .find(|l| l.starts_with("default ="))
        .expect("default features");
    assert!(
        !default_line.contains("phasesentinel"),
        "phasesentinel leaked into default: {default_line}"
    );
}
