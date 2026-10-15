//! Binary bridge abstractions.

use crate::kernel::phaselink::error::PhaseLinkError;
use crate::kernel::phaselink::phase_stream::{bytes_to_phase_stream, phase_stream_to_bytes};
use crate::kernel::phaselink::types::PhaseStream;

pub trait BinaryDeviceBridge {
    fn receive_binary(&mut self, data: &[u8]) -> Result<PhaseStream, PhaseLinkError>;
    fn transmit_phase(
        &mut self,
        stream: &PhaseStream,
    ) -> Result<crate::platform::Vec<u8>, PhaseLinkError>;
}

#[derive(Debug, Default, Clone)]
pub struct HostedMemoryBridge;

impl BinaryDeviceBridge for HostedMemoryBridge {
    fn receive_binary(&mut self, data: &[u8]) -> Result<PhaseStream, PhaseLinkError> {
        Ok(bytes_to_phase_stream(data))
    }

    fn transmit_phase(
        &mut self,
        stream: &PhaseStream,
    ) -> Result<crate::platform::Vec<u8>, PhaseLinkError> {
        phase_stream_to_bytes(stream)
    }
}
