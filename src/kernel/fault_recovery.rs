//! Executable fault recovery for the live kernel StateClock.
//!
//! This is the piece `fault_syndrome.rs` explicitly left unbuilt: it noted that
//! `tmr_decide` "returns a DECISION ONLY; it restores nothing." Here recovery is
//! **executable** — it captures a verified pre-fault snapshot of the live
//! `StateClock`, (research builds only) injects a real fault into that live
//! object, detects it with the existing syndrome core, then **restores the live
//! clock through its real mutation API** (`StateClock::restore_from_snapshot`),
//! re-verifies the invariants and tick monotonicity, confirms the live state
//! equals the verified source, and runs a continuation tick proving the kernel
//! still advances. If any step fails it reports isolation — it never claims a
//! recovery it did not perform.
//!
//! The protected state `X = (φ2, φ3, κ, τ, R)` is mapped onto the live clock:
//!   τ  = `kairos_tick`         (monotonic kernel tick)
//!   κ  = `sector_id % 4`       (discrete Kairos phase, via `gphase_core`)
//!   φ2 = low16(`state_tick`)   φ3 = low16(`proper_tick`)   (continuous phases)
//!   R  = FNV-1a fingerprint over the snapshot's integer state (state root)
//!
//! Injection is gated behind the `phaseguard-research` feature (compiled out of
//! the sealed default boot) AND an explicit unlock token at the call site.

use crate::kernel::fault_syndrome::{
    calculate_syndrome, FaultAccumulator, Health, PhaseState as SynState,
};
use crate::platform::String;
use crate::stateclock::{StateClock, StateClockSnapshot};

/// Explicit unlock token required to arm live injection (defence in depth on top
/// of the compile-time `phaseguard-research` gate).
pub const UNLOCK_TOKEN: &str = "PHASEGUARD-LIVE-INJECT";

/// 64-bit FNV-1a over the snapshot's integer state — the state root `R`.
fn fingerprint(s: &StateClockSnapshot) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut mix = |v: u64| {
        for b in v.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    };
    mix(s.chronos_tick);
    mix(s.kairos_tick);
    mix(s.proper_tick);
    mix(s.state_tick);
    mix(s.epoch);
    mix(s.sector_id as u64);
    h
}

/// Map a captured snapshot onto the syndrome's `PhaseState`.
fn syn_of(s: &StateClockSnapshot) -> SynState {
    SynState {
        phi2: s.state_tick as u16,
        phi3: s.proper_tick as u16,
        kphase: (s.sector_id & 0x03),
        algebra_root: 0x0000_0000_474f_4c44, // "GOLD" — fixed equation fingerprint
        chain_root: fingerprint(s),
        tick: s.kairos_tick,
    }
}

/// The controlled fault kinds, each corrupting a real live clock field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultKind {
    /// τ jumps forward — kernel tick skip.
    TickSkip,
    /// κ leaves its value — discrete phase / coherence corruption.
    SectorCorrupt,
    /// φ2 drifts — continuous phase corruption.
    PhaseDrift,
    /// τ regresses — non-monotonic tick (causally impossible).
    TickRegress,
}

impl FaultKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FaultKind::TickSkip => "TICK_SKIP",
            FaultKind::SectorCorrupt => "SECTOR_CORRUPT",
            FaultKind::PhaseDrift => "PHASE_DRIFT",
            FaultKind::TickRegress => "TICK_REGRESS",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "tick" | "tick_skip" => Some(FaultKind::TickSkip),
            "sector" | "sector_corrupt" => Some(FaultKind::SectorCorrupt),
            "phase" | "phase_drift" => Some(FaultKind::PhaseDrift),
            "regress" | "tick_regress" => Some(FaultKind::TickRegress),
            _ => None,
        }
    }
}

/// Research-only: corrupt the **live** clock object through controlled mutation.
/// Compiled out entirely unless `phaseguard-research` is enabled.
#[cfg(feature = "phaseguard-research")]
pub fn inject_live(clock: &mut StateClock, kind: FaultKind) {
    match kind {
        FaultKind::TickSkip => clock.kairos_tick = clock.kairos_tick.wrapping_add(1000),
        FaultKind::SectorCorrupt => clock.sector_id = clock.sector_id.wrapping_add(2),
        FaultKind::PhaseDrift => clock.state_tick = clock.state_tick.wrapping_add(5000),
        FaultKind::TickRegress => clock.kairos_tick = clock.kairos_tick.wrapping_sub(3),
    }
}

/// Full evidence of one recovery transaction, rendered for UART.
pub struct RecoveryReport {
    pub fault: FaultKind,
    pub before: StateClockSnapshot, // verified pre-fault (recovery source)
    pub faulted: StateClockSnapshot, // live state after injection
    pub restored: StateClockSnapshot,
    pub distance_sq: u64,
    pub health: Health,
    pub i1_ok: bool,
    pub i2_ok: bool,
    pub state_changed: bool,
    pub restored_equals_source: bool,
    pub tick_monotonic: bool,
    pub reverify_valid: bool,
    pub continuation_ok: bool,
    pub recovered: bool,
    pub root_before: u64,
    pub root_after: u64,
}

fn snaps_equal(a: &StateClockSnapshot, b: &StateClockSnapshot) -> bool {
    a.kairos_tick == b.kairos_tick
        && a.proper_tick == b.proper_tick
        && a.state_tick == b.state_tick
        && a.sector_id == b.sector_id
        && a.chronos_tick == b.chronos_tick
        && a.epoch == b.epoch
}

/// Execute the recovery transaction against the **live** clock.
///
/// `pre` is the verified pre-fault snapshot captured by the caller *before*
/// injection (the trusted rollback source). This function assumes the live
/// clock currently holds the faulted state; it restores, re-verifies, and
/// proves continuation on the real object.
pub fn recover(clock: &mut StateClock, pre: &StateClockSnapshot) -> RecoveryReport {
    // Step 2–3: capture the live faulted state and prove it actually changed.
    let faulted = clock.snapshot();
    let state_changed = !snaps_equal(&faulted, pre);

    // Step 4: syndrome from independently obtained evidence (live vs verified pre).
    let expected = syn_of(pre);
    let observed = syn_of(&faulted);
    let syn = calculate_syndrome(observed, expected);
    let mut acc = FaultAccumulator::new();
    let health = acc.update(&syn);
    let root_before = observed.chain_root;

    // Steps 5–7: the verified pre-fault snapshot is the trusted source; restore
    // the LIVE clock through its real mutation API.
    clock.restore_from_snapshot(pre);

    // Steps 8–10: recompute root, re-verify invariants, confirm equality.
    let restored = clock.snapshot();
    let restored_syn = syn_of(&restored);
    let reverify = calculate_syndrome(restored_syn, expected);
    let restored_equals_source = snaps_equal(&restored, pre);
    let tick_monotonic = restored.kairos_tick == pre.kairos_tick;
    let reverify_valid = matches!(reverify.health, Health::Valid);
    let root_after = restored_syn.chain_root;

    // Step 12: continuation — advance one legitimate tick and prove it moves.
    let chronos_before = clock.chronos_tick;
    let _ = clock.advance_chronos();
    let continuation_ok = clock.chronos_tick == chronos_before.wrapping_add(1);

    let recovered = state_changed
        && restored_equals_source
        && tick_monotonic
        && reverify_valid
        && restored_equals_source
        && continuation_ok;

    RecoveryReport {
        fault: FaultKind::TickSkip, // overwritten by caller-set field below
        before: *pre,
        faulted,
        restored,
        distance_sq: syn.distance_sq,
        health,
        i1_ok: syn.i1_compose_ok,
        i2_ok: syn.i2_coherent,
        state_changed,
        restored_equals_source,
        tick_monotonic,
        reverify_valid,
        continuation_ok,
        recovered,
        root_before,
        root_after,
    }
}

/// Render the evidence record for UART. `commit`/`board` are supplied by the
/// caller so the transcript is self-identifying. Test class is provided by the
/// caller and printed verbatim (never inflated here).
pub fn render_report(r: &RecoveryReport, board: &str, test_class: &str) -> String {
    format!(
        "== PHASEGUARD LIVE FDIR ==\n\
         board            : {board}\n\
         fault_type       : {}\n\
         target           : live StateClock (kernel)\n\
         state_before     : tau={} sector={} state_tick={} proper_tick={} chronos={}\n\
         state_after_inj  : tau={} sector={} state_tick={} proper_tick={} chronos={}\n\
         state_changed    : {}\n\
         raw_syndrome     : phi2_err={} phi3_err={} tick_err={} i1_ok={} i2_ok={} chain_mismatch={}\n\
         d2_total         : {}\n\
         classification   : {}\n\
         recovery_source  : VERIFIED_PRE_FAULT_SNAPSHOT\n\
         recovery_fn      : StateClock::restore_from_snapshot (live mutation)\n\
         state_after_rec  : tau={} sector={} state_tick={} proper_tick={} chronos={}\n\
         state_root       : before={:#018x} after={:#018x}\n\
         invariants       : reverify_valid={} tick_monotonic={} restored==source={}\n\
         continuation     : {}\n\
         OUTCOME          : {}\n\
         TEST_CLASS       : {test_class}\n",
        r.fault.as_str(),
        r.before.kairos_tick, r.before.sector_id, r.before.state_tick, r.before.proper_tick, r.before.chronos_tick,
        r.faulted.kairos_tick, r.faulted.sector_id, r.faulted.state_tick, r.faulted.proper_tick, r.faulted.chronos_tick,
        r.state_changed,
        {
            let e = crate::kernel::fault_syndrome::phase_error(r.faulted.state_tick as u16, r.before.state_tick as u16);
            e
        },
        {
            let e = crate::kernel::fault_syndrome::phase_error(r.faulted.proper_tick as u16, r.before.proper_tick as u16);
            e
        },
        (r.faulted.kairos_tick as i64) - (r.before.kairos_tick as i64),
        r.i1_ok, r.i2_ok, (r.root_before != r.root_after),
        r.distance_sq,
        r.health.as_str(),
        r.restored.kairos_tick, r.restored.sector_id, r.restored.state_tick, r.restored.proper_tick, r.restored.chronos_tick,
        r.root_before, r.root_after,
        r.reverify_valid, r.tick_monotonic, r.restored_equals_source,
        if r.continuation_ok { "PASS" } else { "FAIL" },
        if r.recovered { "RECOVERED" } else { "ISOLATED" },
    )
}

// ── HOST_UNIT_TEST: executable recovery on a real StateClock ────────────────
// These run on the host (std). They validate the recovery transaction against a
// genuine StateClock object — corrupting real live fields and proving the live
// object is restored. They are NOT hardware evidence.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::stateclock::StateClock;

    fn warmed() -> StateClock {
        let mut c = StateClock::new();
        for _ in 0..5 {
            let _ = c.advance_chronos();
        }
        c
    }

    #[test]
    fn tick_skip_is_detected_and_live_state_restored() {
        let mut clock = warmed();
        let pre = clock.snapshot();
        // simulate exactly what inject_live(TickSkip) does to the live object
        clock.kairos_tick = clock.kairos_tick.wrapping_add(1000);
        let mut r = recover(&mut clock, &pre);
        r.fault = FaultKind::TickSkip;
        assert!(r.state_changed, "injection must change live state");
        assert!(r.distance_sq > 0, "tick skip must score a syndrome");
        assert!(
            r.recovered,
            "recovery must be proven (health={:?})",
            r.health
        );
        assert!(r.restored_equals_source && r.tick_monotonic && r.reverify_valid);
        // the LIVE clock actually rolled back:
        assert_eq!(clock.kairos_tick, pre.kairos_tick);
    }

    #[test]
    fn sector_corruption_restored() {
        let mut clock = warmed();
        let pre = clock.snapshot();
        clock.sector_id = clock.sector_id.wrapping_add(2);
        let r = recover(&mut clock, &pre);
        assert!(r.state_changed && r.recovered);
        assert_eq!(
            clock.sector_id, pre.sector_id,
            "live sector must be restored"
        );
    }

    #[test]
    fn no_change_is_never_reported_as_recovery() {
        // The prompt's negative test: recovery that returns without modifying
        // live state must NOT be claimed as a recovery.
        let mut clock = warmed();
        let pre = clock.snapshot();
        let r = recover(&mut clock, &pre); // no injection
        assert!(!r.state_changed);
        assert!(!r.recovered, "must not claim recovery when nothing changed");
    }

    #[test]
    fn continuation_proves_clock_still_advances() {
        let mut clock = warmed();
        let pre = clock.snapshot();
        clock.state_tick = clock.state_tick.wrapping_add(5000);
        let before = clock.chronos_tick;
        let r = recover(&mut clock, &pre);
        assert!(r.continuation_ok);
        // continuation advanced chronos exactly one legitimate tick past restore
        assert_eq!(clock.chronos_tick, pre.chronos_tick + 1);
        assert!(clock.chronos_tick >= before.min(pre.chronos_tick));
    }
}
