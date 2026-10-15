//! Gate-4 evidence: PhaseGuard driven by live StateClock (hosted).
#![cfg(feature = "phaseguard-research")]

use kairos::core::gphase_core::PhaseState;
use kairos::kernel::Kernel;
use kairos::stateclock::phase_disk::{
    predict_squeeze, predict_squeeze_from_logical_tick, SignedBranch,
};
use kairos::stateclock::phaseguard_live::{
    PhaseGuardFaultKind, PhaseGuardObserver, PhaseGuardVerdict,
};

#[test]
fn kernel_phase_transition_observes_without_advancing_on_read() {
    let mut k = Kernel::new();
    let before_k = k.get_clock().kairos_tick;
    let snap = k.get_clock().snapshot();
    let _ = k.phaseguard.observe_snapshot(
        &snap,
        PhaseState::Phi1,
        PhaseState::Phi1,
        PhaseState::Phi2,
        None,
        SignedBranch::Positive,
    );
    assert_eq!(k.get_clock().kairos_tick, before_k);
    assert_eq!(k.get_clock().epoch, snap.epoch);
}

#[test]
fn lawful_live_tick_samples_have_zero_false_positives() {
    let mut k = Kernel::new();
    for i in 0..32u64 {
        let before = k.get_clock().kairos_tick;
        let predicted = predict_squeeze_from_logical_tick(before.saturating_add(1));
        // Live path: only the Kairos hook observes (no manual double-observe).
        kairos::stateclock::record_kairos_on_kernel(
            &mut k,
            kairos::stateclock::KairosEventKind::Other,
            format!("step {i}"),
        );
        let after = k.get_clock().kairos_tick;
        assert_eq!(after, before + 1);
        assert!((predicted - predict_squeeze(after)).abs() < 1e-12);
        let last = k.phaseguard.recent().last().expect("row");
        assert_eq!(last.logical_tick, after);
        assert_eq!(
            last.verdict,
            PhaseGuardVerdict::Pass,
            "FP at i={i} tick={after}"
        );
    }
    assert_eq!(k.phaseguard.fault_count, 0);
}

#[test]
fn kernel_kairos_hook_zero_false_positives() {
    let mut k = Kernel::new();
    for i in 0..40u64 {
        kairos::stateclock::record_kairos_on_kernel(
            &mut k,
            kairos::stateclock::KairosEventKind::Other,
            format!("hook {i}"),
        );
    }
    assert_eq!(k.phaseguard.fault_count, 0, "FP on valid Kairos stream");
    assert_eq!(k.phaseguard.warning_count, 0);
    assert!(k.phaseguard.pass_count >= 40);
}

#[test]
fn observe_snapshot_overhead_is_small() {
    let mut obs = PhaseGuardObserver::new();
    let mut k = Kernel::new();
    let n = 10_000u64;
    let t0 = std::time::Instant::now();
    for i in 0..n {
        let snap = k.get_clock().snapshot();
        let predicted = predict_squeeze_from_logical_tick(snap.kairos_tick);
        let _ = obs.observe_snapshot(
            &snap,
            PhaseState::Phi0,
            PhaseState::Phi0,
            PhaseState::Phi1,
            Some(predicted),
            SignedBranch::Positive,
        );
        kairos::stateclock::record_kairos_on_kernel(
            &mut k,
            kairos::stateclock::KairosEventKind::Other,
            format!("ov {i}"),
        );
    }
    let elapsed = t0.elapsed();
    let ns_per = elapsed.as_nanos() as f64 / n as f64;
    println!("PhaseGuard observe_snapshot: {n} iters in {elapsed:?} (~{ns_per:.0} ns/obs)");
    // Soft budget: research observer must stay well under 1ms/observation on host.
    assert!(ns_per < 1_000_000.0, "overhead too high: {ns_per} ns/obs");
}

#[test]
fn dump_gate4_trace_evidence() {
    let mut k = Kernel::new();
    let mut lines = Vec::new();
    lines.push("PHASEGUARD Gate-4 QEMU/host evidence trace".to_string());
    lines.push(
        "logical_tick = kairos_tick; phase_index = tick % 10; theta = index * 36 deg".to_string(),
    );
    lines.push("observation = record_kairos_on_kernel hook (authoritative advance)".to_string());
    for i in 0..12u64 {
        kairos::stateclock::record_kairos_on_kernel(
            &mut k,
            kairos::stateclock::KairosEventKind::Other,
            format!("ev {i}"),
        );
        let row = k.phaseguard.recent().last().expect("row");
        lines.push(PhaseGuardObserver::format_row(row));
        assert_eq!(row.verdict, PhaseGuardVerdict::Pass);
        assert_ne!(row.fault, PhaseGuardFaultKind::SqueezePredictionMismatch);
    }
    let path = "docs/research/PHASEGUARD_GATE4_TRACE_EVIDENCE.txt";
    std::fs::write(path, lines.join("\n") + "\n").expect("write evidence");
    assert!(std::path::Path::new(path).exists());
}
