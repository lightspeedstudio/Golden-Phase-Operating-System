//! Process tool group.

use crate::platform::String;
use crate::userland::kill;
use crate::userland::ps;
use crate::userland::run;
use crate::userland::tools::context::CommandContext;
use crate::userland::tools::error::ToolError;
use crate::userland::tools::types::AuthorityRequirement;

pub fn ps(ctx: &mut CommandContext<'_>, _args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    ps::run(ctx.kernel, ctx.caller, ctx.io)
        .map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
    Ok(0)
}

pub fn run_program(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    let name = args.first().ok_or(ToolError::InvalidArguments)?;
    run::run_program(ctx.kernel, ctx.caller, ctx.io, ctx.loader, name, ctx.vms)
        .map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
    ctx.record_statechain("process_started", name.as_bytes());
    Ok(0)
}

pub fn kill(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    let pid: u32 = args
        .first()
        .ok_or(ToolError::InvalidArguments)?
        .parse()
        .map_err(|_| ToolError::InvalidArguments)?;
    kill::run(ctx.kernel, ctx.caller, ctx.io, pid)
        .map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
    ctx.record_statechain("process_terminated", format!("pid={pid}").as_bytes());
    Ok(0)
}

pub fn jobs(ctx: &mut CommandContext<'_>, _args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    ctx.writeln_line("jobs: no job table on this build")?;
    Ok(0)
}

pub fn wait_cmd(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    ctx.writeln_line("wait: no job table on this build")?;
    if let Some(secs) = args.first().and_then(|s| s.parse::<u64>().ok()) {
        let ticks = secs.saturating_mul(10);
        for _ in 0..ticks {
            ctx.kernel.advance_chronos();
            #[cfg(feature = "std")]
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        ctx.writeln_line(&format!("wait: slept {secs}s via sched2/chronos (not a job join)"))?;
        return Ok(0);
    }
    if args.first().map(|s| s.as_str()) == Some("all") {
        ctx.writeln_line("wait all: refused — no job table")?;
        return Ok(1);
    }
    Ok(1)
}

pub fn sleep(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let secs: u64 = args
        .first()
        .ok_or(ToolError::InvalidArguments)?
        .parse()
        .map_err(|_| ToolError::InvalidArguments)?;
    let ticks = secs.saturating_mul(10);
    for _ in 0..ticks {
        ctx.kernel.advance_chronos();
        #[cfg(feature = "std")]
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Ok(0)
}
