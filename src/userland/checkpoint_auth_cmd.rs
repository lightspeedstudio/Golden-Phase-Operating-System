//! `checkpoint-auth` — secret-backed HMAC seal/verify. Not Ed25519, not PSC1.
//!
//! Existing `checkpoint` remains V1/V2 integrity-only. This command never
//! prints secret bytes or `K_checkpoint`. Provision uses silent input only.

use crate::kernel::hash::tagged_hash;
use crate::platform::String;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;
use crate::userland::syscall;
use crate::userland::terminal_fmt::short_hex;

pub fn run_status(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.writeln_line(&ctx.kernel.checkpoint_auth_status_text())?;
    Ok(())
}

pub fn run_hardware(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.writeln_line(&ctx.kernel.checkpoint_auth_hardware_text())?;
    Ok(())
}

pub fn run_provision(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.writeln("checkpoint-auth secret (64 hex chars, no echo): ")?;
    ctx.ensure_running()?;
    let mut line =
        syscall::read_secret(ctx.kernel, ctx.caller, ctx.io).map_err(ShellError::Syscall)?;
    ctx.writeln_line("")?;
    let result = ctx.kernel.checkpoint_auth.provision_from_line(&line);
    zeroize_line(&mut line);
    match result {
        Ok(()) => ctx.writeln_line(
            "checkpoint-auth: provisioned EPHEMERAL_PROVISIONED_SECRET (RAM this boot only)",
        )?,
        Err(err) => {
            ctx.writeln_line(&format!("checkpoint-auth provision failed: {}", err.as_str()))?
        }
    }
    Ok(())
}

pub fn run_seal(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    match ctx.kernel.checkpoint_auth_seal() {
        Ok(record) => {
            let root = tagged_hash("GPOS/checkpoint/export/v1", &[&record]);
            ctx.writeln_line(&format!(
                "checkpoint-auth sealed\n\
                 KEY_BACKEND: {}\n\
                 HARDWARE_BOUND: NO\n\
                 sequence: {}\n\
                 bytes: {}\n\
                 hmac: {}…",
                ctx.kernel.checkpoint_auth.backend().status_label(),
                ctx.kernel
                    .checkpoint_auth
                    .last_record()
                    .and_then(|b| crate::kernel::checkpoint_auth::decode_record(b).ok())
                    .map(|(f, _)| f.sequence)
                    .unwrap_or(0),
                record.len(),
                short_hex(&root)
            ))?;
            Ok(())
        }
        Err(err) => {
            ctx.writeln_line(&format!("checkpoint-auth seal failed: {}", err.as_str()))?;
            Ok(())
        }
    }
}

pub fn run_verify(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    match ctx.kernel.checkpoint_auth_verify_last() {
        Ok(fields) => {
            ctx.writeln_line(&format!(
                "checkpoint-auth verify: OK\n\
                 KEY_BACKEND: {}\n\
                 HARDWARE_BOUND: NO\n\
                 sequence: {}\n\
                 key_epoch: {}",
                ctx.kernel.checkpoint_auth.backend().status_label(),
                fields.sequence,
                fields.key_epoch
            ))?;
            Ok(())
        }
        Err(err) => {
            ctx.writeln_line(&format!(
                "checkpoint-auth verify failed: {}",
                err.as_str()
            ))?;
            Ok(())
        }
    }
}

fn zeroize_line(s: &mut String) {
    unsafe {
        crate::kernel::checkpoint_auth::zeroize(s.as_bytes_mut());
    }
    s.clear();
}
