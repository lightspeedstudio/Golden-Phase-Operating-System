//! PhaseGuard mapping for identity (`kernel::identity`, Gate 10).
//!
//! ## Decision
//! There is no single `PhaseGuard` module with VALID/SUSPECT/RECOVER/ISOLATE
//! in this repository. The live analog is
//! [`crate::kernel::fault_syndrome::Health`] (`Valid`/`Suspect`/`Fault`/
//! `Isolate` — no `Recover`). Squeeze-only `phaseguard_live` under
//! `phaseguard-research` is research-gated and is not used here.
//!
//! Mapping (identity-local, does not rewrite `Health`):
//! - `Health::Valid` → [`PhaseGuardState::Valid`]
//! - `Health::Suspect` → [`PhaseGuardState::Suspect`]
//! - `Health::Fault` → [`PhaseGuardState::Recover`] (the spec's RECOVER;
//!   `Health` has no Recover variant)
//! - `Health::Isolate` → [`PhaseGuardState::Isolate`]
//!
//! `mod` elevation requires `Valid`. Root recovery-only operations
//! (`credential.unlock`, `session.close`) are also permitted in `Recover`.
//! `Suspect` and `Isolate` deny elevation.

use crate::kernel::fault_syndrome::Health;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseGuardState {
    Valid,
    Suspect,
    Recover,
    Isolate,
}

impl PhaseGuardState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Valid => "VALID",
            Self::Suspect => "SUSPECT",
            Self::Recover => "RECOVER",
            Self::Isolate => "ISOLATE",
        }
    }

    pub fn from_health(health: Health) -> Self {
        match health {
            Health::Valid => Self::Valid,
            Health::Suspect => Self::Suspect,
            Health::Fault => Self::Recover,
            Health::Isolate => Self::Isolate,
        }
    }

    pub fn allows_mod_elevation(self) -> bool {
        self == Self::Valid
    }

    pub fn allows_root_recovery(self) -> bool {
        matches!(self, Self::Valid | Self::Recover)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_fault_maps_to_recover_not_isolate() {
        assert_eq!(
            PhaseGuardState::from_health(Health::Fault),
            PhaseGuardState::Recover
        );
        assert_eq!(
            PhaseGuardState::from_health(Health::Isolate),
            PhaseGuardState::Isolate
        );
        assert!(PhaseGuardState::Valid.allows_mod_elevation());
        assert!(!PhaseGuardState::Recover.allows_mod_elevation());
        assert!(PhaseGuardState::Recover.allows_root_recovery());
        assert!(!PhaseGuardState::Isolate.allows_root_recovery());
    }
}
