//! Protocol layer — encoder, topology scan, GPL checksum.

use kairos::core::gphase_core::{compose, PhaseState, TopologyKind};
use kairos::protocol::encoder::{bytes_to_phase, phase_to_bytes, round_trip};
use kairos::protocol::{
    bytes_stream_checksum, chi_normalized_pair, gpl_checksum, gpl_verify, reflection_count,
    scan_non_overlapping_triples, scan_sliding_triples,
};

#[test]
fn encoder_round_trip() {
    let data = [0x1A, 0x2B, 0xFF];
    assert_eq!(round_trip(&data).unwrap(), data);
}

#[test]
fn gpl_checksum_matches_python_reference() {
    let states = [
        PhaseState::Phi0,
        PhaseState::Phi1,
        PhaseState::Phi2,
        PhaseState::Phi3,
        PhaseState::Phi1,
        PhaseState::Phi2,
    ];
    let (acc, chi) = gpl_checksum(&states);
    assert_eq!(acc, PhaseState::Phi3);
    assert!(chi.abs() < 1e-12);
    assert!(gpl_verify(&states, PhaseState::Phi3, 0.0));
}

#[test]
fn bytes_checksum_matches_phase_checksum() {
    let states = vec![
        PhaseState::Phi0,
        PhaseState::Phi1,
        PhaseState::Phi2,
        PhaseState::Phi3,
    ];
    let bytes = phase_to_bytes(&states).unwrap();
    let (acc, chi) = bytes_stream_checksum(&bytes);
    assert_eq!(acc, gpl_checksum(&states).0);
    assert!((chi - gpl_checksum(&states).1).abs() < 1e-12);
}

#[test]
fn inner_pairs_separate_by_normalized_chi() {
    let pairs = [
        (PhaseState::Phi1, PhaseState::Phi1),
        (PhaseState::Phi1, PhaseState::Phi2),
        (PhaseState::Phi2, PhaseState::Phi2),
    ];
    let norms: Vec<_> = pairs
        .iter()
        .map(|&(a, b)| chi_normalized_pair(a, b))
        .collect();
    assert!((norms[0] + 1.0).abs() < 1e-9);
    assert!(norms[1].abs() < 1e-9);
    assert!((norms[2] - 1.0).abs() < 1e-9);
    for &(a, b) in &pairs {
        assert_eq!(compose(a, b), PhaseState::Phi3);
    }
}

#[test]
fn sliding_topology_counts_reflections() {
    let stream = bytes_to_phase(&[0x6D]); // arbitrary byte expands to four states
    let events = scan_sliding_triples(&stream);
    assert_eq!(events.len(), stream.len() - 2);
    let _ = reflection_count(&stream);
    let blocked = scan_non_overlapping_triples(&stream);
    assert!(blocked.len() <= events.len());
}

#[test]
fn topology_scan_finds_reflection_triple() {
    let states = [PhaseState::Phi1, PhaseState::Phi2, PhaseState::Phi3];
    let events = scan_sliding_triples(&states);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, TopologyKind::Reflection);
}
