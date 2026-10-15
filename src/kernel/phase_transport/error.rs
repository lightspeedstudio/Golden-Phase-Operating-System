//! Transport error and status types.

use crate::platform::String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    NotFound,
    BusyPort,
    InvalidDestination,
    DuplicatePacketId,
    ZeroTtl,
    BadChecksum,
    LaneQuarantined,
    CarrierLost,
    CarrierDuplicated,
    CarrierSpreading,
    CarrierCorrupt,
    PositionMismatch {
        expected: (usize, usize),
        got: (usize, usize),
    },
    Timeout,
    TraceMismatch,
    TamperDetected,
    InvalidOpcode,
    Quarantined,
    Chain(String),
}

impl core::fmt::Display for TransportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}
