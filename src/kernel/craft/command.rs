//! Craft semantic commands, GPL operands, and guards.

use crate::core::gphase_core::PhaseState;
use crate::kernel::craft::error::CraftError;
use crate::kernel::craft::state::CraftKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftCommand {
    NavStale,
    NavSolve,
    NavLock,
    NavReset,
    PropArm,
    PropFire,
    PropMarkFault,
    PropSafeReset,
    PowerReady,
    PowerLoad,
    PowerProtect,
    PowerOff,
    FdirDetect,
    FdirIsolate,
    FdirRecover,
    FdirProve,
}

impl CraftCommand {
    pub fn parse(subsystem: &str, op: &str) -> Result<Self, CraftError> {
        match (subsystem, op) {
            ("nav", "stale") => Ok(Self::NavStale),
            ("nav", "solve") => Ok(Self::NavSolve),
            ("nav", "lock") => Ok(Self::NavLock),
            ("nav", "reset") => Ok(Self::NavReset),
            ("prop", "arm") => Ok(Self::PropArm),
            ("prop", "fire") => Ok(Self::PropFire),
            ("prop", "mark_fault" | "fault") => Ok(Self::PropMarkFault),
            ("prop", "safe" | "safe_reset" | "idle") => Ok(Self::PropSafeReset),
            ("power", "ready") => Ok(Self::PowerReady),
            ("power", "load") => Ok(Self::PowerLoad),
            ("power", "protect") => Ok(Self::PowerProtect),
            ("power", "off") => Ok(Self::PowerOff),
            ("fdir", "detect") => Ok(Self::FdirDetect),
            ("fdir", "isolate") => Ok(Self::FdirIsolate),
            ("fdir", "recover") => Ok(Self::FdirRecover),
            ("fdir", "prove") => Ok(Self::FdirProve),
            _ => Err(CraftError::UnknownCommand),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::NavStale => "nav_stale",
            Self::NavSolve => "nav_solve",
            Self::NavLock => "nav_lock",
            Self::NavReset => "nav_reset",
            Self::PropArm => "prop_arm",
            Self::PropFire => "prop_fire",
            Self::PropMarkFault => "prop_mark_fault",
            Self::PropSafeReset => "forced_safe_recovery",
            Self::PowerReady => "power_ready",
            Self::PowerLoad => "power_load",
            Self::PowerProtect => "power_protect",
            Self::PowerOff => "power_off",
            Self::FdirDetect => "fdir_detect",
            Self::FdirIsolate => "fdir_isolate",
            Self::FdirRecover => "fdir_recover",
            Self::FdirProve => "fdir_prove",
        }
    }

    pub fn kind(self) -> Option<CraftKind> {
        match self {
            Self::NavStale | Self::NavSolve | Self::NavLock | Self::NavReset => {
                Some(CraftKind::Navigation)
            }
            Self::PropArm | Self::PropFire | Self::PropMarkFault | Self::PropSafeReset => {
                Some(CraftKind::Propulsion)
            }
            Self::PowerReady | Self::PowerLoad | Self::PowerProtect | Self::PowerOff => {
                Some(CraftKind::Power)
            }
            Self::FdirDetect | Self::FdirIsolate | Self::FdirRecover | Self::FdirProve => None,
        }
    }

    /// Universal mission-cycle operand: Φ1, Φ3, Φ1, Φ3.
    pub fn operand(self) -> PhaseState {
        match self {
            Self::NavStale
            | Self::NavLock
            | Self::PropArm
            | Self::PropMarkFault
            | Self::PowerReady
            | Self::PowerProtect
            | Self::FdirDetect
            | Self::FdirRecover => PhaseState::Phi1,
            Self::NavSolve
            | Self::NavReset
            | Self::PropFire
            | Self::PropSafeReset
            | Self::PowerLoad
            | Self::PowerOff
            | Self::FdirIsolate
            | Self::FdirProve => PhaseState::Phi3,
        }
    }

    pub fn required_state(self) -> PhaseState {
        match self {
            Self::NavStale | Self::PropArm | Self::PowerReady | Self::FdirDetect => {
                PhaseState::Phi0
            }
            Self::NavSolve | Self::PropFire | Self::PowerLoad | Self::FdirIsolate => {
                PhaseState::Phi1
            }
            Self::NavLock | Self::PropMarkFault | Self::PowerProtect | Self::FdirRecover => {
                PhaseState::Phi2
            }
            Self::NavReset | Self::PropSafeReset | Self::PowerOff | Self::FdirProve => {
                PhaseState::Phi3
            }
        }
    }

    pub fn expected_next(self) -> PhaseState {
        match self {
            Self::NavStale | Self::PropArm | Self::PowerReady | Self::FdirDetect => {
                PhaseState::Phi1
            }
            Self::NavSolve | Self::PropFire | Self::PowerLoad | Self::FdirIsolate => {
                PhaseState::Phi2
            }
            Self::NavLock | Self::PropMarkFault | Self::PowerProtect | Self::FdirRecover => {
                PhaseState::Phi3
            }
            Self::NavReset | Self::PropSafeReset | Self::PowerOff | Self::FdirProve => {
                PhaseState::Phi0
            }
        }
    }

    /// Ordinary mission commands are refused while quarantined.
    /// Recovery / FDIR commands remain legal under quarantine.
    pub fn allowed_while_quarantined(self) -> bool {
        matches!(
            self,
            Self::PropMarkFault
                | Self::PropSafeReset
                | Self::PowerProtect
                | Self::FdirDetect
                | Self::FdirIsolate
                | Self::FdirRecover
                | Self::FdirProve
        )
    }

    pub fn guard(self, current: PhaseState) -> Result<(), CraftError> {
        if current != self.required_state() {
            return Err(CraftError::GuardRejected(
                "required current state does not match command",
            ));
        }
        Ok(())
    }
}
