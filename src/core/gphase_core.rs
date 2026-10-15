//! Golden Phase core algebra — single source of truth for compose and topology.
//!
//! States derive from `y^4 - phi*y^2 + alpha = 0` mapped onto `[0, pi/phi]`.
//! No higher layer may reimplement compose.

use core::f64::consts::PI;

#[cfg(feature = "baremetal")]
use spin::Once;
#[cfg(feature = "std")]
use std::sync::OnceLock;

/// Fine-structure constant: the kernel's one empirical parameter.
pub const ALPHA: f64 = 7.2973525693e-3;

/// Four discrete **Kairos States** on the bounded rail.
///
/// Enum variants keep historical `PhiN` names for ABI/source stability.
/// User-facing labels are `Kairos0`..`Kairos3` via [`Self::kairos_label`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PhaseState {
    Phi0 = 0,
    Phi1 = 1,
    Phi2 = 2,
    Phi3 = 3,
}

/// Alias: product terminology for [`PhaseState`].
pub type KairosState = PhaseState;

impl PhaseState {
    pub const ALL: [PhaseState; 4] = [
        PhaseState::Phi0,
        PhaseState::Phi1,
        PhaseState::Phi2,
        PhaseState::Phi3,
    ];

    pub fn from_u8(n: u8) -> Option<Self> {
        match n {
            0 => Some(PhaseState::Phi0),
            1 => Some(PhaseState::Phi1),
            2 => Some(PhaseState::Phi2),
            3 => Some(PhaseState::Phi3),
            _ => None,
        }
    }

    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// User-facing Kairos State label.
    pub const fn kairos_label(self) -> &'static str {
        match self {
            PhaseState::Phi0 => "Kairos0",
            PhaseState::Phi1 => "Kairos1",
            PhaseState::Phi2 => "Kairos2",
            PhaseState::Phi3 => "Kairos3",
        }
    }

    /// Compact Φ glyph label.
    pub const fn phi_glyph(self) -> &'static str {
        match self {
            PhaseState::Phi0 => "Φ0",
            PhaseState::Phi1 => "Φ1",
            PhaseState::Phi2 => "Φ2",
            PhaseState::Phi3 => "Φ3",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopologyKind {
    Same,
    Reflection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopologyResult {
    pub kind: TopologyKind,
    pub left: PhaseState,
    pub right: PhaseState,
}

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

#[inline]
pub fn phi() -> f64 {
    (1.0 + fsqrt(5.0)) / 2.0
}

#[inline]
pub fn rail() -> f64 {
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
    #[cfg(feature = "baremetal")]
    {
        static PARAMS: Once<Params> = Once::new();
        PARAMS.call_once(compute_params)
    }
    #[cfg(all(feature = "std", not(feature = "baremetal")))]
    {
        static PARAMS: OnceLock<Params> = OnceLock::new();
        PARAMS.get_or_init(compute_params)
    }
}

#[inline]
fn fold(x: f64, rail: f64) -> f64 {
    if x <= rail {
        x
    } else {
        2.0 * rail - x
    }
}

#[inline]
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

/// `snap(fold(states[a] + states[b]))` — the OS arbitration gate.
pub fn compose(a: PhaseState, b: PhaseState) -> PhaseState {
    let p = params();
    let raw = p.states[a.as_u8() as usize] + p.states[b.as_u8() as usize];
    snap(fold(raw, p.rail), &p.bounds)
}

/// Reflection map `rho(x) = 3 - x`, equivalent to `compose(Phi3, x)`.
pub fn reflect(a: PhaseState) -> PhaseState {
    PhaseState::from_u8(3 - a.as_u8()).expect("reflect on valid state")
}

pub fn left_group(a: PhaseState, b: PhaseState, c: PhaseState) -> PhaseState {
    compose(compose(a, b), c)
}

pub fn right_group(a: PhaseState, b: PhaseState, c: PhaseState) -> PhaseState {
    compose(a, compose(b, c))
}

pub fn topology_status(a: PhaseState, b: PhaseState, c: PhaseState) -> TopologyResult {
    let left = left_group(a, b, c);
    let right = right_group(a, b, c);
    if right == left {
        TopologyResult {
            kind: TopologyKind::Same,
            left,
            right,
        }
    } else if right == reflect(left) {
        TopologyResult {
            kind: TopologyKind::Reflection,
            left,
            right,
        }
    } else {
        panic!(
            "topology error for ({:?},{:?},{:?}): L={:?} R={:?}",
            a, b, c, left, right
        );
    }
}

/// `{Phi0, Phi1} -> 0`, `{Phi2, Phi3} -> 1`.
pub fn threshold_sign(a: PhaseState) -> u8 {
    if matches!(a, PhaseState::Phi0 | PhaseState::Phi1) {
        0
    } else {
        1
    }
}

/// `{Phi0, Phi3} -> 1` (rail), `{Phi1, Phi2} -> 0` (inner).
pub fn threshold_energy(a: PhaseState) -> u8 {
    if matches!(a, PhaseState::Phi0 | PhaseState::Phi3) {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: [[u8; 4]; 4] = [[0, 1, 2, 3], [1, 3, 3, 2], [2, 3, 3, 1], [3, 2, 1, 0]];

    #[test]
    fn compose_matches_table() {
        for a in PhaseState::ALL {
            for b in PhaseState::ALL {
                assert_eq!(
                    compose(a, b).as_u8(),
                    TABLE[a.as_u8() as usize][b.as_u8() as usize],
                    "compose({a:?},{b:?})"
                );
            }
        }
    }

    #[test]
    fn reflect_equals_compose_phi3() {
        for x in PhaseState::ALL {
            assert_eq!(reflect(x), compose(PhaseState::Phi3, x));
        }
    }

    #[test]
    fn topology_64_triples() {
        let mut same = 0;
        let mut reflection = 0;
        for a in PhaseState::ALL {
            for b in PhaseState::ALL {
                for c in PhaseState::ALL {
                    match topology_status(a, b, c).kind {
                        TopologyKind::Same => same += 1,
                        TopologyKind::Reflection => reflection += 1,
                    }
                }
            }
        }
        assert_eq!(same, 52);
        assert_eq!(reflection, 12);
    }
}
