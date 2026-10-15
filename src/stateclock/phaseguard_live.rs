//! PhaseGuard live-kernel observer (Gate 4) — read-only stateclock binding.
//!
//! Feature `phaseguard-research` only. Does **not** advance, reset, or alter
//! [`crate::stateclock::StateClock`]. Does **not** block or repair transitions.
//!
//! Logical tick source: **`kairos_tick`** (event / transition ordering). Chronos
//! is the fine-grained runtime clock and is recorded for evidence, but the
//! ten-position squeeze projection uses Kairos as the Gate-3B sequence analogue:
//!
//! ```text
//! phase_index = kairos_tick mod 10
//! theta       = phase_index × π/5
//! expected    = s* × cos(theta)
//! ```

use crate::core::gphase_core::PhaseState;
use crate::platform::Vec;
use crate::stateclock::phase_disk::{
    phase_index_from_logical_tick, phase_theta_deg, phase_theta_rad,
    predict_squeeze_from_logical_tick, SignedBranch, SQUEEZE_PRED_TOL,
};
use crate::stateclock::{StateClock, StateClockSnapshot};

/// Compact read-only observation at a transition request.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseGuardClockSnapshot {
    pub epoch: u64,
    pub logical_tick: u64,
    pub chronos_tick: u64,
    pub phase_index: u8,
    pub theta_rad: f64,
    pub theta_deg: f64,
    pub previous_state: PhaseState,
    pub current_state: PhaseState,
    pub proposed_state: PhaseState,
    pub transition_id: u64,
    pub branch: SignedBranch,
}

impl PhaseGuardClockSnapshot {
    /// Build from a live [`StateClock`] snapshot without mutating the clock.
    ///
    /// `logical_tick` = `kairos_tick` (authoritative event ordering).
    pub fn from_stateclock(
        clock: &StateClock,
        previous_state: PhaseState,
        current_state: PhaseState,
        proposed_state: PhaseState,
        transition_id: u64,
        branch: SignedBranch,
    ) -> Self {
        Self::from_snapshot(
            &clock.snapshot(),
            previous_state,
            current_state,
            proposed_state,
            transition_id,
            branch,
        )
    }

    pub fn from_snapshot(
        snap: &StateClockSnapshot,
        previous_state: PhaseState,
        current_state: PhaseState,
        proposed_state: PhaseState,
        transition_id: u64,
        branch: SignedBranch,
    ) -> Self {
        let logical_tick = snap.kairos_tick;
        let phase_index = phase_index_from_logical_tick(logical_tick);
        Self {
            epoch: snap.epoch,
            logical_tick,
            chronos_tick: snap.chronos_tick,
            phase_index,
            theta_rad: phase_theta_rad(phase_index),
            theta_deg: phase_theta_deg(phase_index),
            previous_state,
            current_state,
            proposed_state,
            transition_id,
            branch,
        }
    }

    pub fn expected_squeeze(&self) -> f64 {
        predict_squeeze_from_logical_tick(self.logical_tick)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseGuardVerdict {
    Pass,
    Warning,
    Fault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseGuardFaultKind {
    None,
    SqueezePredictionMismatch,
    SkippedTick,
    DuplicatedTick,
    ReversedTick,
    StaleTick,
    ReplayEpochTick,
    EpochChanged,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseGuardTraceRow {
    pub epoch: u64,
    pub logical_tick: u64,
    pub phase_index: u8,
    pub theta_deg: f64,
    pub previous_state: PhaseState,
    pub proposed_state: PhaseState,
    pub observed_squeeze: f64,
    pub predicted_squeeze: f64,
    pub difference: f64,
    pub fault: PhaseGuardFaultKind,
    pub verdict: PhaseGuardVerdict,
}

const TRACE_CAPACITY: usize = 64;

/// Read-only PhaseGuard observer attached to the kernel when the feature is on.
#[derive(Debug, Clone)]
pub struct PhaseGuardObserver {
    transition_counter: u64,
    last_epoch: Option<u64>,
    last_logical_tick: Option<u64>,
    seen_pairs: Vec<(u64, u64)>,
    pub trace: Vec<PhaseGuardTraceRow>,
    pub pass_count: u64,
    pub warning_count: u64,
    pub fault_count: u64,
}

impl Default for PhaseGuardObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl PhaseGuardObserver {
    pub fn new() -> Self {
        Self {
            transition_counter: 0,
            last_epoch: None,
            last_logical_tick: None,
            seen_pairs: Vec::new(),
            trace: Vec::new(),
            pass_count: 0,
            warning_count: 0,
            fault_count: 0,
        }
    }

    /// Observe a proposed transition. Never mutates `clock`.
    ///
    /// When `observed_squeeze` is `None`, no physical squeeze sample is available:
    /// tick/epoch anomalies are still evaluated, and the predicted squeeze is
    /// recorded, but squeeze-mismatch is **not** scored (avoids forcing the
    /// cosine law onto transitions that do not carry a squeeze measurement).
    pub fn observe(
        &mut self,
        clock: &StateClock,
        previous_state: PhaseState,
        current_state: PhaseState,
        proposed_state: PhaseState,
        observed_squeeze: Option<f64>,
        branch: SignedBranch,
    ) -> PhaseGuardTraceRow {
        self.observe_snapshot(
            &clock.snapshot(),
            previous_state,
            current_state,
            proposed_state,
            observed_squeeze,
            branch,
        )
    }

    pub fn observe_snapshot(
        &mut self,
        snap: &StateClockSnapshot,
        previous_state: PhaseState,
        current_state: PhaseState,
        proposed_state: PhaseState,
        observed_squeeze: Option<f64>,
        branch: SignedBranch,
    ) -> PhaseGuardTraceRow {
        let id = self.transition_counter;
        self.transition_counter = self.transition_counter.wrapping_add(1);

        let snap = PhaseGuardClockSnapshot::from_snapshot(
            snap,
            previous_state,
            current_state,
            proposed_state,
            id,
            branch,
        );
        let predicted = snap.expected_squeeze();
        let observed = observed_squeeze.unwrap_or(f64::NAN);
        let difference = if observed_squeeze.is_some() {
            observed - predicted
        } else {
            f64::NAN
        };

        let mut fault = PhaseGuardFaultKind::None;
        let mut verdict = PhaseGuardVerdict::Pass;

        // Ordering anomalies vs prior observation (evidence only).
        if let (Some(prev_epoch), Some(prev_tick)) = (self.last_epoch, self.last_logical_tick) {
            if snap.epoch != prev_epoch {
                fault = PhaseGuardFaultKind::EpochChanged;
                verdict = PhaseGuardVerdict::Warning;
            } else if snap.logical_tick < prev_tick {
                let pair = (snap.epoch, snap.logical_tick);
                if self.seen_pairs.iter().any(|p| *p == pair) {
                    fault = PhaseGuardFaultKind::ReplayEpochTick;
                } else {
                    fault = PhaseGuardFaultKind::ReversedTick;
                }
                verdict = PhaseGuardVerdict::Fault;
            } else if snap.logical_tick == prev_tick {
                fault = PhaseGuardFaultKind::DuplicatedTick;
                verdict = PhaseGuardVerdict::Fault;
            } else if snap.logical_tick > prev_tick.saturating_add(1) {
                fault = PhaseGuardFaultKind::SkippedTick;
                verdict = PhaseGuardVerdict::Fault;
            }
        }

        let pair = (snap.epoch, snap.logical_tick);
        if self.seen_pairs.iter().any(|p| *p == pair)
            && fault == PhaseGuardFaultKind::None
            && self.last_logical_tick.is_some()
        {
            fault = PhaseGuardFaultKind::ReplayEpochTick;
            verdict = PhaseGuardVerdict::Fault;
        }

        if let Some(obs_s) = observed_squeeze {
            if fault == PhaseGuardFaultKind::None || fault == PhaseGuardFaultKind::EpochChanged {
                let abs_diff = {
                    let d = obs_s - predicted;
                    if d < 0.0 {
                        -d
                    } else {
                        d
                    }
                };
                if !abs_diff.is_finite() || abs_diff > SQUEEZE_PRED_TOL {
                    fault = PhaseGuardFaultKind::SqueezePredictionMismatch;
                    verdict = PhaseGuardVerdict::Fault;
                }
            }
        }

        if !self.seen_pairs.iter().any(|p| *p == pair) {
            if self.seen_pairs.len() >= TRACE_CAPACITY {
                self.seen_pairs.remove(0);
            }
            self.seen_pairs.push(pair);
        }

        self.last_epoch = Some(snap.epoch);
        self.last_logical_tick = Some(snap.logical_tick);

        match verdict {
            PhaseGuardVerdict::Pass => self.pass_count += 1,
            PhaseGuardVerdict::Warning => self.warning_count += 1,
            PhaseGuardVerdict::Fault => self.fault_count += 1,
        }

        let row = PhaseGuardTraceRow {
            epoch: snap.epoch,
            logical_tick: snap.logical_tick,
            phase_index: snap.phase_index,
            theta_deg: snap.theta_deg,
            previous_state,
            proposed_state,
            observed_squeeze: observed,
            predicted_squeeze: predicted,
            difference,
            fault,
            verdict,
        };
        if self.trace.len() >= TRACE_CAPACITY {
            self.trace.remove(0);
        }
        self.trace.push(row);
        row
    }

    pub fn recent(&self) -> &[PhaseGuardTraceRow] {
        &self.trace
    }

    /// Format a deterministic evidence line for QEMU / UART.
    pub fn format_row(row: &PhaseGuardTraceRow) -> crate::platform::String {
        format!(
            "[PhaseGuard] epoch={} tick={} phase_idx={} theta_deg={:.1} prev={:?} proposed={:?} observed={:.9} predicted={:.9} diff={:.3e} verdict={:?} fault={:?}",
            row.epoch,
            row.logical_tick,
            row.phase_index,
            row.theta_deg,
            row.previous_state,
            row.proposed_state,
            row.observed_squeeze,
            row.predicted_squeeze,
            row.difference,
            row.verdict,
            row.fault
        )
    }
}

/// Prove snapshot construction does not require a second clock and does not
/// need mutable access to StateClock.
pub fn read_logical_tick(clock: &StateClock) -> u64 {
    clock.kairos_tick
}

pub fn read_phase_index(clock: &StateClock) -> u8 {
    phase_index_from_logical_tick(clock.kairos_tick)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stateclock::phase_disk::{canonical_squeeze_coordinate, predict_squeeze};

    #[test]
    fn phaseguard_receives_exact_kairos_tick() {
        let mut clock = StateClock::new();
        clock.record_kairos_event(crate::stateclock::KairosEventKind::Other, "t1");
        clock.record_kairos_event(crate::stateclock::KairosEventKind::Other, "t2");
        assert_eq!(read_logical_tick(&clock), 2);
        let snap = PhaseGuardClockSnapshot::from_stateclock(
            &clock,
            PhaseState::Phi0,
            PhaseState::Phi0,
            PhaseState::Phi1,
            0,
            SignedBranch::Positive,
        );
        assert_eq!(snap.logical_tick, 2);
        assert_eq!(snap.phase_index, 2);
    }

    #[test]
    fn no_separate_phaseguard_clock_field_on_observer() {
        // Observer stores last observed ticks for anomaly detection only —
        // it has no free-running counter used as a time base.
        let obs = PhaseGuardObserver::new();
        assert!(obs.last_logical_tick.is_none());
        assert_eq!(obs.transition_counter, 0); // observation id, not a clock
    }

    #[test]
    fn repeated_reads_do_not_advance_stateclock() {
        let mut clock = StateClock::new();
        clock.record_kairos_event(crate::stateclock::KairosEventKind::Boot, "boot");
        let k0 = clock.kairos_tick;
        let c0 = clock.chronos_tick;
        let e0 = clock.epoch;
        for _ in 0..16 {
            let _ = read_logical_tick(&clock);
            let _ = PhaseGuardClockSnapshot::from_stateclock(
                &clock,
                PhaseState::Phi1,
                PhaseState::Phi1,
                PhaseState::Phi2,
                0,
                SignedBranch::Positive,
            );
        }
        assert_eq!(clock.kairos_tick, k0);
        assert_eq!(clock.chronos_tick, c0);
        assert_eq!(clock.epoch, e0);
    }

    #[test]
    fn tampered_observed_squeeze_does_not_alter_prediction() {
        let clock = StateClock::new();
        let snap = PhaseGuardClockSnapshot::from_stateclock(
            &clock,
            PhaseState::Phi0,
            PhaseState::Phi0,
            PhaseState::Phi1,
            0,
            SignedBranch::Positive,
        );
        let clean = snap.expected_squeeze();
        let mut obs = PhaseGuardObserver::new();
        let row = obs.observe(
            &clock,
            PhaseState::Phi0,
            PhaseState::Phi0,
            PhaseState::Phi1,
            Some(clean + 0.37),
            SignedBranch::Positive,
        );
        assert!((row.predicted_squeeze - clean).abs() < 1e-12);
        assert_eq!(row.fault, PhaseGuardFaultKind::SqueezePredictionMismatch);
    }

    #[test]
    fn skipped_logical_ticks_detected() {
        let mut clock = StateClock::new();
        let mut obs = PhaseGuardObserver::new();
        let s = predict_squeeze_from_logical_tick(0);
        obs.observe(
            &clock,
            PhaseState::Phi0,
            PhaseState::Phi0,
            PhaseState::Phi1,
            Some(s),
            SignedBranch::Positive,
        );
        clock.record_kairos_event(crate::stateclock::KairosEventKind::Other, "a");
        clock.record_kairos_event(crate::stateclock::KairosEventKind::Other, "b");
        // kairos jumped 0→2
        let s2 = predict_squeeze_from_logical_tick(clock.kairos_tick);
        let row = obs.observe(
            &clock,
            PhaseState::Phi1,
            PhaseState::Phi1,
            PhaseState::Phi2,
            Some(s2),
            SignedBranch::Positive,
        );
        assert_eq!(row.fault, PhaseGuardFaultKind::SkippedTick);
    }

    #[test]
    fn duplicated_ticks_detected() {
        let clock = StateClock::new();
        let mut obs = PhaseGuardObserver::new();
        let s = predict_squeeze_from_logical_tick(0);
        obs.observe(
            &clock,
            PhaseState::Phi0,
            PhaseState::Phi0,
            PhaseState::Phi1,
            Some(s),
            SignedBranch::Positive,
        );
        let row = obs.observe(
            &clock,
            PhaseState::Phi1,
            PhaseState::Phi1,
            PhaseState::Phi2,
            Some(s),
            SignedBranch::Positive,
        );
        assert_eq!(row.fault, PhaseGuardFaultKind::DuplicatedTick);
    }

    #[test]
    fn reversed_ticks_detected() {
        let mut clock = StateClock::new();
        let mut obs = PhaseGuardObserver::new();
        clock.record_kairos_event(crate::stateclock::KairosEventKind::Other, "a");
        clock.record_kairos_event(crate::stateclock::KairosEventKind::Other, "b");
        let s2 = predict_squeeze_from_logical_tick(2);
        obs.observe(
            &clock,
            PhaseState::Phi0,
            PhaseState::Phi0,
            PhaseState::Phi1,
            Some(s2),
            SignedBranch::Positive,
        );
        // Fabricate a snapshot-equivalent by manually rolling kairos back is not
        // allowed on live clock; simulate via a second clock copy.
        let mut earlier = StateClock::new();
        earlier.record_kairos_event(crate::stateclock::KairosEventKind::Other, "a");
        let s1 = predict_squeeze_from_logical_tick(1);
        let row = obs.observe(
            &earlier,
            PhaseState::Phi1,
            PhaseState::Phi1,
            PhaseState::Phi2,
            Some(s1),
            SignedBranch::Positive,
        );
        assert_eq!(row.fault, PhaseGuardFaultKind::ReversedTick);
    }

    #[test]
    fn stale_ticks_detected_as_duplicate_when_kairos_frozen() {
        // Stale = logical tick fails to advance across observations.
        let clock = StateClock::new();
        let mut obs = PhaseGuardObserver::new();
        let s = predict_squeeze_from_logical_tick(0);
        obs.observe(
            &clock,
            PhaseState::Phi0,
            PhaseState::Phi0,
            PhaseState::Phi1,
            Some(s),
            SignedBranch::Positive,
        );
        let row = obs.observe(
            &clock,
            PhaseState::Phi1,
            PhaseState::Phi1,
            PhaseState::Phi2,
            Some(s),
            SignedBranch::Positive,
        );
        assert!(
            row.fault == PhaseGuardFaultKind::DuplicatedTick
                || row.fault == PhaseGuardFaultKind::StaleTick
        );
    }

    #[test]
    fn replay_of_earlier_epoch_tick_detected() {
        let mut clock = StateClock::new();
        let mut obs = PhaseGuardObserver::new();
        let s0 = predict_squeeze_from_logical_tick(0);
        obs.observe(
            &clock,
            PhaseState::Phi0,
            PhaseState::Phi0,
            PhaseState::Phi1,
            Some(s0),
            SignedBranch::Positive,
        );
        clock.record_kairos_event(crate::stateclock::KairosEventKind::Other, "a");
        let s1 = predict_squeeze_from_logical_tick(1);
        obs.observe(
            &clock,
            PhaseState::Phi1,
            PhaseState::Phi1,
            PhaseState::Phi2,
            Some(s1),
            SignedBranch::Positive,
        );
        // Replay tick 0 in same epoch via a fresh clock at tick 0.
        let replay = StateClock::new();
        let row = obs.observe(
            &replay,
            PhaseState::Phi2,
            PhaseState::Phi2,
            PhaseState::Phi3,
            Some(s0),
            SignedBranch::Positive,
        );
        assert_eq!(row.fault, PhaseGuardFaultKind::ReplayEpochTick);
    }

    #[test]
    fn wraparound_at_ten_position_boundary() {
        let mut clock = StateClock::new();
        for _ in 0..10 {
            clock.record_kairos_event(crate::stateclock::KairosEventKind::Other, "step");
        }
        assert_eq!(phase_index_from_logical_tick(clock.kairos_tick), 0);
        assert_eq!(clock.kairos_tick, 10);
        let predicted = predict_squeeze_from_logical_tick(10);
        let refp = predict_squeeze(10);
        assert!((predicted - refp).abs() < 1e-12);
        assert!((predicted - canonical_squeeze_coordinate()).abs() < 1e-9); // cos(0)=1
    }

    #[test]
    fn epoch_change_distinguishable_from_wrap() {
        let mut clock = StateClock::new();
        let mut obs = PhaseGuardObserver::new();
        let s0 = predict_squeeze_from_logical_tick(0);
        obs.observe(
            &clock,
            PhaseState::Phi0,
            PhaseState::Phi0,
            PhaseState::Phi1,
            Some(s0),
            SignedBranch::Positive,
        );
        clock.resync("test epoch bump");
        assert_eq!(clock.epoch, 1);
        // After resync, kairos unchanged; observation sees epoch change.
        let row = obs.observe(
            &clock,
            PhaseState::Phi1,
            PhaseState::Phi1,
            PhaseState::Phi2,
            Some(s0),
            SignedBranch::Positive,
        );
        assert_eq!(row.fault, PhaseGuardFaultKind::EpochChanged);
        assert_eq!(row.verdict, PhaseGuardVerdict::Warning);
    }

    #[test]
    fn live_mapping_matches_host_predictor() {
        for n in [0u64, 1, 9, 10, 11, 99, 100] {
            let a = predict_squeeze(n);
            let b = predict_squeeze_from_logical_tick(n);
            assert!((a - b).abs() < 1e-12, "mismatch at n={n}");
        }
    }

    #[test]
    fn valid_monotonic_kairos_trace_zero_false_positives() {
        let mut clock = StateClock::new();
        let mut obs = PhaseGuardObserver::new();
        for i in 0..24u64 {
            let s = predict_squeeze_from_logical_tick(clock.kairos_tick);
            let row = obs.observe(
                &clock,
                PhaseState::Phi1,
                PhaseState::Phi1,
                PhaseState::Phi2,
                Some(s),
                SignedBranch::Positive,
            );
            assert_eq!(row.verdict, PhaseGuardVerdict::Pass, "FP at step {i}");
            clock.record_kairos_event(crate::stateclock::KairosEventKind::Other, "step");
        }
        assert_eq!(obs.fault_count, 0);
    }
}
