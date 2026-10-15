//! `fault` — live phase-state fault-tolerance harness.
//!
//! The previous synthetic path (corrupt a *copy*, fake TMR, string-only
//! recovery) was quarantined 2026-08-11. This is the real replacement: behind
//! the `phaseguard-research` feature (compiled out of the sealed default boot)
//! and an explicit unlock token, `fault inject` corrupts the **live** kernel
//! StateClock, detects it with the syndrome core, restores the live object
//! through `StateClock::restore_from_snapshot`, re-verifies invariants and tick
//! monotonicity, and proves a continuation tick — printing full UART evidence.
//! It reports ISOLATED, never a false RECOVERED, if any step fails.

use crate::platform::String;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

pub fn run(ctx: &mut ShellContext<'_>, args: &[String]) -> Result<(), ShellError> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("help");
    match sub {
        "help" | "status" => help(ctx),
        "inject" | "run" => inject_cmd(ctx, args),
        other => {
            ctx.writeln_line(&format!(
                "fault: unknown subcommand '{other}' — try: fault help"
            ))?;
            Ok(())
        }
    }
}

fn help(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.writeln_line("fault — live StateClock fault detection + executable recovery")?;
    ctx.writeln_line("  usage: fault inject <tick|sector|phase|regress> <unlock-token>")?;
    #[cfg(feature = "phaseguard-research")]
    ctx.writeln_line("  build: phaseguard-research ENABLED (live injection armed)")?;
    #[cfg(not(feature = "phaseguard-research"))]
    ctx.writeln_line(
        "  build: phaseguard-research OFF (injection compiled out; sealed boot unaffected)",
    )?;
    ctx.writeln_line("  recovery restores the LIVE clock, not a copy; isolates if unproven")?;
    Ok(())
}

#[cfg(not(feature = "phaseguard-research"))]
fn inject_cmd(ctx: &mut ShellContext<'_>, _args: &[String]) -> Result<(), ShellError> {
    ctx.writeln_line(
        "fault inject: compiled out — phaseguard-research feature is off; sealed boot is byte-identical.",
    )?;
    Ok(())
}

#[cfg(feature = "phaseguard-research")]
fn inject_cmd(ctx: &mut ShellContext<'_>, args: &[String]) -> Result<(), ShellError> {
    use crate::kernel::fault_recovery::{
        inject_live, recover, render_report, FaultKind, UNLOCK_TOKEN,
    };

    let kind = match args.get(1).and_then(|s| FaultKind::parse(s)) {
        Some(k) => k,
        None => {
            ctx.writeln_line("usage: fault inject <tick|sector|phase|regress> <unlock-token>")?;
            return Ok(());
        }
    };
    let token = args.get(2).map(|s| s.as_str()).unwrap_or("");
    if token != UNLOCK_TOKEN {
        ctx.writeln_line("fault inject: REFUSED — missing/invalid unlock token")?;
        ctx.writeln_line(&format!(
            "  required: fault inject {} {UNLOCK_TOKEN}",
            kind.as_str()
        ))?;
        return Ok(());
    }

    // Capture the verified pre-fault snapshot (the trusted rollback source)
    // BEFORE injecting, so recovery has a proven source.
    let clock = ctx.kernel.get_clock_mut();
    let pre = clock.snapshot();
    inject_live(clock, kind);
    let mut report = recover(clock, &pre);
    report.fault = kind;

    let class = if cfg!(target_arch = "arm") {
        "ON_BOARD_SOFTWARE_TEST"
    } else {
        "HOST_UNIT_TEST"
    };
    let text = render_report(&report, "PZ7020-SL-C(live-clock)", class);
    ctx.writeln(&text)?;
    Ok(())
}
