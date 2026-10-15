//! Shared formatting helpers for operator terminal views.

use crate::core::gphase_core::PhaseState;
use crate::platform::String;

pub const EQUATION_LINE: &str = "y^4 - phi*y^2 + alpha = 0";

pub fn short_hex(bytes: &[u8; 32]) -> String {
    bytes
        .iter()
        .take(4)
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
}

pub fn status_word(ok: bool) -> &'static str {
    if ok {
        "OK"
    } else {
        "FAIL"
    }
}

pub fn phi_label(state: PhaseState) -> &'static str {
    state.kairos_label()
}

pub fn process_mode(state: PhaseState) -> &'static str {
    match state {
        PhaseState::Phi0 => "sleep",
        PhaseState::Phi1 => "user",
        PhaseState::Phi2 => "active",
        PhaseState::Phi3 => "kernel",
    }
}

pub fn bump_phase(counts: &mut [usize; 4], state: PhaseState) {
    counts[state.as_u8() as usize] += 1;
}

pub fn format_phase_totals(title: &str, counts: [usize; 4]) -> String {
    format!(
        "{title}\n\
         Kairos0 sleep/free:      {}\n\
         Kairos1 ready/user:      {}\n\
         Kairos2 running/active:  {}\n\
         Kairos3 kernel/locked:   {}",
        counts[0], counts[1], counts[2], counts[3]
    )
}
