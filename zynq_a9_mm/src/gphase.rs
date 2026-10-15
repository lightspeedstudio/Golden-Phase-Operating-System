//! Golden Phase compose — same algebra as `kairos::core::gphase_core`.

use core::f64::consts::PI;
use spin::Once;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PhaseState {
    Phi0 = 0,
    Phi1 = 1,
    Phi2 = 2,
    Phi3 = 3,
}

impl PhaseState {
    pub fn from_u8(n: u8) -> Option<Self> {
        match n {
            0 => Some(Self::Phi0),
            1 => Some(Self::Phi1),
            2 => Some(Self::Phi2),
            3 => Some(Self::Phi3),
            _ => None,
        }
    }

    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

pub const ALPHA: f64 = 7.2973525693e-3;

fn fsqrt(x: f64) -> f64 {
    libm::sqrt(x)
}

fn phi() -> f64 {
    (1.0 + fsqrt(5.0)) / 2.0
}

fn rail() -> f64 {
    PI / phi()
}

struct Params {
    rail: f64,
    states: [f64; 4],
    bounds: [f64; 3],
}

fn compute_params() -> Params {
    let phi = phi();
    let rail = rail();
    let discriminant = phi * phi - 4.0 * ALPHA;
    let sqrt_disc = fsqrt(discriminant);
    let y2_outer = (phi + sqrt_disc) / 2.0;
    let y2_inner = (phi - sqrt_disc) / 2.0;
    let s0 = -fsqrt(y2_outer);
    let s1 = -fsqrt(y2_inner);
    let s2 = fsqrt(y2_inner);
    let s3 = fsqrt(y2_outer);
    let to_rail = |s: f64| (s - s0) / (s3 - s0) * rail;
    let states = [to_rail(s0), to_rail(s1), to_rail(s2), to_rail(s3)];
    let bounds = [
        (states[0] + states[1]) / 2.0,
        (states[1] + states[2]) / 2.0,
        (states[2] + states[3]) / 2.0,
    ];
    Params {
        rail,
        states,
        bounds,
    }
}

fn params() -> &'static Params {
    static PARAMS: Once<Params> = Once::new();
    PARAMS.call_once(compute_params)
}

fn fold(x: f64, rail: f64) -> f64 {
    if x <= rail {
        x
    } else {
        2.0 * rail - x
    }
}

fn snap(x: f64, bounds: &[f64; 3]) -> PhaseState {
    if x <= bounds[0] {
        PhaseState::Phi0
    } else if x <= bounds[1] {
        PhaseState::Phi1
    } else if x <= bounds[2] {
        PhaseState::Phi2
    } else {
        PhaseState::Phi3
    }
}

pub fn compose(a: PhaseState, b: PhaseState) -> PhaseState {
    let p = params();
    let raw = p.states[a.as_u8() as usize] + p.states[b.as_u8() as usize];
    snap(fold(raw, p.rail), &p.bounds)
}
