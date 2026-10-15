//! StateClock — dual-clock subsystem (Chronos + Kairos).

pub mod clock;
pub mod event;
pub mod geometry;
/// PHASEGUARD phase-disk research solver (docket CAREGAGA-GPOS-2026-001).
/// Feature-gated; not part of the default sealed boot.
#[cfg(feature = "phaseguard-research")]
pub mod phase_disk;
/// Gate-4 live observer: stateclock-bound squeeze prediction (read-only).
#[cfg(feature = "phaseguard-research")]
pub mod phaseguard_live;

#[cfg(any(feature = "std", feature = "baremetal"))]
pub use clock::commit_clock_event;
pub use clock::{
    record_kairos_on_kernel, ClockError, StateClock, StateClockSnapshot, CLOCK_STATECHAIN_OBJECT_ID,
};
pub use event::{ClockEvent, ClockEventKind, KairosEventKind};
pub use geometry::{
    default_delta_phi, master_x, sector_base_phi, sector_from_phi, wrap_phi,
    DEFAULT_TICKS_PER_SECTOR, SECTOR_COUNT,
};
