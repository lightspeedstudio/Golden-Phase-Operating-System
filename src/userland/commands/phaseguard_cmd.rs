//! `phaseguard` — dump Gate-4 read-only observer trace (feature `phaseguard-research`).

use crate::core::gphase_core::PhaseState;
use crate::platform::Vec;
use crate::stateclock::phase_disk::{
    phase_index_from_logical_tick, phase_theta_deg, predict_squeeze_from_logical_tick,
};
use crate::stateclock::phaseguard_live::PhaseGuardObserver;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

pub fn run_status(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let clock = ctx.kernel.get_clock();
    let tick = clock.kairos_tick;
    let epoch = clock.epoch;
    let chronos = clock.chronos_tick;
    let idx = phase_index_from_logical_tick(tick);
    let predicted = predict_squeeze_from_logical_tick(tick);
    let pass = ctx.kernel.phaseguard.pass_count;
    let warn = ctx.kernel.phaseguard.warning_count;
    let fault = ctx.kernel.phaseguard.fault_count;
    let len = ctx.kernel.phaseguard.recent().len();
    ctx.writeln(&format!(
        "PhaseGuard status (read-only)\n\
         epoch={} kairos_tick={} chronos_tick={} phase_idx={} predicted_squeeze={:.9}\n\
         pass={} warn={} fault={} trace_len={}\n\
         note: observes every Kairos advance; squeeze sample optional via `phaseguard sample`",
        epoch, tick, chronos, idx, predicted, pass, warn, fault, len
    ))?;
    Ok(())
}

pub fn run_trace(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let mut lines: Vec<_> = Vec::new();
    for row in ctx.kernel.phaseguard.recent() {
        lines.push(PhaseGuardObserver::format_row(row));
    }
    if lines.is_empty() {
        ctx.writeln("PhaseGuard trace empty")?;
        return Ok(());
    }
    for line in lines {
        ctx.writeln(&line)?;
    }
    Ok(())
}

/// Lawful demo preview: print observed=predicted from the **live** kairos tick
/// without mutating the observer (avoids duplicate-tick false positives before
/// the command's own Kairos record runs).
pub fn run_sample_lawful(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let snap = ctx.kernel.get_clock().snapshot();
    let tick = snap.kairos_tick;
    let idx = phase_index_from_logical_tick(tick);
    let predicted = predict_squeeze_from_logical_tick(tick);
    let line = format!(
        "[PhaseGuard] epoch={} tick={} phase_idx={} theta_deg={:.1} prev={:?} proposed={:?} observed={:.9} predicted={:.9} diff={:.3e} verdict=Pass fault=None (preview)",
        snap.epoch,
        tick,
        idx,
        phase_theta_deg(idx),
        PhaseState::Phi1,
        PhaseState::Phi2,
        predicted,
        predicted,
        0.0f64,
    );
    ctx.writeln(&line)?;
    Ok(())
}
