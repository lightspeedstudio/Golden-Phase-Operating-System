//! Byte ↔ phase-stream bridge.

use crate::kernel::phaselink::error::PhaseLinkError;
use crate::kernel::phaselink::types::{PhaseStream, PhaseSymbol};
use crate::platform::Vec;
use crate::protocol::encoder::{bytes_to_phase, phase_to_bytes};

pub fn bytes_to_phase_stream(input: &[u8]) -> PhaseStream {
    PhaseStream {
        symbols: bytes_to_phase(input)
            .into_iter()
            .map(PhaseSymbol::from)
            .collect(),
    }
}

pub fn phase_stream_to_bytes(stream: &PhaseStream) -> Result<Vec<u8>, PhaseLinkError> {
    let states = stream
        .symbols
        .iter()
        .map(|s| s.as_phase_state())
        .collect::<Vec<_>>();
    phase_to_bytes(&states).map_err(|_| PhaseLinkError::InvalidPhaseStreamLength)
}

pub fn parse_phase_symbols(tokens: &[&str]) -> Result<PhaseStream, PhaseLinkError> {
    let mut symbols = Vec::with_capacity(tokens.len());
    for token in tokens {
        let sym = match *token {
            "Φ0" | "phi0" | "Phi0" | "kairos0" | "Kairos0" | "0" => PhaseSymbol::Phi0,
            "Φ1" | "phi1" | "Phi1" | "kairos1" | "Kairos1" | "1" => PhaseSymbol::Phi1,
            "Φ2" | "phi2" | "Phi2" | "kairos2" | "Kairos2" | "2" => PhaseSymbol::Phi2,
            "Φ3" | "phi3" | "Phi3" | "kairos3" | "Kairos3" | "3" => PhaseSymbol::Phi3,
            _ => return Err(PhaseLinkError::InvalidPhaseSymbol),
        };
        symbols.push(sym);
    }
    Ok(PhaseStream { symbols })
}

pub fn render_phase_stream(stream: &PhaseStream) -> crate::platform::String {
    stream
        .symbols
        .iter()
        .map(|p| match p {
            PhaseSymbol::Phi0 => "Φ0",
            PhaseSymbol::Phi1 => "Φ1",
            PhaseSymbol::Phi2 => "Φ2",
            PhaseSymbol::Phi3 => "Φ3",
        })
        .collect::<Vec<_>>()
        .join(" ")
}
