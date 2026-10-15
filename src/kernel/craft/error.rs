//! Craft fixture errors.

use crate::kernel::statechain::ChainError;
use crate::kernel::statechain_checkpoint::CheckpointError;
use crate::platform::String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CraftError {
    NotFound,
    Quarantined,
    GuardRejected(&'static str),
    IllegalTransition(&'static str),
    Chain(String),
    Checkpoint(String),
    Replay(String),
    ProofIncomplete(&'static str),
    InjectRequiresReason,
    UnknownCommand,
}

impl CraftError {
    pub fn as_str(&self) -> String {
        match self {
            Self::NotFound => "object not found".into(),
            Self::Quarantined => "quarantined — command rejected".into(),
            Self::GuardRejected(s) => format!("guard rejected: {s}"),
            Self::IllegalTransition(s) => format!("illegal transition: {s}"),
            Self::Chain(s) => format!("statechain: {s}"),
            Self::Checkpoint(s) => format!("checkpoint: {s}"),
            Self::Replay(s) => format!("replay: {s}"),
            Self::ProofIncomplete(s) => format!("proof incomplete: {s}"),
            Self::InjectRequiresReason => "fault inject requires --reason".into(),
            Self::UnknownCommand => "unknown craft command".into(),
        }
    }
}

impl core::fmt::Display for CraftError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<ChainError> for CraftError {
    fn from(e: ChainError) -> Self {
        CraftError::Chain(format!("{e:?}"))
    }
}

impl From<CheckpointError> for CraftError {
    fn from(e: CheckpointError) -> Self {
        CraftError::Checkpoint(format!("{e:?}"))
    }
}
