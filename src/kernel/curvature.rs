//! Causal-curvature watchdog statistic (Avenue 8,
//! `docs/EXPLORATION_ROADMAP.md`).
//!
//! "Causal curvature" (`docs/THEOREMS_2026-07-23.md`, `docs/paper/
//! OSF_GPL_MATHEMATICS.md` §5/§7) is the reflection-defect rate: the
//! fraction of associator evaluations where left- and right-grouping
//! disagree by the reflection ρ. In the abstract ring-CA model
//! (`scripts/research/gpl_program.py` §2) that rate is measured directly
//! over all 64 triples and settles at ≈0.22-0.26. In the hosted OS, the
//! only reflection defects that actually occur are the ones logged by
//! [`crate::kernel::process::Process::check_topology`] on real Kairos-state
//! transition history — and the OS's legal transition graph
//! ([`crate::kernel::process::VALID_TRANSITIONS`]) only reaches 12 of the
//! 64 possible triples, of which exactly 2 are Reflection-kind (see the
//! Avenue 8 report for the derivation). The two baselines are related but
//! **not the same object**; this module measures the OS one directly
//! rather than assuming the CA number applies.
//!
//! [`CurvatureLog`] is a live, rolling per-tick counter: one sample per
//! kernel "tick" (a [`crate::kernel::Kernel::advance_chronos`] boundary,
//! which fires once per shell command and once per timer preemption in the
//! hosted runtime — see `src/runtime/host.rs`). Each sample is the delta of
//! [`crate::kernel::scheduler::Scheduler::reflection_count`] since the
//! previous tick. No thresholds are hardcoded here beyond the watchdog's
//! own leave-one-out rolling z-score; the alert threshold and its measured
//! false-positive rate live in the `curvature` shell command and the
//! Avenue 8 report, not in this module.

use crate::platform::Vec;

/// Number of recent per-tick samples kept for the rolling window statistic.
pub const ROLLING_WINDOW: usize = 64;

#[inline]
fn fsqrt(x: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        x.sqrt()
    }
    #[cfg(all(feature = "baremetal", not(feature = "std")))]
    {
        libm::sqrt(x)
    }
}

/// Per-tick reflection-defect log with rolling and lifetime statistics.
#[derive(Debug, Clone)]
pub struct CurvatureLog {
    /// Number of ticks recorded so far.
    pub ticks: u64,
    /// Lifetime total reflection count as of the last recorded tick
    /// (used only to compute the next delta).
    pub last_total: usize,
    /// Ring buffer of the last `ROLLING_WINDOW` per-tick defect deltas,
    /// oldest first.
    pub window: Vec<u32>,
    /// Lifetime sum of per-tick deltas (for the reference lifetime mean).
    pub lifetime_sum: u64,
    /// Lifetime sum of squared per-tick deltas (for the reference lifetime
    /// variance).
    pub lifetime_sumsq: u64,
    /// Lifetime tick count (mirrors `ticks`, kept separate so the struct
    /// stays simple to reason about independently).
    pub lifetime_count: u64,
}

impl Default for CurvatureLog {
    fn default() -> Self {
        Self::new()
    }
}

impl CurvatureLog {
    pub fn new() -> Self {
        Self {
            ticks: 0,
            last_total: 0,
            window: Vec::new(),
            lifetime_sum: 0,
            lifetime_sumsq: 0,
            lifetime_count: 0,
        }
    }

    /// Record one tick given the current lifetime total reflection-defect
    /// count (`scheduler.reflection_count()`). Returns the per-tick delta
    /// just recorded.
    pub fn record_tick(&mut self, total_reflections: usize) -> u32 {
        let delta = total_reflections.saturating_sub(self.last_total) as u32;
        self.last_total = total_reflections;
        self.ticks += 1;

        if self.window.len() == ROLLING_WINDOW {
            self.window.remove(0);
        }
        self.window.push(delta);

        self.lifetime_sum += delta as u64;
        self.lifetime_sumsq += (delta as u64) * (delta as u64);
        self.lifetime_count += 1;

        delta
    }

    pub fn latest(&self) -> Option<u32> {
        self.window.last().copied()
    }

    pub fn window_len(&self) -> usize {
        self.window.len()
    }

    /// Mean and (population) standard deviation over the full current
    /// rolling window.
    pub fn window_stats(&self) -> (f64, f64) {
        let n = self.window.len();
        if n == 0 {
            return (0.0, 0.0);
        }
        let sum: u64 = self.window.iter().map(|&v| v as u64).sum();
        let mean = sum as f64 / n as f64;
        let var = self
            .window
            .iter()
            .map(|&v| {
                let d = v as f64 - mean;
                d * d
            })
            .sum::<f64>()
            / n as f64;
        (mean, fsqrt(var))
    }

    /// Rolling z-score of the latest tick against the mean/std of the rest
    /// of the window (leave-one-out), so a single injected spike does not
    /// inflate its own baseline. Returns `None` if there are fewer than 2
    /// samples.
    pub fn rolling_z(&self) -> Option<f64> {
        let n = self.window.len();
        if n < 2 {
            return None;
        }
        let latest = *self.window.last().unwrap() as f64;
        let rest = &self.window[..n - 1];
        let sum: f64 = rest.iter().map(|&v| v as f64).sum();
        let mean = sum / rest.len() as f64;
        let var = rest
            .iter()
            .map(|&v| {
                let d = v as f64 - mean;
                d * d
            })
            .sum::<f64>()
            / rest.len() as f64;
        let std = fsqrt(var);
        if std < 1e-9 {
            // Degenerate (constant) baseline: any deviation is arbitrarily
            // significant. Report a large finite sentinel rather than
            // inf/NaN so callers can format it safely.
            if (latest - mean).abs() < 1e-9 {
                return Some(0.0);
            }
            return Some(if latest > mean { 1.0e6 } else { -1.0e6 });
        }
        Some((latest - mean) / std)
    }

    /// Mean and (population) standard deviation over the full lifetime of
    /// the log (reference only; the live watchdog uses the rolling window).
    pub fn lifetime_stats(&self) -> (f64, f64) {
        if self.lifetime_count == 0 {
            return (0.0, 0.0);
        }
        let n = self.lifetime_count as f64;
        let mean = self.lifetime_sum as f64 / n;
        let var = (self.lifetime_sumsq as f64 / n) - mean * mean;
        (mean, fsqrt(var.max(0.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_deltas_not_totals() {
        let mut log = CurvatureLog::new();
        assert_eq!(log.record_tick(0), 0);
        assert_eq!(log.record_tick(3), 3);
        assert_eq!(log.record_tick(3), 0);
        assert_eq!(log.record_tick(10), 7);
        assert_eq!(log.ticks, 4);
    }

    #[test]
    fn window_caps_at_rolling_window() {
        let mut log = CurvatureLog::new();
        for i in 0..(ROLLING_WINDOW + 10) {
            log.record_tick(i);
        }
        assert_eq!(log.window_len(), ROLLING_WINDOW);
        assert_eq!(log.ticks, (ROLLING_WINDOW + 10) as u64);
    }

    #[test]
    fn rolling_z_flags_a_spike() {
        let mut log = CurvatureLog::new();
        let mut total = 0usize;
        for _ in 0..20 {
            total += 1;
            log.record_tick(total);
        }
        total += 500;
        log.record_tick(total);
        let z = log.rolling_z().expect("enough samples");
        assert!(z > 3.29, "expected a large z-score, got {z}");
    }
}
