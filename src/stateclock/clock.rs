//! StateClock — Chronos (runtime) and Kairos (event) dual-clock MVP.

use crate::platform::{String, Vec};
use crate::stateclock::event::{ClockEvent, ClockEventKind, KairosEventKind};
use crate::stateclock::geometry::{
    default_delta_phi, update_from_phi, wrap_phi, DEFAULT_TICKS_PER_SECTOR,
};

pub const CLOCK_EVENT_CAPACITY: usize = 64;
pub const CLOCK_STATECHAIN_OBJECT_ID: u32 = 1;

#[derive(Debug, Clone)]
pub struct StateClock {
    pub chronos_tick: u64,
    pub kairos_tick: u64,
    pub proper_tick: u64,
    pub state_tick: u64,
    /// Boot / resync generation. Distinguishes ordinary phase wrap from epoch change.
    pub epoch: u64,
    pub phi: f64,
    pub delta_phi: f64,
    pub sector_id: u8,
    pub epsilon: f64,
    pub x: f64,
    pub dilation_factor: f64,
    state_milli: u64,
    last_sector_id: u8,
    pub events: Vec<ClockEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockError {
    InvalidDilation,
}

impl Default for StateClock {
    fn default() -> Self {
        Self::new()
    }
}

impl StateClock {
    pub fn new() -> Self {
        let delta_phi = default_delta_phi(DEFAULT_TICKS_PER_SECTOR);
        let (sector_id, epsilon, x) = update_from_phi(0.0);
        Self {
            chronos_tick: 0,
            kairos_tick: 0,
            proper_tick: 0,
            state_tick: 0,
            epoch: 0,
            phi: 0.0,
            delta_phi,
            sector_id,
            epsilon,
            x,
            dilation_factor: 1.0,
            state_milli: 0,
            last_sector_id: sector_id,
            events: Vec::new(),
        }
    }

    pub fn skew(&self) -> i64 {
        self.state_tick as i64 - self.proper_tick as i64
    }

    pub fn snapshot(&self) -> StateClockSnapshot {
        StateClockSnapshot {
            chronos_tick: self.chronos_tick,
            kairos_tick: self.kairos_tick,
            proper_tick: self.proper_tick,
            state_tick: self.state_tick,
            epoch: self.epoch,
            phi: self.phi,
            delta_phi: self.delta_phi,
            sector_id: self.sector_id,
            epsilon: self.epsilon,
            x: self.x,
            dilation_factor: self.dilation_factor,
            skew: self.skew(),
        }
    }

    /// Recovery mutation API: roll the live clock's scalar state back to a
    /// previously captured snapshot. This is the real restore used by the
    /// fault-recovery transaction — it writes the live fields, it does not
    /// return a copy. The event log (`events`) is preserved: a recovery is
    /// itself part of history, only the live state rolls back.
    pub fn restore_from_snapshot(&mut self, snap: &StateClockSnapshot) {
        self.chronos_tick = snap.chronos_tick;
        self.kairos_tick = snap.kairos_tick;
        self.proper_tick = snap.proper_tick;
        self.state_tick = snap.state_tick;
        self.epoch = snap.epoch;
        self.phi = snap.phi;
        self.delta_phi = snap.delta_phi;
        self.sector_id = snap.sector_id;
        self.epsilon = snap.epsilon;
        self.x = snap.x;
        self.dilation_factor = snap.dilation_factor;
    }

    pub fn advance_chronos(&mut self) -> Option<ClockEvent> {
        let prev_chronos = self.chronos_tick;
        let prev_kairos = self.kairos_tick;
        let prev_sector = self.last_sector_id;

        self.chronos_tick += 1;
        self.proper_tick += 1;
        self.phi = wrap_phi(self.phi + self.delta_phi);
        self.refresh_geometry();

        self.state_milli += (self.dilation_factor * 1000.0) as u64;
        self.state_tick += self.state_milli / 1000;
        self.state_milli %= 1000;

        let event = self.make_event(
            ClockEventKind::ChronosTick,
            prev_chronos,
            prev_kairos,
            "chronos tick",
        );
        self.push_event(event);

        if self.sector_id != prev_sector {
            self.last_sector_id = self.sector_id;
            let crossing = self.make_event(
                ClockEventKind::SectorCrossing,
                self.chronos_tick,
                self.kairos_tick,
                "sector crossing",
            );
            self.push_event(crossing.clone());
            return Some(crossing);
        }

        self.last_sector_id = self.sector_id;
        None
    }

    pub fn record_kairos_event(
        &mut self,
        kind: KairosEventKind,
        reason: impl Into<String>,
    ) -> ClockEvent {
        let prev_chronos = self.chronos_tick;
        let prev_kairos = self.kairos_tick;
        self.kairos_tick += 1;
        self.refresh_geometry();
        let reason = format!("{}: {}", kind.as_str(), reason.into());
        let event = self.make_event(
            ClockEventKind::KairosEvent,
            prev_chronos,
            prev_kairos,
            &reason,
        );
        self.push_event(event.clone());
        event
    }

    pub fn set_dilation_factor(&mut self, value: f64) -> Result<ClockEvent, ClockError> {
        if !value.is_finite() || value <= 0.0 {
            return Err(ClockError::InvalidDilation);
        }
        let prev_chronos = self.chronos_tick;
        let prev_kairos = self.kairos_tick;
        self.dilation_factor = value;
        let event = self.make_event(
            ClockEventKind::DilationChanged,
            prev_chronos,
            prev_kairos,
            &format!("dilation set to {value:.3}"),
        );
        self.push_event(event.clone());
        Ok(event)
    }

    pub fn resync(&mut self, reason: impl Into<String>) -> ClockEvent {
        let prev_chronos = self.chronos_tick;
        let prev_kairos = self.kairos_tick;
        let old_skew = self.skew();
        self.state_tick = self.proper_tick;
        self.state_milli = 0;
        // Epoch advances on resync so PhaseGuard can distinguish a new generation
        // from ordinary ten-position wrap of logical_tick.
        self.epoch = self.epoch.wrapping_add(1);
        let event = self.make_event(
            ClockEventKind::Resync,
            prev_chronos,
            prev_kairos,
            &format!(
                "{} (old skew {old_skew}, epoch {})",
                reason.into(),
                self.epoch
            ),
        );
        self.push_event(event.clone());
        event
    }

    pub fn recent_events(&self) -> &[ClockEvent] {
        &self.events
    }

    fn refresh_geometry(&mut self) {
        let (sector_id, epsilon, x) = update_from_phi(self.phi);
        self.sector_id = sector_id;
        self.epsilon = epsilon;
        self.x = x;
    }

    fn make_event(
        &self,
        kind: ClockEventKind,
        previous_chronos_tick: u64,
        previous_kairos_tick: u64,
        reason: &str,
    ) -> ClockEvent {
        ClockEvent {
            kind,
            previous_chronos_tick,
            new_chronos_tick: self.chronos_tick,
            previous_kairos_tick,
            new_kairos_tick: self.kairos_tick,
            proper_tick: self.proper_tick,
            state_tick: self.state_tick,
            phi: self.phi,
            sector_id: self.sector_id,
            epsilon: self.epsilon,
            x: self.x,
            dilation_factor: self.dilation_factor,
            skew: self.skew(),
            reason: reason.into(),
        }
    }

    fn push_event(&mut self, event: ClockEvent) {
        if self.events.len() >= CLOCK_EVENT_CAPACITY {
            self.events.remove(0);
        }
        self.events.push(event);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StateClockSnapshot {
    pub chronos_tick: u64,
    pub kairos_tick: u64,
    pub proper_tick: u64,
    pub state_tick: u64,
    pub epoch: u64,
    pub phi: f64,
    pub delta_phi: f64,
    pub sector_id: u8,
    pub epsilon: f64,
    pub x: f64,
    pub dilation_factor: f64,
    pub skew: i64,
}

#[cfg(any(feature = "std", feature = "baremetal"))]
pub fn commit_clock_event(kernel: &mut crate::kernel::Kernel, event: &ClockEvent) {
    use crate::core::gphase_core::PhaseState;
    use crate::kernel::state_proof::ObjectKind;
    use crate::kernel::state_root::SubsystemKind;

    let payload = format!(
        "{:?}|c{}|k{}|p{}|s{}|phi{:.6}|sec{}|x{:.6}|d{:.3}|skew{}",
        event.kind,
        event.new_chronos_tick,
        event.new_kairos_tick,
        event.proper_tick,
        event.state_tick,
        event.phi,
        event.sector_id,
        event.x,
        event.dilation_factor,
        event.skew,
    );
    let _ = kernel.statechain.record_transition(
        ObjectKind::Driver,
        CLOCK_STATECHAIN_OBJECT_ID,
        SubsystemKind::Driver,
        PhaseState::Phi1,
        PhaseState::Phi2,
        &format!("clock:{:?}", event.kind),
        None,
        payload.as_bytes(),
    );
}

#[cfg(feature = "std")]
pub fn record_kairos_on_kernel(
    kernel: &mut crate::kernel::Kernel,
    kind: KairosEventKind,
    reason: impl Into<String>,
) {
    let event = kernel.clock.record_kairos_event(kind, reason);
    phaseguard_observe_after_kairos(kernel);
    commit_clock_event(kernel, &event);
}

#[cfg(not(feature = "std"))]
pub fn record_kairos_on_kernel(
    kernel: &mut crate::kernel::Kernel,
    kind: KairosEventKind,
    reason: impl Into<String>,
) {
    let _ = kernel.clock.record_kairos_event(kind, reason);
    phaseguard_observe_after_kairos(kernel);
}

/// Gate-4: after the authoritative Kairos tick advances, take a read-only
/// snapshot and record PhaseGuard evidence. Never mutates the clock again.
#[inline]
fn phaseguard_observe_after_kairos(_kernel: &mut crate::kernel::Kernel) {
    #[cfg(feature = "phaseguard-research")]
    {
        use crate::core::gphase_core::PhaseState;
        use crate::stateclock::phase_disk::SignedBranch;

        let (prev, curr, prop) = _kernel.phaseguard_pending.take().unwrap_or((
            PhaseState::Phi0,
            PhaseState::Phi0,
            PhaseState::Phi0,
        ));
        let snap = _kernel.clock.snapshot();
        let _ = _kernel.phaseguard.observe_snapshot(
            &snap,
            prev,
            curr,
            prop,
            None, // no physical squeeze sample on ordinary Kairos advances
            SignedBranch::Positive,
        );
    }
}
