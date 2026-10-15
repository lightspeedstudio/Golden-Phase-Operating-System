//! `mod` — structured operator elevation (Gate 9). No `mod shell`.

#[cfg(feature = "identity")]
use crate::kernel::identity::ModOperation;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

pub fn run_list(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(feature = "identity")]
    {
        ctx.writeln_line("mod operations:")?;
        for op in [
            ModOperation::PrincipalCreate,
            ModOperation::PrincipalDisable,
            ModOperation::PrincipalEnable,
            ModOperation::CertificateIssue,
            ModOperation::CertificateRevoke,
            ModOperation::CredentialLock,
            ModOperation::CredentialUnlock,
            ModOperation::FsRemove,
            ModOperation::SessionClose,
        ] {
            ctx.writeln_line(&format!("  {}", op.as_str()))?;
        }
        #[cfg(feature = "phasestack")]
        {
            ctx.writeln_line(&format!("  {}", ModOperation::AppInstall.as_str()))?;
            ctx.writeln_line(&format!(
                "  {}",
                ModOperation::DeveloperCertRevoke.as_str()
            ))?;
        }
        let tickets = ctx.kernel.identity.tickets().active();
        ctx.writeln_line(&format!("active tickets: {}", tickets.len()))?;
        for t in tickets {
            ctx.writeln_line(&format!(
                "  ticket={} op={} session={} expires={}",
                t.ticket_id,
                t.op.as_str(),
                t.session_id,
                t.expires_tick
            ))?;
        }
        ctx.writeln_line(&format!(
            "phaseguard: {}",
            ctx.kernel.identity.phase_guard().as_str()
        ))?;
    }
    #[cfg(not(feature = "identity"))]
    {
        ctx.writeln_line("mod unavailable (identity feature not enabled)")?;
    }
    Ok(())
}

pub fn run(ctx: &mut ShellContext<'_>, verb: &str, lease: u64) -> Result<(), ShellError> {
    #[cfg(feature = "identity")]
    {
        if verb == "shell" {
            ctx.writeln_line("mod shell is not permitted")?;
            return Ok(());
        }
        let Some(op) = ModOperation::parse(verb) else {
            ctx.writeln_line("mod: unknown operation (structured verbs only; see `mod -l`)")?;
            return Ok(());
        };
        let Some(session) = ctx.kernel.identity.sessions().session_for_process(ctx.caller) else {
            ctx.writeln_line("mod: no authenticated session (run `login` first)")?;
            return Ok(());
        };
        match ctx
            .kernel
            .identity_issue_mod_ticket(session.session_id, op, lease)
        {
            Ok(ticket) => {
                if let Err(err) = ctx.kernel.identity_require(ticket.ticket_id, op) {
                    ctx.writeln_line(&format!("mod require failed: {err:?}"))?;
                    return Ok(());
                }
                ctx.writeln_line(&format!(
                    "mod ticket={} op={} expires={}",
                    ticket.ticket_id,
                    op.as_str(),
                    ticket.expires_tick
                ))?;
            }
            Err(err) => ctx.writeln_line(&format!("mod failed: {err:?}"))?,
        }
    }
    #[cfg(not(feature = "identity"))]
    {
        let _ = (verb, lease);
        ctx.writeln_line("mod unavailable (identity feature not enabled)")?;
    }
    Ok(())
}

pub fn run_kill(ctx: &mut ShellContext<'_>, ticket_id: u32) -> Result<(), ShellError> {
    #[cfg(feature = "identity")]
    {
        match ctx.kernel.identity_revoke_ticket(ticket_id) {
            Ok(()) => ctx.writeln_line(&format!("mod ticket {ticket_id} revoked"))?,
            Err(err) => ctx.writeln_line(&format!("mod -k failed: {err:?}"))?,
        }
    }
    #[cfg(not(feature = "identity"))]
    {
        let _ = ticket_id;
        ctx.writeln_line("mod unavailable (identity feature not enabled)")?;
    }
    Ok(())
}
