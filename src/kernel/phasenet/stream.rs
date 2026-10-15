//! PhaseStream — bytes ↔ Φ state pairs (00→Φ0 … 11→Φ3).

use crate::core::gphase_core::PhaseState;
use crate::platform::Vec;
use crate::protocol::encoder::{bytes_to_phase, phase_to_bytes, EncodeError};

pub use crate::protocol::encoder::EncodeError as PhaseStreamError;

pub type PhaseStream = Vec<PhaseState>;

pub fn encode_bytes(data: &[u8]) -> PhaseStream {
    bytes_to_phase(data)
}

pub fn decode_bytes(stream: &[PhaseState]) -> Result<Vec<u8>, EncodeError> {
    phase_to_bytes(stream)
}

pub fn render_stream(stream: &[PhaseState]) -> crate::platform::String {
    stream
        .iter()
        .map(|p| match p {
            PhaseState::Phi0 => "Φ0",
            PhaseState::Phi1 => "Φ1",
            PhaseState::Phi2 => "Φ2",
            PhaseState::Phi3 => "Φ3",
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn render_stream_compact(stream: &[PhaseState]) -> crate::platform::String {
    stream
        .iter()
        .map(|p| match p {
            PhaseState::Phi0 => "0",
            PhaseState::Phi1 => "1",
            PhaseState::Phi2 => "2",
            PhaseState::Phi3 => "3",
        })
        .collect::<Vec<_>>()
        .join("")
}

pub fn hex_encode(data: &[u8]) -> crate::platform::String {
    data.iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

pub fn hex_decode(hex: &str) -> Result<Vec<u8>, HexDecodeError> {
    let s = hex.trim();
    if !s.len().is_multiple_of(2) {
        return Err(HexDecodeError::OddLength);
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = from_hex_nibble(bytes[i])?;
        let lo = from_hex_nibble(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

fn from_hex_nibble(b: u8) -> Result<u8, HexDecodeError> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(HexDecodeError::InvalidNibble),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HexDecodeError {
    OddLength,
    InvalidNibble,
}

impl HexDecodeError {
    pub fn reason(&self) -> &'static str {
        match self {
            HexDecodeError::OddLength => "hex string must have even length",
            HexDecodeError::InvalidNibble => "invalid hex digit",
        }
    }
}
