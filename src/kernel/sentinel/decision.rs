//! Sentinel decisions and phase `δ_S` using existing tables only.

use crate::core::gphase_core::{compose, reflect, PhaseState};
use crate::kernel::process::VALID_TRANSITIONS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SentinelDecision {
    Allow,
    Refuse,
    Quarantine,
    Isolate,
}

impl SentinelDecision {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Allow => 0,
            Self::Refuse => 1,
            Self::Quarantine => 2,
            Self::Isolate => 3,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "ALLOW",
            Self::Refuse => "REFUSE",
            Self::Quarantine => "QUARANTINE",
            Self::Isolate => "ISOLATE",
        }
    }
}

pub fn phase_transition_legal(from: PhaseState, to: PhaseState) -> bool {
    VALID_TRANSITIONS
        .iter()
        .find(|(s, _)| *s == from)
        .map(|(_, targets)| targets.contains(&to))
        .unwrap_or(false)
}

/// I1: observed next state equals `compose(prev, operand)`.
pub fn i1_compose_ok(prev: PhaseState, operand: PhaseState, observed: PhaseState) -> bool {
    compose(prev, operand) == observed
}

/// I2: observed value lies in the reflection orbit of the predicted value.
pub fn i2_orbit_ok(predicted: PhaseState, observed: PhaseState) -> bool {
    observed == predicted || observed == reflect(predicted)
}
