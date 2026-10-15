//! Phase-state benchmark shell commands (hosted).

use crate::kernel::phase_benchmark;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

pub fn run_status(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.writeln(&phase_benchmark::status(ctx.kernel))?;
    Ok(())
}

pub fn run_single(ctx: &mut ShellContext<'_>, n: u64) -> Result<(), ShellError> {
    match phase_benchmark::run_single(ctx.kernel, n) {
        Ok(report) => ctx.writeln(&report)?,
        Err(err) => ctx.writeln_line(&err.reason())?,
    }
    Ok(())
}

pub fn run_compare(ctx: &mut ShellContext<'_>, n: u64) -> Result<(), ShellError> {
    match phase_benchmark::run_compare(ctx.kernel, n) {
        Ok(report) => ctx.writeln(&report)?,
        Err(err) => ctx.writeln_line(&err.reason())?,
    }
    Ok(())
}

pub fn run_sweep(ctx: &mut ShellContext<'_>, start: u64, end: u64) -> Result<(), ShellError> {
    match phase_benchmark::run_sweep(ctx.kernel, start, end) {
        Ok(report) => ctx.writeln(&report)?,
        Err(err) => ctx.writeln_line(&err.reason())?,
    }
    Ok(())
}

pub fn run_repeat(ctx: &mut ShellContext<'_>, n: u64, count: u32) -> Result<(), ShellError> {
    match phase_benchmark::run_repeat(ctx.kernel, n, count) {
        Ok(report) => ctx.writeln(&report)?,
        Err(err) => ctx.writeln_line(&err.reason())?,
    }
    Ok(())
}

pub fn run_scale(ctx: &mut ShellContext<'_>, n: u64, workers: usize) -> Result<(), ShellError> {
    match phase_benchmark::run_scale(ctx.kernel, n, workers) {
        Ok(report) => ctx.writeln(&report)?,
        Err(err) => ctx.writeln_line(&err.reason())?,
    }
    Ok(())
}

pub fn run_dual_clock(ctx: &mut ShellContext<'_>, n: u64) -> Result<(), ShellError> {
    match phase_benchmark::run_dual_clock(ctx.kernel, n) {
        Ok(report) => ctx.writeln(&report)?,
        Err(err) => ctx.writeln_line(&err.reason())?,
    }
    Ok(())
}

pub fn run_network_compare(
    ctx: &mut ShellContext<'_>,
    peer: &str,
    n: u64,
) -> Result<(), ShellError> {
    match phase_benchmark::run_network_compare(ctx.kernel, peer, n) {
        Ok(report) => ctx.writeln(&report)?,
        Err(err) => ctx.writeln_line(&err.reason())?,
    }
    Ok(())
}

pub fn run_report(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    match phase_benchmark::last_report(ctx.kernel) {
        Ok(report) => ctx.writeln(&report)?,
        Err(err) => ctx.writeln_line(&err.reason())?,
    }
    Ok(())
}
