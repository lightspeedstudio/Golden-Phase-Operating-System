//! Input quarantine.

use crate::kernel::authority::StateHash;
use crate::kernel::phaselink::types::{NodeId, PHASELINK_MAX_QUARANTINE};
use crate::platform::Vec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuarantineReason {
    UnknownEquationRoot,
    UnknownNode,
    BootSessionChanged,
    SequenceGap,
    ReplayDetected,
    PreviousHashMismatch,
    InvalidTransition,
    InvalidProof,
    UnauthorizedOperation,
    AnomPolicyViolation,
    OversizedPayload,
    MalformedFrame,
}

impl QuarantineReason {
    pub fn label(&self) -> &'static str {
        match self {
            Self::UnknownEquationRoot => "unknown equation root",
            Self::UnknownNode => "unknown node",
            Self::BootSessionChanged => "boot session changed",
            Self::SequenceGap => "sequence gap",
            Self::ReplayDetected => "replay detected",
            Self::PreviousHashMismatch => "previous hash mismatch",
            Self::InvalidTransition => "invalid transition",
            Self::InvalidProof => "invalid proof",
            Self::UnauthorizedOperation => "unauthorized operation",
            Self::AnomPolicyViolation => "anom policy violation",
            Self::OversizedPayload => "oversized payload",
            Self::MalformedFrame => "malformed frame",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarantinedFrame {
    pub received_length: usize,
    pub source_hint: Option<NodeId>,
    pub calculated_hash: StateHash,
    pub reason: QuarantineReason,
    pub chronos_tick: u64,
    pub kairos_tick: u64,
}

#[derive(Debug, Default, Clone)]
pub struct QuarantineLog {
    pub entries: Vec<QuarantinedFrame>,
}

impl QuarantineLog {
    pub fn push(&mut self, frame: QuarantinedFrame) {
        if self.entries.len() >= PHASELINK_MAX_QUARANTINE {
            self.entries.remove(0);
        }
        self.entries.push(frame);
    }
}
