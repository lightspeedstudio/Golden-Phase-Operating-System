//! `sentinel` shell commands. Identity Root/MOD checks live here because
//! local `dispatch` is ungated.

use crate::platform::String;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

pub fn run(ctx: &mut ShellContext<'_>, args: &[String]) -> Result<(), ShellError> {
    #[cfg(feature = "phasesentinel")]
    {
        dispatch(ctx, args)
    }
    #[cfg(not(feature = "phasesentinel"))]
    {
        let _ = args;
        ctx.writeln_line("sentinel unavailable (phasesentinel feature not enabled)")?;
        Ok(())
    }
}

#[cfg(feature = "phasesentinel")]
fn dispatch(ctx: &mut ShellContext<'_>, args: &[String]) -> Result<(), ShellError> {
    use crate::kernel::identity::ModOperation;
    use crate::kernel::sentinel::SentinelError;

    let sub = args.first().map(|s| s.as_str()).unwrap_or("status");
    match sub {
        "status" | "evidence" | "list" => {
            if let Err(e) = require_root_or_mod(ctx) {
                ctx.writeln_line(e.as_str())?;
                return Ok(());
            }
            let header = format!(
                "sentinel observe={} evidence={} drops={} gen={} anchor={}",
                ctx.kernel.sentinel.observe_count(),
                ctx.kernel.sentinel.evidence.len(),
                ctx.kernel.sentinel.evidence.drop_count(),
                ctx.kernel.sentinel.generation,
                if ctx.kernel.sentinel.signers.root_pubkey.is_some() {
                    "pinned"
                } else {
                    "none"
                }
            );
            ctx.writeln_line(&header)?;
            if sub == "evidence" || sub == "list" {
                let lines: crate::platform::Vec<String> = ctx
                    .kernel
                    .sentinel
                    .evidence
                    .records()
                    .iter()
                    .map(|rec| {
                        format!(
                            "  {} pid={} nr={} {}",
                            rec.decision.as_str(),
                            rec.event.pid,
                            rec.event.nr,
                            rec.event.kind.as_u8()
                        )
                    })
                    .collect();
                for line in lines {
                    ctx.writeln_line(&line)?;
                }
            }
        }
        "cert" => {
            let verb = args.get(1).map(|s| s.as_str()).unwrap_or("");
            match verb {
                "pin" => {
                    if let Err(e) = require_ticket(ctx, ModOperation::SentinelCertInstall) {
                        ctx.writeln_line(e.as_str())?;
                        return Ok(());
                    }
                    let Some(hex) = args.get(2) else {
                        ctx.writeln_line("usage: sentinel cert pin <pubkey-hex>")?;
                        return Ok(());
                    };
                    match parse_hex32(hex) {
                        Ok(pk) => {
                            ctx.kernel.sentinel.signers.pin_root(pk);
                            if !ctx.kernel.sentinel.policy.allows_signer(&pk) {
                                ctx.kernel.sentinel.policy.allowed_signers.push(pk);
                            }
                            ctx.writeln_line("sentinel trust anchor pinned")?;
                        }
                        Err(msg) => ctx.writeln_line(msg)?,
                    }
                }
                "revoke" => {
                    if let Err(e) = require_ticket(ctx, ModOperation::SentinelCertRevoke) {
                        ctx.writeln_line(e.as_str())?;
                        return Ok(());
                    }
                    let Some(hex) = args.get(2) else {
                        ctx.writeln_line("usage: sentinel cert revoke <pubkey-hex>")?;
                        return Ok(());
                    };
                    match parse_hex32(hex) {
                        Ok(pk) => match ctx.kernel.sentinel.signers.revoke(&pk) {
                            Ok(()) => ctx.writeln_line("sentinel signer revoked")?,
                            Err(e) => ctx.writeln_line(e.as_str())?,
                        },
                        Err(msg) => ctx.writeln_line(msg)?,
                    }
                }
                _ => ctx.writeln_line("usage: sentinel cert <pin|revoke> <pubkey-hex>")?,
            }
        }
        "quarantine" => {
            if let Err(e) = require_ticket(ctx, ModOperation::SentinelPolicySet) {
                ctx.writeln_line(e.as_str())?;
                return Ok(());
            }
            let Some(pid) = args.get(1).and_then(|s| s.parse().ok()) else {
                ctx.writeln_line("usage: sentinel quarantine <pid>")?;
                return Ok(());
            };
            match ctx.kernel.sentinel_quarantine_task(pid) {
                Ok(()) => ctx.writeln_line("QUARANTINE")?,
                Err(e) => ctx.writeln_line(e.as_str())?,
            }
        }
        "isolate" => {
            if let Err(e) = require_ticket(ctx, ModOperation::SentinelPolicySet) {
                ctx.writeln_line(e.as_str())?;
                return Ok(());
            }
            let Some(pid) = args.get(1).and_then(|s| s.parse().ok()) else {
                ctx.writeln_line("usage: sentinel isolate <pid>")?;
                return Ok(());
            };
            match ctx.kernel.sentinel_isolate_task(pid) {
                Ok(()) => ctx.writeln_line("ISOLATE")?,
                Err(e) => ctx.writeln_line(e.as_str())?,
            }
        }
        "recover" => {
            if let Err(e) = require_ticket(ctx, ModOperation::SentinelRecover) {
                ctx.writeln_line(e.as_str())?;
                return Ok(());
            }
            let Some(pid) = args.get(1).and_then(|s| s.parse().ok()) else {
                ctx.writeln_line("usage: sentinel recover <pid>")?;
                return Ok(());
            };
            let Some(ck) = ctx.kernel.sentinel.latest_checkpoint().cloned() else {
                ctx.writeln_line(SentinelError::NoCheckpoint.as_str())?;
                return Ok(());
            };
            match ctx.kernel.sentinel_recover_task(pid, ck) {
                Ok(()) => ctx.writeln_line("RECOVERED")?,
                Err(e) => ctx.writeln_line(e.as_str())?,
            }
        }
        _ => ctx.writeln_line(
            "usage: sentinel <status|evidence|cert|quarantine|isolate|recover>",
        )?,
    }
    Ok(())
}

#[cfg(feature = "phasesentinel")]
fn require_root_or_mod(ctx: &ShellContext<'_>) -> Result<(), crate::kernel::sentinel::SentinelError> {
    use crate::kernel::identity::PrincipalKind;
    use crate::kernel::sentinel::SentinelError;
    let session = ctx
        .kernel
        .identity
        .sessions()
        .session_for_process(ctx.caller)
        .ok_or(SentinelError::Unauthorized)?;
    let rec = ctx
        .kernel
        .identity
        .principals()
        .get(session.principal_id)
        .map_err(|_| SentinelError::Unauthorized)?;
    if matches!(rec.kind, PrincipalKind::Root | PrincipalKind::Mod) && rec.enabled {
        Ok(())
    } else {
        Err(SentinelError::Unauthorized)
    }
}

#[cfg(feature = "phasesentinel")]
fn require_ticket(
    ctx: &ShellContext<'_>,
    op: crate::kernel::identity::ModOperation,
) -> Result<(), crate::kernel::sentinel::SentinelError> {
    use crate::kernel::sentinel::SentinelError;
    require_root_or_mod(ctx)?;
    let session = ctx
        .kernel
        .identity
        .sessions()
        .session_for_process(ctx.caller)
        .ok_or(SentinelError::Unauthorized)?;
    let now = ctx.kernel.statechain.tick();
    let boot = ctx.kernel.authority.boot_session_root;
    let ok = ctx.kernel.identity.tickets().active().iter().any(|t| {
        t.session_id == session.session_id
            && ctx.kernel.identity.require(t.ticket_id, op, boot, now).is_ok()
    });
    if ok {
        Ok(())
    } else {
        Err(SentinelError::Unauthorized)
    }
}

#[cfg(feature = "phasesentinel")]
fn parse_hex32(s: &str) -> Result<[u8; 32], &'static str> {
    let raw = s.trim();
    if raw.len() != 64 {
        return Err("pubkey hex must be 64 chars");
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&raw[i * 2..i * 2 + 2], 16).map_err(|_| "bad pubkey hex")?;
    }
    Ok(out)
}
