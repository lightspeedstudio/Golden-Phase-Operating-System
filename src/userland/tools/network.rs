//! Network tool wrappers — GEM wire ping, PhaseLink, netsess. No Ok(0) stubs.

use crate::userland::net_cmd;
use crate::userland::netsess_cmd;
use crate::userland::tools::context::CommandContext;
use crate::userland::tools::error::ToolError;
use crate::userland::tools::prelude::*;
use crate::userland::tools::shell;
use crate::userland::tools::types::AuthorityRequirement;

pub fn ping(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    if args.is_empty() {
        return Err(ToolError::InvalidArguments);
    }
    let mut sh = shell::shell_context(ctx);
    match args[0].as_str() {
        "ethernet" | "wire" => {
            net_cmd::run_wire_ping(&mut sh).map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
        }
        "phase" => {
            #[cfg(all(feature = "baremetal", not(feature = "std"), feature = "bm-net"))]
            {
                crate::userland::phaselink_baremetal_cmd::run_status(&mut sh)
                    .map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
            }
            #[cfg(not(all(feature = "baremetal", not(feature = "std"), feature = "bm-net")))]
            {
                sh.writeln_line("ping phase: requires bm-net PhaseLink over GEM0 0x88B5")?;
            }
        }
        "ipv4" => {
            ctx.writeln_line(
                "ping ipv4: no ICMP echo on this image — use `net ip <a.b.c.d>` or `ai ask <a.b.c.d> <port> <prompt>` for TCP",
            )?;
            return Ok(1);
        }
        _ => return Err(ToolError::InvalidArguments),
    }
    Ok(0)
}

pub fn connect(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    if args.len() < 2 {
        return Err(ToolError::InvalidArguments);
    }
    let mut sh = shell::shell_context(ctx);
    #[cfg(all(feature = "baremetal", not(feature = "std"), feature = "bm-net"))]
    {
        crate::userland::phaselink_baremetal_cmd::run_init(&mut sh, &args[0], &args[1])
            .map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
        return Ok(0);
    }
    #[cfg(not(all(feature = "baremetal", not(feature = "std"), feature = "bm-net")))]
    {
        sh.writeln_line(&format!(
            "connect {} {}: requires bm-net PhaseLink (`phaselink init <local> <peer>`)",
            args[0], args[1]
        ))?;
        Ok(1)
    }
}

pub fn listen(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    ctx.writeln_line(&format!(
        "listen {}: PhaseLink has no bind socket — use `phaselink recv` / `net rx-test`",
        args.first().unwrap_or(&"?".into())
    ))?;
    Ok(1)
}

pub fn send(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Operator)?;
    if args.len() < 3 {
        return Err(ToolError::InvalidArguments);
    }
    if args[2].len() > 256 {
        return Err(ToolError::InputTooLarge);
    }
    let text = args[2..].join(" ");
    #[cfg(all(feature = "baremetal", not(feature = "std"), feature = "bm-net"))]
    {
        let mut sh = shell::shell_context(ctx);
        crate::userland::phaselink_baremetal_cmd::run_send(&mut sh, &args[1], &text)
            .map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
        drop(sh);
        ctx.record_statechain("network_send", text.as_bytes());
        return Ok(0);
    }
    #[cfg(not(all(feature = "baremetal", not(feature = "std"), feature = "bm-net")))]
    {
        let _ = text;
        let mut sh = shell::shell_context(ctx);
        sh.writeln_line("send: requires bm-net (`phaselink send <peer> <text>` or `net tx-test`)")?;
        Ok(1)
    }
}

pub fn receive(ctx: &mut CommandContext<'_>, _args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    #[cfg(all(feature = "baremetal", not(feature = "std"), feature = "bm-net"))]
    {
        let mut sh = shell::shell_context(ctx);
        crate::userland::phaselink_baremetal_cmd::run_recv(&mut sh)
            .map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
        drop(sh);
        let netsess = netsess_cmd::render(ctx.kernel);
        ctx.writeln(&netsess)?;
        ctx.writeln("\n")?;
        return Ok(0);
    }
    #[cfg(not(all(feature = "baremetal", not(feature = "std"), feature = "bm-net")))]
    {
        let netsess = netsess_cmd::render(ctx.kernel);
        ctx.writeln(&netsess)?;
        ctx.writeln("\n")?;
        ctx.writeln_line("receive: PhaseLink recv requires bm-net")?;
        Ok(1)
    }
}

pub fn ssh(ctx: &mut CommandContext<'_>, args: &[String]) -> Result<i32, ToolError> {
    ctx.check_authority(AuthorityRequirement::Viewer)?;
    #[cfg(feature = "ssh")]
    {
        use crate::userland::ssh_cmd;
        let mut shell_ctx = crate::userland::tools::shell::shell_context(ctx);
        let sub = args.first().map(|s| s.as_str()).unwrap_or("status");
        match sub {
            "status" => ssh_cmd::run_status(&mut shell_ctx)
                .map_err(|e| ToolError::InternalError(format!("{e:?}")))?,
            "start" => ssh_cmd::run_start(&mut shell_ctx)
                .map_err(|e| ToolError::InternalError(format!("{e:?}")))?,
            "stop" => ssh_cmd::run_stop(&mut shell_ctx)
                .map_err(|e| ToolError::InternalError(format!("{e:?}")))?,
            "sessions" => ssh_cmd::run_sessions(&mut shell_ctx)
                .map_err(|e| ToolError::InternalError(format!("{e:?}")))?,
            _ => return Err(ToolError::InvalidArguments),
        }
        return Ok(0);
    }
    #[cfg(all(not(feature = "ssh"), feature = "ssh-bm"))]
    {
        let sub = args.first().map(|s| s.as_str()).unwrap_or("status");
        match sub {
            "status" | "stop" | "start" => {
                ctx.writeln_line("ssh-bm: parser owns start/status/stop (sunset over GEM0)")?;
            }
            _ => return Err(ToolError::InvalidArguments),
        }
        Ok(0)
    }
    #[cfg(all(not(feature = "ssh"), not(feature = "ssh-bm")))]
    {
        ctx.writeln_line("ssh: requires ssh (hosted russh) or ssh-bm (PZ7020 sunset)")?;
        Ok(1)
    }
}
