//! PHI-STABILITY shell commands.

use crate::kernel::phi_stability;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

pub fn run_status(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.writeln(&phi_stability::status(ctx.kernel))?;
    Ok(())
}

pub fn run_local(ctx: &mut ShellContext<'_>, n: u64) -> Result<(), ShellError> {
    match phi_stability::run_local(ctx.kernel, n, false) {
        Ok(report) => ctx.writeln(&report)?,
        Err(err) => ctx.writeln_line(&err.reason())?,
    }
    Ok(())
}

pub fn run_dual_clock(ctx: &mut ShellContext<'_>, n: u64) -> Result<(), ShellError> {
    match phi_stability::run_local(ctx.kernel, n, true) {
        Ok(report) => ctx.writeln(&report)?,
        Err(err) => ctx.writeln_line(&err.reason())?,
    }
    Ok(())
}

pub fn run_compare(ctx: &mut ShellContext<'_>, n: u64) -> Result<(), ShellError> {
    match phi_stability::run_compare(ctx.kernel, n) {
        Ok(report) => ctx.writeln(&report)?,
        Err(err) => ctx.writeln_line(&err.reason())?,
    }
    Ok(())
}

pub fn run_network_send(ctx: &mut ShellContext<'_>, peer: &str, n: u64) -> Result<(), ShellError> {
    match phi_stability::network_send(ctx.kernel, peer, n) {
        Ok(report) => ctx.writeln(&report)?,
        Err(err) => ctx.writeln_line(&err.reason())?,
    }
    Ok(())
}

pub fn run_network_receive(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    match phi_stability::network_receive(ctx.kernel) {
        Ok(report) => ctx.writeln(&report)?,
        Err(err) => ctx.writeln_line(&err.reason())?,
    }
    Ok(())
}

pub fn run_fault(ctx: &mut ShellContext<'_>, fault: &str) -> Result<(), ShellError> {
    if fault == "replay" {
        match phi_stability::fault_replay(ctx.kernel) {
            Ok(msg) => ctx.writeln_line(&msg)?,
            Err(err) => ctx.writeln_line(&err.reason())?,
        }
        return Ok(());
    }
    match phi_stability::fault_inject(ctx.kernel, fault) {
        Ok(msg) => ctx.writeln_line(&msg)?,
        Err(err) => ctx.writeln_line(&err.reason())?,
    }
    Ok(())
}
