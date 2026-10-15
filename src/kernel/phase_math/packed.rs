//! Packed phase batches for future SIMD/GPU/FPGA paths.

use crate::core::gphase_core::PhaseState;
use crate::platform::Vec;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackedPhaseBatch {
    pub packed: Vec<u8>,
}

impl PackedPhaseBatch {
    pub fn pack(states: &[PhaseState]) -> Self {
        Self {
            packed: states.iter().map(|s| s.as_u8()).collect(),
        }
    }

    pub fn unpack(&self) -> Vec<PhaseState> {
        self.packed
            .iter()
            .filter_map(|b| PhaseState::from_u8(*b))
            .collect()
    }
}
