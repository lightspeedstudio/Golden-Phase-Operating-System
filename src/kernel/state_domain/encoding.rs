//! Phase-word encoding — base-4 digits over ΣΦ = {Φ0, Φ1, Φ2, Φ3}.

use crate::core::gphase_core::PhaseState;
use crate::platform::Vec;

pub type PhaseWord = Vec<PhaseState>;

pub fn word_length_for_state_count(n: usize) -> usize {
    if n <= 1 {
        return 1;
    }
    let mut length = 0usize;
    let mut cap = 1usize;
    while cap < n {
        length += 1;
        cap = cap.saturating_mul(4);
    }
    length.max(1)
}

pub fn capacity_for_word_length(length: usize) -> usize {
    4usize.saturating_pow(length as u32)
}

pub fn encode_state_index(index: usize, word_length: usize) -> PhaseWord {
    let mut digits = Vec::with_capacity(word_length);
    let mut n = index;
    for _ in 0..word_length {
        let digit = (n % 4) as u8;
        digits.push(PhaseState::from_u8(digit).expect("digit 0..3"));
        n /= 4;
    }
    digits.reverse();
    digits
}

pub fn decode_phase_word(word: &[PhaseState]) -> usize {
    let mut index = 0usize;
    for phase in word {
        index = index * 4 + phase.as_u8() as usize;
    }
    index
}

pub fn render_phase_word(word: &[PhaseState]) -> crate::platform::String {
    word.iter()
        .map(|p| p.phi_glyph())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn render_phase_word_ascii(word: &[PhaseState]) -> crate::platform::String {
    word.iter()
        .map(|p| p.kairos_label())
        .collect::<Vec<_>>()
        .join(" ")
}
