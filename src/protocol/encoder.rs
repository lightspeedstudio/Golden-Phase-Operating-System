//! Byte ↔ phase-state codec (four states per byte, MSB pair first).

use crate::core::gphase_core::PhaseState;
use crate::platform::Vec;
use core::fmt;

const SHIFTS: [u8; 4] = [6, 4, 2, 0];

pub fn bytes_to_phase(data: &[u8]) -> Vec<PhaseState> {
    let mut states = Vec::with_capacity(data.len() * 4);
    for byte in data {
        for &shift in &SHIFTS {
            let n = (byte >> shift) & 0b11;
            states.push(PhaseState::from_u8(n).expect("two-bit phase"));
        }
    }
    states
}

pub fn phase_to_bytes(states: &[PhaseState]) -> Result<Vec<u8>, EncodeError> {
    if !states.len().is_multiple_of(4) {
        return Err(EncodeError::InvalidLength(states.len()));
    }
    let mut out = Vec::with_capacity(states.len() / 4);
    for chunk in states.chunks_exact(4) {
        let mut byte = 0u8;
        for (&state, &shift) in chunk.iter().zip(SHIFTS.iter()) {
            byte |= state.as_u8() << shift;
        }
        out.push(byte);
    }
    Ok(out)
}

pub fn round_trip(data: &[u8]) -> Result<Vec<u8>, EncodeError> {
    let states = bytes_to_phase(data);
    phase_to_bytes(&states)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    InvalidLength(usize),
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncodeError::InvalidLength(n) => write!(f, "phase length {n} is not a multiple of 4"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EncodeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_all_bytes() {
        for byte in 0u8..=255 {
            let data = [byte];
            assert_eq!(round_trip(&data).unwrap(), data);
        }
    }
}
