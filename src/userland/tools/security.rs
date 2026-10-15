//! Security tool wrappers.

use crate::userland::tools::context::CommandContext;
use crate::userland::tools::error::ToolError;
use crate::userland::tools::prelude::*;
use crate::userland::tools::types::AuthorityRequirement;

pub fn authority(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let sub = args.first().map(|s| s.as_str()).unwrap_or("status");
    match sub {
        "status" => {
            ctx.writeln_line(&format!(
                "authority mode: {:?}",
                ctx.kernel.authority.authority
            ))?;
            ctx.writeln_line(&format!(
                "visibility: {:?}",
                ctx.kernel.authority.visibility
            ))?;
        }
        "whoami" => {
            ctx.writeln_line(&format!("{:?}", ctx.session_authority()))?;
        }
        "capabilities" => {
            ctx.writeln_line("capabilities: viewer operator root anom-administrator")?;
        }
        _ => return Err(ToolError::InvalidArguments),
    }
    Ok(0)
}

pub fn anom(ctx: &mut CommandContext<'_>, _args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let mut sh = crate::userland::tools::shell::shell_context(ctx);
    crate::userland::commands::anom_cmd::run_status(&mut sh)
        .map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
    Ok(0)
}

pub fn statechain(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let sub = args.first().map(|s| s.as_str()).unwrap_or("status");
    match sub {
        "status" | "verify" | "root" | "show" => {
            crate::userland::terminal_cmd::run_root(ctx.kernel, ctx.caller, ctx.io)
                .map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
        }
        _ => return Err(ToolError::InvalidArguments),
    }
    Ok(0)
}

pub fn audit(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let sub = args.first().map(|s| s.as_str()).unwrap_or("status");
    match sub {
        "status" | "verify" | "show" | "object" => {
            crate::userland::terminal_cmd::run_audit(ctx.kernel, ctx.caller, ctx.io)
                .map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
        }
        _ => return Err(ToolError::InvalidArguments),
    }
    Ok(0)
}

pub fn quarantine(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let sub = args.first().map(|s| s.as_str()).unwrap_or("list");
    match sub {
        "list" | "show" => {
            let mut sh = crate::userland::tools::shell::shell_context(ctx);
            crate::userland::sentinel_cmd::run(
                &mut sh,
                &[String::from("evidence")],
            )
            .map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
            drop(sh);
            ctx.writeln_line("PhaseLink quarantine:")?;
            ctx.writeln_line(&format!(
                "quarantined_frames={}",
                ctx.kernel.phaselink.quarantined_frames
            ))?;
            ctx.writeln_line("ID\tSOURCE\tREASON\tHASH")?;
            let rows: crate::platform::Vec<String> = ctx
                .kernel
                .phaselink
                .quarantine
                .entries
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    format!(
                        "{}\t{:?}\t{}\t{:02x}{:02x}",
                        i,
                        item.source_hint,
                        item.reason.label(),
                        item.calculated_hash[0],
                        item.calculated_hash[1]
                    )
                })
                .collect();
            if rows.is_empty() {
                ctx.writeln_line("(no PhaseLink quarantine entries)")?;
            } else {
                for row in rows {
                    ctx.writeln_line(&row)?;
                }
            }
        }
        "release" | "delete" => {
            ctx.check_authority(AuthorityRequirement::Root)?;
            ctx.writeln_line(
                "quarantine release: no live un-quarantine API on this path (sentinel evidence + PhaseLink log are append-only)",
            )?;
            return Ok(1);
        }
        _ => return Err(ToolError::InvalidArguments),
    }
    Ok(0)
}

pub fn crypto(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    let sub = args.first().map(|s| s.as_str()).unwrap_or("selftest");
    match sub {
        "selftest" | "status" => {
            #[cfg(feature = "baremetal")]
            {
                ctx.writeln_line(&crate::kernel::crypto::selftest())?;
            }
            #[cfg(not(feature = "baremetal"))]
            {
                ctx.writeln_line(
                    "crypto selftest: AES-GCM + ed25519 self-test runs on baremetal builds",
                )?;
            }
        }
        _ => return Err(ToolError::InvalidArguments),
    }
    Ok(0)
}
