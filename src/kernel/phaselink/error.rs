//! PhaseLink error types.

use crate::platform::String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseLinkError {
    InvalidMagic,
    UnsupportedVersion,
    InvalidFlags,
    TruncatedFrame,
    TrailingData,
    InvalidLength,
    PayloadTooLarge,
    InvalidPhaseSymbol,
    InvalidPhaseStreamLength,
    InvalidObjectId,
    PayloadHashMismatch,
    FrameHashMismatch,
    EquationRootMismatch,
    UnknownNode,
    BootSessionMismatch,
    SequenceReplay,
    SequenceGap,
    PreviousFrameHashMismatch,
    InvalidTransition,
    InvalidProof,
    UnauthorizedOperation,
    GatewayDisabled,
    AnomAccessDenied,
    TransportUnavailable,
    SerializationFailure,
    BaremetalUnavailable,
}

impl PhaseLinkError {
    pub fn reason(&self) -> &'static str {
        match self {
            PhaseLinkError::InvalidMagic => "invalid magic",
            PhaseLinkError::UnsupportedVersion => "unsupported version",
            PhaseLinkError::InvalidFlags => "invalid flags",
            PhaseLinkError::TruncatedFrame => "truncated frame",
            PhaseLinkError::TrailingData => "trailing frame data",
            PhaseLinkError::InvalidLength => "invalid length",
            PhaseLinkError::PayloadTooLarge => "payload too large",
            PhaseLinkError::InvalidPhaseSymbol => "invalid phase symbol",
            PhaseLinkError::InvalidPhaseStreamLength => "invalid phase-stream length",
            PhaseLinkError::InvalidObjectId => "invalid object id",
            PhaseLinkError::PayloadHashMismatch => "payload hash mismatch",
            PhaseLinkError::FrameHashMismatch => "frame hash mismatch",
            PhaseLinkError::EquationRootMismatch => "equation root mismatch",
            PhaseLinkError::UnknownNode => "unknown node",
            PhaseLinkError::BootSessionMismatch => "boot session mismatch",
            PhaseLinkError::SequenceReplay => "sequence replay",
            PhaseLinkError::SequenceGap => "sequence gap",
            PhaseLinkError::PreviousFrameHashMismatch => "previous frame hash mismatch",
            PhaseLinkError::InvalidTransition => "invalid transition",
            PhaseLinkError::InvalidProof => "invalid proof",
            PhaseLinkError::UnauthorizedOperation => "unauthorized operation",
            PhaseLinkError::GatewayDisabled => "gateway disabled",
            PhaseLinkError::AnomAccessDenied => "anom access denied",
            PhaseLinkError::TransportUnavailable => "transport unavailable",
            PhaseLinkError::SerializationFailure => "serialization failure",
            PhaseLinkError::BaremetalUnavailable => "baremetal unavailable",
        }
    }

    pub fn as_string(&self) -> String {
        self.reason().into()
    }
}
