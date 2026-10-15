//! Phase-state benchmark errors.

use crate::platform::String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseBenchmarkError {
    HostedOnly,
    InvalidTarget,
    ResultMismatch,
    ZeroDuration,
    InsufficientRuns,
    WorkerError(String),
}

impl PhaseBenchmarkError {
    pub fn reason(&self) -> String {
        match self {
            Self::HostedOnly => "phase-benchmark requires hosted std build (BigUint)".into(),
            Self::InvalidTarget => "invalid benchmark target".into(),
            Self::ResultMismatch => "engine results do not match".into(),
            Self::ZeroDuration => "zero-duration sample rejected".into(),
            Self::InsufficientRuns => "insufficient measured runs".into(),
            Self::WorkerError(msg) => msg.clone(),
        }
    }
}
