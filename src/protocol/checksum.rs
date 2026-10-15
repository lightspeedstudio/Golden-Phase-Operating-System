//! Phase-stream checksum — compose accumulation plus branch-sheet chi.

use crate::core::gphase_core::{compose, phi, PhaseState, ALPHA};
use crate::protocol::encoder::bytes_to_phase;

#[inline]
fn fsqrt(x: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        x.sqrt()
    }
    #[cfg(all(feature = "baremetal", not(feature = "std")))]
    {
        libm::sqrt(x)
    }
}

fn lambda_plus() -> f64 {
    let phi = phi();
    let delta = fsqrt(phi * phi - 4.0 * ALPHA);
    fsqrt((phi + delta) / 2.0)
}

fn lambda_minus() -> f64 {
    let phi = phi();
    let delta = fsqrt(phi * phi - 4.0 * ALPHA);
    fsqrt((phi - delta) / 2.0)
}

fn sqrt_alpha() -> f64 {
    fsqrt(ALPHA)
}

fn y_coord(state: PhaseState) -> f64 {
    match state {
        PhaseState::Phi0 => -lambda_plus(),
        PhaseState::Phi1 => -lambda_minus(),
        PhaseState::Phi2 => lambda_minus(),
        PhaseState::Phi3 => lambda_plus(),
    }
}

/// Branch swap `T(y) = sqrt(alpha) / y` for the second checksum sheet.
pub fn branch_swap_y(y: f64) -> f64 {
    sqrt_alpha() / y
}

pub fn state_sheet(state: PhaseState) -> f64 {
    branch_swap_y(y_coord(state))
}

/// Fold a phase stream with compose starting at Phi0.
pub fn accumulate_compose(states: &[PhaseState]) -> PhaseState {
    let mut acc = PhaseState::Phi0;
    for &state in states {
        acc = compose(acc, state);
    }
    acc
}

/// Two-layer GPL checksum: compose fold plus summed sheet weights.
pub fn gpl_checksum(states: &[PhaseState]) -> (PhaseState, f64) {
    let mut acc = PhaseState::Phi0;
    let mut chi = 0.0;
    for &state in states {
        acc = compose(acc, state);
        chi += state_sheet(state);
    }
    (acc, chi)
}

pub fn gpl_verify(states: &[PhaseState], expected_acc: PhaseState, expected_chi: f64) -> bool {
    let (acc, chi) = gpl_checksum(states);
    acc == expected_acc && (chi - expected_chi).abs() < 1e-12
}

pub fn phase_stream_checksum(states: &[PhaseState]) -> (PhaseState, f64) {
    gpl_checksum(states)
}

pub fn bytes_stream_checksum(data: &[u8]) -> (PhaseState, f64) {
    let states = bytes_to_phase(data);
    gpl_checksum(&states)
}

pub fn verify_bytes_checksum(data: &[u8], expected_acc: PhaseState, expected_chi: f64) -> bool {
    let (acc, chi) = bytes_stream_checksum(data);
    acc == expected_acc && (chi - expected_chi).abs() < 1e-12
}

/// Normalized chi for adjacent pairs — separates inner pairs collapsing to Phi3.
pub fn chi_normalized_pair(a: PhaseState, b: PhaseState) -> f64 {
    let norm = 2.0 * lambda_plus();
    (state_sheet(a) + state_sheet(b)) / norm
}
