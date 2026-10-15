//! `passwd` / `passwd --status` — Root credential provisioning and password
//! change (`kernel::identity`, Gate 4).
//!
//! ## Root provisioning gate
//! [`RootCredentialState::Unprovisioned`] already denies every
//! authentication attempt at the state-machine level (Gate 3,
//! `RootCredential::authenticate`). This command is the one real call site
//! that transitions Root out of `Unprovisioned` — it does so via the exact
//! same interactive, no-echo, policy-validated flow as any later password
//! change, so there is no separate "insecure first boot" path.
//!
//! ## Real no-echo secret entry
//! Every password/passphrase this command reads uses
//! [`IoBuffer::read_secret_line`] (via [`syscall::read_secret`]) — real
//! terminal-echo suppression where a real terminal exists to protect (bare-
//! metal UART, or hosted `std` termios/console), and an honest degrade to a
//! plain read otherwise (scripted/non-interactive input, or a hosted target
//! with no wired terminal-control API). See that method's docs.
//!
//! ## Buffer zeroing
//! Every plaintext password `String` this command reads is overwritten via
//! [`credential::zeroize`] as soon as it is no longer needed (before it is
//! dropped) — best-effort, disclosed as such in that function's docs.
//!
//! ## Non-goals (Gate 4)
//! - No recovery flow for `Locked`/`RecoveryRequired` Root credentials —
//!   `passwd` reports the state and refuses, rather than inventing an
//!   unlock procedure the platform spec has not yet specified here.
//!
//! ## Ledger wiring (Gate 5)
//! Every credential mutation this command performs now goes through
//! `Kernel::identity_provision_root_credential` /
//! `identity_change_root_password` / `identity_authenticate_root_credential`
//! instead of touching [`crate::kernel::identity::RootCredential`]
//! directly, so each one is recorded as a real
//! [`crate::kernel::identity::IdentityEventKind`] event in the identity
//! ledger and checkpointed into the real `StatechainLedger` — see
//! `kernel::identity::ledger` module docs.

#[cfg(feature = "identity")]
use crate::kernel::identity::credential::{
    self, create_verifier, is_password_in_history, validate_password, Blocklist,
    DEFAULT_ARGON2_ITERATIONS, DEFAULT_ARGON2_LANES, DEFAULT_ARGON2_MEMORY_KIB,
};
#[cfg(feature = "identity")]
use crate::kernel::identity::RootCredentialState;
#[cfg(feature = "identity")]
use crate::platform::String;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;
#[cfg(feature = "identity")]
use crate::userland::syscall;

/// `passwd --status` — report the Root credential's current state and the
/// active password policy, without reading any input.
pub fn run_status(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(feature = "identity")]
    {
        let state = ctx.kernel.identity.root_credential().state();
        let label = match state {
            RootCredentialState::Unprovisioned => {
                "unprovisioned (no Root password set; run `passwd` to provision)"
            }
            RootCredentialState::Active => "active",
            RootCredentialState::Locked => "locked (too many failed attempts)",
            RootCredentialState::RecoveryRequired => "recovery required",
        };
        ctx.writeln_line(&format!("root credential: {label}"))?;
        let policy = *ctx.kernel.identity.password_policy();
        ctx.writeln_line(&format!(
            "policy: min_length={} max_length={} max_failures={} history_depth={}",
            policy.minimum_length, policy.maximum_length, policy.maximum_failures,
            policy.history_depth
        ))?;
    }
    #[cfg(not(feature = "identity"))]
    {
        ctx.writeln_line("passwd unavailable (identity feature not enabled)")?;
    }
    Ok(())
}

/// `passwd` — interactively provision (from `Unprovisioned`) or change
/// (from `Active`) the Root password.
pub fn run(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(feature = "identity")]
    {
        run_interactive(ctx)?;
    }
    #[cfg(not(feature = "identity"))]
    {
        ctx.writeln_line("passwd unavailable (identity feature not enabled)")?;
    }
    Ok(())
}

#[cfg(feature = "identity")]
fn read_secret(ctx: &mut ShellContext<'_>, prompt: &str) -> Result<String, ShellError> {
    ctx.writeln(prompt)?;
    ctx.ensure_running()?;
    let line = syscall::read_secret(ctx.kernel, ctx.caller, ctx.io).map_err(ShellError::Syscall)?;
    ctx.writeln_line("")?;
    Ok(line)
}

#[cfg(feature = "identity")]
fn zeroize_owned(s: &mut String) {
    // Safety: overwriting every byte with 0x00 keeps the buffer valid UTF-8
    // (NUL is a valid single-byte code point), so the resulting `String`
    // remains sound even though we never read it again before dropping it.
    unsafe {
        credential::zeroize(s.as_bytes_mut());
    }
    s.clear();
}

#[cfg(feature = "identity")]
fn run_interactive(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let state = ctx.kernel.identity.root_credential().state();

    match state {
        RootCredentialState::Locked => {
            ctx.writeln_line(
                "root credential is locked (too many failed attempts); passwd is unavailable \
                 (no recovery flow implemented yet)",
            )?;
            return Ok(());
        }
        RootCredentialState::RecoveryRequired => {
            ctx.writeln_line(
                "root credential requires recovery; passwd is unavailable \
                 (no recovery flow implemented yet)",
            )?;
            return Ok(());
        }
        RootCredentialState::Unprovisioned => {
            ctx.writeln_line("no root password is set yet; provisioning now")?;
        }
        RootCredentialState::Active => {
            let policy = *ctx.kernel.identity.password_policy();
            let mut current = read_secret(ctx, "current root password: ")?;
            let now = ctx.kernel.clock.chronos_tick;
            let result =
                ctx.kernel
                    .identity_authenticate_root_credential(&policy, current.as_bytes(), now);
            zeroize_owned(&mut current);
            match result {
                Ok(true) => {}
                Ok(false) => {
                    ctx.writeln_line("incorrect password")?;
                    return Ok(());
                }
                Err(credential::CredentialError::RateLimited) => {
                    ctx.writeln_line("too many recent failed attempts; try again later")?;
                    return Ok(());
                }
                Err(err) => {
                    ctx.writeln_line(&format!("authentication error: {err:?}"))?;
                    return Ok(());
                }
            }
        }
    }

    let mut new1 = read_secret(ctx, "new root password: ")?;
    let mut new2 = read_secret(ctx, "confirm new root password: ")?;

    if new1 != new2 {
        zeroize_owned(&mut new1);
        zeroize_owned(&mut new2);
        ctx.writeln_line("passwords did not match; root password unchanged")?;
        return Ok(());
    }
    zeroize_owned(&mut new2);

    let policy = *ctx.kernel.identity.password_policy();
    let blocklist = Blocklist::default_blocklist();
    if let Err(err) = validate_password(&policy, new1.as_bytes(), Some(b"root"), &blocklist) {
        zeroize_owned(&mut new1);
        ctx.writeln_line(&format!("password rejected: {err:?}"))?;
        return Ok(());
    }

    if state == RootCredentialState::Active {
        let history = ctx.kernel.identity.root_credential().history();
        if matches!(is_password_in_history(history, new1.as_bytes()), Ok(true)) {
            zeroize_owned(&mut new1);
            ctx.writeln_line("password rejected: matches a recently used root password")?;
            return Ok(());
        }
    }

    let now = ctx.kernel.clock.chronos_tick;
    let verifier = create_verifier(
        new1.as_bytes(),
        DEFAULT_ARGON2_MEMORY_KIB,
        DEFAULT_ARGON2_ITERATIONS,
        DEFAULT_ARGON2_LANES,
        now,
    );
    zeroize_owned(&mut new1);
    let verifier = match verifier {
        Ok(v) => v,
        Err(err) => {
            ctx.writeln_line(&format!("password hashing failed: {err:?}"))?;
            return Ok(());
        }
    };

    let result = if state == RootCredentialState::Unprovisioned {
        ctx.kernel.identity_provision_root_credential(verifier)
    } else {
        ctx.kernel.identity_change_root_password(&policy, verifier)
    };
    match result {
        Ok(()) => ctx.writeln_line("root password set")?,
        Err(err) => ctx.writeln_line(&format!("failed to set root password: {err:?}"))?,
    }
    Ok(())
}
