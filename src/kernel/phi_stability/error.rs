//! PHI-STABILITY error types.

use crate::platform::String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhiStabilityError {
    InvalidTarget,
    TargetTooLarge,
    IntegerOverflow,
    UnsupportedIntegerMode,
    InvalidPhaseTransition,
    ClockSequenceInvalid,
    KairosMismatch,
    ChronosTimeout,
    ScalarMismatch,
    BackendMismatch,
    ResultHashMismatch,
    PairHashMismatch,
    TrajectoryMismatch,
    MerkleMismatch,
    NetworkUnavailable,
    FrameDecodeFailed,
    SequenceReplay,
    PreviousFrameHashMismatch,
    EquationRootMismatch,
    RemoteVerificationFailed,
    QueueFull,
    TestNotFound,
    BaremetalUnsupported,
    NotInitialized,
    PhaseLink(String),
}

impl PhiStabilityError {
    pub fn reason(&self) -> String {
        match self {
            Self::InvalidTarget => "invalid target index".into(),
            Self::TargetTooLarge => "target exceeds baremetal integer capacity".into(),
            Self::IntegerOverflow => "integer overflow".into(),
            Self::UnsupportedIntegerMode => "unsupported integer mode".into(),
            Self::InvalidPhaseTransition => "invalid phase transition".into(),
            Self::ClockSequenceInvalid => "clock sequence invalid".into(),
            Self::KairosMismatch => "kairos event sequence mismatch".into(),
            Self::ChronosTimeout => "chronos timeout".into(),
            Self::ScalarMismatch => "scalar mismatch".into(),
            Self::BackendMismatch => "backend mismatch".into(),
            Self::ResultHashMismatch => "result hash mismatch".into(),
            Self::PairHashMismatch => "pair hash mismatch".into(),
            Self::TrajectoryMismatch => "trajectory mismatch".into(),
            Self::MerkleMismatch => "merkle mismatch".into(),
            Self::NetworkUnavailable => "network unavailable".into(),
            Self::FrameDecodeFailed => "frame decode failed".into(),
            Self::SequenceReplay => "sequence replay".into(),
            Self::PreviousFrameHashMismatch => "previous frame hash mismatch".into(),
            Self::EquationRootMismatch => "equation root mismatch".into(),
            Self::RemoteVerificationFailed => "remote verification failed".into(),
            Self::QueueFull => "queue full".into(),
            Self::TestNotFound => "test not found".into(),
            Self::BaremetalUnsupported => "baremetal unsupported".into(),
            Self::NotInitialized => "phaselink not initialized".into(),
            Self::PhaseLink(msg) => msg.clone(),
        }
    }
}
