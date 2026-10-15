//! System inspection tools.

use crate::userland::log;
use crate::userland::mem;
use crate::userland::terminal_cmd;
use crate::userland::tools::context::CommandContext;
use crate::userland::tools::error::ToolError;
use crate::userland::tools::prelude::*;
use crate::userland::tools::types::AuthorityRequirement;

pub fn status(ctx: &mut CommandContext<'_>, _args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let clock = ctx.kernel.get_clock().snapshot();
    ctx.writeln_line("KairOS system status")?;
    ctx.writeln_line(&format!("target: {:?}", ctx.target))?;
    ctx.writeln_line(&format!("authority: {:?}", ctx.kernel.authority.authority))?;
    ctx.writeln_line(&format!(
        "anom: {}",
        if ctx.kernel.authority.is_anom_protected() {
            "protected"
        } else {
            "transparent"
        }
    ))?;
    ctx.writeln_line(&format!("processes: {}", ctx.kernel.process_ids().len()))?;
    ctx.writeln_line(&format!("chronos: {}", clock.chronos_tick))?;
    ctx.writeln_line(&format!("kairos: {}", clock.kairos_tick))?;
    ctx.writeln_line(&format!(
        "math backend: {}",
        ctx.kernel.phase_math.backend_report()
    ))?;
    Ok(0)
}

pub fn mem(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let sub = args.first().map(|s| s.as_str()).unwrap_or("status");
    match sub {
        "status" | "pages" | "verify" => {
            mem::run(ctx.kernel, ctx.caller, ctx.io)
                .map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
        }
        "object" => {
            ctx.writeln_line("mem object: use mem status for page summary")?;
        }
        _ => return Err(ToolError::InvalidArguments),
    }
    Ok(0)
}

pub fn cpu(ctx: &mut CommandContext<'_>, _args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    ctx.writeln_line("cpu status")?;
    #[cfg(all(target_arch = "arm", feature = "zynq-a9"))]
    {
        ctx.writeln_line("architecture: armv7a Cortex-A9")?;
        ctx.writeln_line("board: PZ7020-SL-C")?;
        ctx.writeln_line("cores: 2 (CPU0 shell; CPU1 opt-in)")?;
    }
    #[cfg(not(all(target_arch = "arm", feature = "zynq-a9")))]
    {
        ctx.writeln_line(&format!("architecture: {}", core::env!("CARGO_CFG_TARGET_ARCH")))?;
        ctx.writeln_line("cores: 1")?;
    }
    ctx.writeln_line(&format!(
        "timer ticks: {}",
        ctx.kernel.get_clock().snapshot().chronos_tick
    ))?;
    ctx.writeln_line(&format!(
        "backend: {}",
        ctx.kernel.phase_math.backend_report()
    ))?;
    Ok(0)
}

pub fn uptime(ctx: &mut CommandContext<'_>, _args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let clock = ctx.kernel.get_clock().snapshot();
    ctx.writeln_line(&format!("chronos elapsed: {}", clock.chronos_tick))?;
    ctx.writeln_line(&format!("kairos events: {}", clock.kairos_tick))?;
    ctx.writeln_line(&format!(
        "boot session: {:02x?}",
        &ctx.kernel.authority.boot_session_root[..8]
    ))?;
    Ok(0)
}

pub fn clock(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let sub = args.first().map(|s| s.as_str()).unwrap_or("status");
    match sub {
        "status" | "chronos" | "kairos" | "stateclock" | "atlas" => {
            let mut shell_ctx = crate::userland::context::ShellContext::new(
                ctx.kernel, ctx.caller, ctx.io, ctx.loader, ctx.vms,
            );
            crate::userland::commands::clock_cmd::run(&mut shell_ctx)
                .map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
        }
        _ => return Err(ToolError::InvalidArguments),
    }
    Ok(0)
}

pub fn trace(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let sub = args.first().map(|s| s.as_str()).unwrap_or("show");
    match sub {
        "status" | "show" | "clear" | "filter" => {
            let mut shell_ctx = crate::userland::context::ShellContext::new(
                ctx.kernel, ctx.caller, ctx.io, ctx.loader, ctx.vms,
            );
            crate::userland::commands::trace_cmd::run(&mut shell_ctx)
                .map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
        }
        _ => return Err(ToolError::InvalidArguments),
    }
    Ok(0)
}

pub fn log_cmd(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let sub = args.first().map(|s| s.as_str()).unwrap_or("show");
    match sub {
        "show" | "clear" | "level" => {
            log::run(ctx.kernel, ctx.caller, ctx.io)
                .map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
        }
        _ => return Err(ToolError::InvalidArguments),
    }
    Ok(0)
}

pub fn watchdog(ctx: &mut CommandContext<'_>, _args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    terminal_cmd::run_watchdog(ctx.kernel, ctx.caller, ctx.io)
        .map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
    Ok(0)
}
