//! Lifecycle supervisor. Maps PhaseGuard Pass/Warning/Fault → VALID/SUSPECT/ISOLATE.
//! RECOVER is policy only — never automatic.

use crate::kernel::phasestack::types::PhaseStackError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardMap {
    Valid,
    Suspect,
    Isolate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupervisorPolicy {
    pub recover_enabled: bool,
}

impl Default for SupervisorPolicy {
    fn default() -> Self {
        Self {
            recover_enabled: false,
        }
    }
}

pub fn map_phaseguard(verdict: &str) -> GuardMap {
    match verdict {
        "Pass" => GuardMap::Valid,
        "Warning" => GuardMap::Suspect,
        "Fault" => GuardMap::Isolate,
        _ => GuardMap::Valid,
    }
}

#[cfg(feature = "phaseguard-research")]
pub fn map_live(
    v: crate::stateclock::phaseguard_live::PhaseGuardVerdict,
) -> GuardMap {
    use crate::stateclock::phaseguard_live::PhaseGuardVerdict;
    match v {
        PhaseGuardVerdict::Pass => GuardMap::Valid,
        PhaseGuardVerdict::Warning => GuardMap::Suspect,
        PhaseGuardVerdict::Fault => GuardMap::Isolate,
    }
}

#[cfg(not(feature = "phaseguard-research"))]
pub fn live_policy() -> GuardMap {
    GuardMap::Valid
}

pub fn recover(policy: SupervisorPolicy) -> Result<(), PhaseStackError> {
    if !policy.recover_enabled {
        return Err(PhaseStackError::AuthorityDenied);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_three_verdicts() {
        assert_eq!(map_phaseguard("Pass"), GuardMap::Valid);
        assert_eq!(map_phaseguard("Warning"), GuardMap::Suspect);
        assert_eq!(map_phaseguard("Fault"), GuardMap::Isolate);
    }

    #[test]
    fn recover_is_policy_only() {
        assert_eq!(
            recover(SupervisorPolicy {
                recover_enabled: false
            }),
            Err(PhaseStackError::AuthorityDenied)
        );
        recover(SupervisorPolicy {
            recover_enabled: true,
        })
        .unwrap();
    }
}
