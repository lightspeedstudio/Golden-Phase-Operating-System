//! Soft Q16 EWMA score. Uncalibrated. Hard crypto/root/revocation/illegal-cap
//! failures bypass this score.

/// Uncalibrated α = 1/16 in Q16.
pub const SCORE_ALPHA_Q16: u32 = 4096;
pub const SCORE_ONE_Q16: u32 = 65536;
/// Uncalibrated soft threshold (0.5). Not a proven detector.
pub const SCORE_THRESHOLD_Q16: u32 = 32768;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoftScore {
    pub value_q16: u32,
}

impl SoftScore {
    pub fn new() -> Self {
        Self { value_q16: 0 }
    }

    /// `v <- v + α (sample - v)` in Q16. `sample_q16` is uncalibrated.
    pub fn update(&mut self, sample_q16: u32) {
        let v = self.value_q16 as i64;
        let s = sample_q16 as i64;
        let a = SCORE_ALPHA_Q16 as i64;
        let next = v + (a * (s - v)) / (SCORE_ONE_Q16 as i64);
        self.value_q16 = next.clamp(0, SCORE_ONE_Q16 as i64) as u32;
    }

    pub fn exceeds_threshold(&self) -> bool {
        self.value_q16 >= SCORE_THRESHOLD_Q16
    }
}

impl Default for SoftScore {
    fn default() -> Self {
        Self::new()
    }
}
