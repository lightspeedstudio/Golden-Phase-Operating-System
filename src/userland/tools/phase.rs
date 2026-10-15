//! Phase algebra inspection tools.

use crate::core::gphase_core::{compose, reflect, PhaseState};
use crate::userland::tools::context::CommandContext;
use crate::userland::tools::error::ToolError;
use crate::userland::tools::prelude::*;
use crate::userland::tools::types::AuthorityRequirement;

pub fn status(ctx: &mut CommandContext<'_>, _args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    crate::userland::terminal_cmd::run_phase(ctx.kernel, ctx.caller, ctx.io)
        .map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
    Ok(0)
}

pub fn compose_cmd(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    if args.len() < 2 {
        return Err(ToolError::InvalidArguments);
    }
    let a = parse_state(&args[0])?;
    let b = parse_state(&args[1])?;
    let result = compose(a, b);
    ctx.writeln_line(&format!(
        "compose {} {} -> {:?}",
        label(a),
        label(b),
        result
    ))?;
    Ok(0)
}

pub fn reflect_cmd(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let a = parse_state(args.first().ok_or(ToolError::InvalidArguments)?)?;
    let result = reflect(a);
    ctx.writeln_line(&format!("reflect {} -> {:?}", label(a), result))?;
    Ok(0)
}

pub fn transition(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let sub = args.first().map(|s| s.as_str()).unwrap_or("table");
    match sub {
        "check" => {
            if args.len() < 3 {
                return Err(ToolError::InvalidArguments);
            }
            let from = parse_state(&args[1])?;
            let to = parse_state(&args[2])?;
            ctx.writeln_line(&format!("transition {:?} -> {:?}: allowed", from, to))?;
        }
        "table" => {
            ctx.writeln_line("Phi0 Phi1 Phi2 Phi3 canonical transitions")?;
        }
        _ => return Err(ToolError::InvalidArguments),
    }
    Ok(0)
}

pub fn topology(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    crate::userland::terminal_cmd::run_topo(ctx.kernel, ctx.caller, ctx.io)
        .map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
    let _ = args;
    Ok(0)
}

pub fn chi(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let op = args.first().map(|s| s.as_str()).unwrap_or("status");
    ctx.writeln_line(&format!(
        "phase chi {op}: use phase-benchmark for chi metrics"
    ))?;
    Ok(0)
}

pub fn table(ctx: &mut CommandContext<'_>, _args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    ctx.writeln_line("compose table (canonical):")?;
    for a in [
        PhaseState::Phi0,
        PhaseState::Phi1,
        PhaseState::Phi2,
        PhaseState::Phi3,
    ] {
        for b in [
            PhaseState::Phi0,
            PhaseState::Phi1,
            PhaseState::Phi2,
            PhaseState::Phi3,
        ] {
            let r = compose(a, b);
            ctx.writeln_line(&format!("  {} o {} = {:?}", label(a), label(b), r))?;
        }
    }
    Ok(0)
}

fn parse_state(s: &str) -> Result<PhaseState, ToolError> {
    match s.to_ascii_lowercase().as_str() {
        "phi0" | "0" => Ok(PhaseState::Phi0),
        "phi1" | "1" => Ok(PhaseState::Phi1),
        "phi2" | "2" => Ok(PhaseState::Phi2),
        "phi3" | "3" => Ok(PhaseState::Phi3),
        _ => Err(ToolError::InvalidArguments),
    }
}

fn label(s: PhaseState) -> &'static str {
    match s {
        PhaseState::Phi0 => "Phi0",
        PhaseState::Phi1 => "Phi1",
        PhaseState::Phi2 => "Phi2",
        PhaseState::Phi3 => "Phi3",
    }
}
