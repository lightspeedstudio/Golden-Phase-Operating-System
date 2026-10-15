//! PhaseLink decoder.

use crate::kernel::phaselink::error::PhaseLinkError;
use crate::kernel::phaselink::frame::decode_frame;
use crate::kernel::phaselink::types::PhaseLinkFrame;

pub struct PhaseLinkDecoder;

impl PhaseLinkDecoder {
    pub fn decode_frame(bytes: &[u8]) -> Result<PhaseLinkFrame, PhaseLinkError> {
        decode_frame(bytes)
    }
}
