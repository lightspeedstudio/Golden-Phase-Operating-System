//! Craft subsystem objects and phase vocabulary.

use crate::core::gphase_core::PhaseState;
use crate::kernel::hash::StateHash;
use crate::platform::String;

pub const NAV_SOLVING_MAX_TICKS: u64 = 8;
pub const PROP_FIRING_MAX_TICKS: u64 = 5;
pub const POWER_LOAD_MAX_TICKS: u64 = 10;
pub const POWER_LOAD_MAX_MEASURED: u64 = 100;
pub const CURVATURE_Z_SUPPORT: f64 = 3.29;

pub const FDIR_OBJECT_ID: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftKind {
    Navigation,
    Propulsion,
    Power,
}

impl CraftKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Navigation => "navigation",
            Self::Propulsion => "propulsion",
            Self::Power => "power",
        }
    }

    pub fn object_id(self) -> u32 {
        match self {
            Self::Navigation => 1,
            Self::Propulsion => 2,
            Self::Power => 3,
        }
    }

    pub fn phase_label(self, state: PhaseState) -> &'static str {
        match (self, state) {
            (Self::Navigation, PhaseState::Phi0) => "absent",
            (Self::Navigation, PhaseState::Phi1) => "stale",
            (Self::Navigation, PhaseState::Phi2) => "solving",
            (Self::Navigation, PhaseState::Phi3) => "locked",
            (Self::Propulsion, PhaseState::Phi0) => "idle",
            (Self::Propulsion, PhaseState::Phi1) => "armed",
            (Self::Propulsion, PhaseState::Phi2) => "firing",
            (Self::Propulsion, PhaseState::Phi3) => "fault",
            (Self::Power, PhaseState::Phi0) => "off",
            (Self::Power, PhaseState::Phi1) => "ready",
            (Self::Power, PhaseState::Phi2) => "load",
            (Self::Power, PhaseState::Phi3) => "protected",
        }
    }
}

/// FDIR controller phase vocabulary (same GPL cycle, different mission meaning).
pub fn fdir_label(state: PhaseState) -> &'static str {
    match state {
        PhaseState::Phi0 => "normal",
        PhaseState::Phi1 => "detected",
        PhaseState::Phi2 => "isolated",
        PhaseState::Phi3 => "recovered_pending_proof",
    }
}

#[derive(Debug, Clone)]
pub struct CraftObject {
    pub id: u32,
    pub kind: CraftKind,
    pub state: PhaseState,
    pub ticks_in_state: u64,
    pub quarantined: bool,
    pub active_command: Option<String>,
    pub deadline_tick: Option<u64>,
    pub current_episode: Option<u64>,
    pub last_transition_hash: Option<StateHash>,
    pub measured_load: u64,
}

#[derive(Debug, Clone)]
pub struct FdirEpisode {
    pub id: u64,
    pub object_id: u32,
    pub reason: String,
    pub detected_tick: u64,
    pub dwell: u64,
    pub threshold: u64,
    pub curvature_z: f64,
    pub previous_event_hash: StateHash,
}
