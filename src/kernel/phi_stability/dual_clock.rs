//! PHI-STABILITY dual-clock event tracking.

use crate::kernel::phi_stability::types::{PhiKairosEvent, PhiKairosEventKind};
use crate::platform::Vec;
use crate::stateclock::{record_kairos_on_kernel, KairosEventKind, StateClockSnapshot};

pub struct DualClockRecorder {
    pub events: Vec<PhiKairosEvent>,
    pub calculation_events: u64,
}

impl DualClockRecorder {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            calculation_events: 0,
        }
    }

    pub fn record(
        &mut self,
        kind: PhiKairosEventKind,
        snapshot: &StateClockSnapshot,
        kernel: Option<&mut crate::kernel::Kernel>,
    ) {
        self.events.push(PhiKairosEvent {
            kind,
            chronos_tick: snapshot.chronos_tick,
            kairos_tick: snapshot.kairos_tick,
        });
        match kind {
            PhiKairosEventKind::DoublingStepCompleted
            | PhiKairosEventKind::CalculationCompleted
            | PhiKairosEventKind::ResultVerified
            | PhiKairosEventKind::TrajectorySealed
            | PhiKairosEventKind::MerkleCommitted => {
                self.calculation_events += 1;
            }
            _ => {}
        }
        if let Some(kernel) = kernel {
            record_kairos_on_kernel(kernel, KairosEventKind::Other, "phi-stability");
        }
    }

    pub fn expected_calculation_count(recurrence_steps: u32) -> u64 {
        8 + recurrence_steps as u64
    }

    pub fn verify_policy(&self, recurrence_steps: u32) -> bool {
        let expected = Self::expected_calculation_count(recurrence_steps);
        let actual = self.events.len() as u64;
        actual == expected
    }

    pub fn kairos_sequence(&self) -> Vec<PhiKairosEventKind> {
        self.events.iter().map(|e| e.kind).collect()
    }
}

impl Default for DualClockRecorder {
    fn default() -> Self {
        Self::new()
    }
}

pub fn chronos_elapsed(start: u64, finish: u64) -> u64 {
    finish.saturating_sub(start)
}

pub fn chronos_policy_label(a: u64, b: u64) -> &'static str {
    if a == 0 || b == 0 {
        return "NORMAL";
    }
    let max = a.max(b);
    let min = a.min(b);
    let ratio = max as f64 / min as f64;
    if ratio > 10.0 {
        "OUTLIER"
    } else if ratio > 3.0 {
        "SLOW"
    } else {
        "NORMAL"
    }
}
