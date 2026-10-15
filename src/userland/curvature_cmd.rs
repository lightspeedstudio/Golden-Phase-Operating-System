//! `curvature` — causal-curvature watchdog: per-tick reflection-defect rate
//! and rolling z-score (Avenue 8, `docs/EXPLORATION_ROADMAP.md`). See
//! `src/kernel/curvature.rs` for the underlying log and
//! `docs/reports/AVENUE_08_curvature_watchdog.md` for the measured null
//! distributions, threshold justification, and false-positive rate.

use crate::kernel::Kernel;
use crate::platform::String;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

/// Rolling z-score magnitude flagged as an anomaly: two-sided p < 0.001
/// under a normal approximation (z_{0.0005} = 3.29). The Avenue 8 report
/// measures the *actual* false-positive rate this threshold produces on
/// clean workloads, since per-tick defect counts are discrete/bursty and
/// not exactly Gaussian.
pub const Z_ALERT_THRESHOLD: f64 = 3.29;

pub fn render(kernel: &Kernel) -> String {
    let log = &kernel.curvature;
    let (w_mean, w_std) = log.window_stats();
    let latest = log.latest().unwrap_or(0);
    let (l_mean, l_std) = log.lifetime_stats();

    let mut out = String::new();
    out.push_str(&format!("ticks observed:      {}\n", log.ticks));
    out.push_str(&format!("window size:         {}\n", log.window_len()));
    out.push_str(&format!("latest tick defects: {latest}\n"));
    out.push_str(&format!("window mean:         {w_mean:.4}\n"));
    out.push_str(&format!("window std:          {w_std:.4}\n"));
    match log.rolling_z() {
        Some(z) => {
            let alert = z.abs() >= Z_ALERT_THRESHOLD;
            out.push_str(&format!("rolling z-score:     {z:.3}\n"));
            out.push_str(&format!(
                "threshold:           |z| >= {Z_ALERT_THRESHOLD:.2} (p<0.001, two-sided, see report)\n"
            ));
            out.push_str(&format!(
                "status:              {}\n",
                if alert {
                    "ALERT (curvature anomaly)"
                } else {
                    "OK"
                }
            ));
        }
        None => out.push_str("rolling z-score:     n/a (need >=2 ticks)\n"),
    }
    out.push_str(&format!(
        "lifetime mean/std:   {l_mean:.4} / {l_std:.4} (n={})\n",
        log.lifetime_count
    ));
    out
}

pub fn run(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let text = render(ctx.kernel);
    ctx.writeln(&text)?;
    Ok(())
}

/// `curvature inject <reps>` — force a *real* burst of reflection defects
/// (not a fabricated counter bump). It spawns a scratch process and drives
/// it through `reps` legal schedule -> syscall -> kreturn cycles. That
/// transition cycle repeats the phase sequence ..,Phi1,Phi2,Phi3,Phi1,..,
/// which the OS's legal transition graph realizes as the triples
/// (Phi1,Phi2,Phi3) and (Phi3,Phi1,Phi2) every 3 steps — both of which are
/// Reflection-kind (see `src/kernel/curvature.rs` and the Avenue 8 report
/// for the derivation) — so ~2/3 of the transitions in the burst are
/// genuine reflection defects, recorded by the real
/// `Process::check_topology` path exactly as they would be from any other
/// caller. The watchdog is then re-sampled immediately so the spike shows
/// up in this command's own output.
pub fn run_inject(ctx: &mut ShellContext<'_>, reps: u32) -> Result<(), ShellError> {
    let reps = reps.max(1);
    let pid = ctx.kernel.spawn_process("curvature-inject");
    let before = ctx.kernel.scheduler.reflection_count();

    let _ = ctx.kernel.wake(pid);
    for _ in 0..reps {
        let _ = ctx.kernel.schedule(pid);
        let _ = ctx.kernel.syscall(pid);
        let _ = ctx.kernel.kreturn(pid);
    }
    let _ = ctx.kernel.exit(pid);

    let after = ctx.kernel.scheduler.reflection_count();
    let generated = after.saturating_sub(before);
    ctx.kernel.record_curvature_tick();

    ctx.writeln_line(&format!(
        "curvature inject: pid={pid} reps={reps} reflection defects generated={generated}"
    ))?;
    let text = render(ctx.kernel);
    ctx.writeln(&text)?;
    Ok(())
}
