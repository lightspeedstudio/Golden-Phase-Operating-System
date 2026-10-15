//! Baremetal PhaseLink over Ethernet commands.
//!
//! Transport is selected per baremetal target: Zynq/ARM drives GEM0
//! (`board::zynq7000::phaselink_eth`, EtherType 0x88B5 over U15 RTL8211F);
//! RISC-V drives VirtIO (`boot::arch::riscv64::phaselink_eth`). Same protocol.

use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

#[cfg(target_arch = "arm")]
use crate::board::zynq7000::phaselink_eth as transport;
#[cfg(target_arch = "riscv64")]
use crate::boot::arch::riscv64::phaselink_eth as transport;

pub fn run_init(ctx: &mut ShellContext<'_>, local: &str, peer: &str) -> Result<(), ShellError> {
    match transport::init_node(&mut ctx.kernel, local, peer) {
        Ok(()) => ctx.writeln_line(&format!("phaselink initialized: {local} -> peer {peer}"))?,
        Err(err) => ctx.writeln_line(err)?,
    }
    Ok(())
}

pub fn run_send(ctx: &mut ShellContext<'_>, peer: &str, text: &str) -> Result<(), ShellError> {
    match transport::send_text(&mut ctx.kernel, peer, text) {
        Ok(()) => ctx.writeln_line(&format!("phaselink sent to {peer}: {text}"))?,
        Err(err) => ctx.writeln_line(err)?,
    }
    Ok(())
}

pub fn run_recv(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    match transport::recv_and_report(&mut ctx.kernel) {
        Ok(report) => ctx.writeln(&report)?,
        Err(err) => ctx.writeln_line(err)?,
    }
    Ok(())
}

pub fn run_replay(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    match transport::replay_last(&mut ctx.kernel) {
        Ok(()) => ctx.writeln_line("phaselink replay sent")?,
        Err(err) => ctx.writeln_line(err)?,
    }
    Ok(())
}

#[cfg(target_arch = "arm")]
pub fn run_bridge(ctx: &mut ShellContext<'_>, text: &str) -> Result<(), ShellError> {
    if text.is_empty() {
        ctx.writeln_line("usage: phaselink bridge <text>")?;
        return Ok(());
    }
    match transport::bridge_exchange(&mut ctx.kernel, text) {
        Ok(report) => ctx.writeln(&report)?,
        Err(err) => ctx.writeln_line(err)?,
    }
    Ok(())
}

pub fn run_status(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.writeln(&ctx.kernel.phaselink.status_lines())?;
    Ok(())
}
