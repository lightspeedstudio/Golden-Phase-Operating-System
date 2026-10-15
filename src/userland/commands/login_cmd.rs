//! `login` / `logout` — Root password session (Gate 7).

#[cfg(feature = "identity")]
use crate::kernel::identity::credential::zeroize;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;
#[cfg(feature = "identity")]
use crate::userland::syscall;

pub fn run_login(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(feature = "identity")]
    {
        ctx.writeln("root password: ")?;
        ctx.ensure_running()?;
        let mut pw =
            syscall::read_secret(ctx.kernel, ctx.caller, ctx.io).map_err(ShellError::Syscall)?;
        ctx.writeln_line("")?;
        let result = ctx
            .kernel
            .identity_login_root(pw.as_bytes(), ctx.caller);
        unsafe {
            zeroize(pw.as_bytes_mut());
        }
        pw.clear();
        match result {
            Ok(session) => ctx.writeln_line(&format!(
                "login ok session={} principal={}",
                session.session_id, session.principal_id
            ))?,
            Err(err) => ctx.writeln_line(&format!("login failed: {err:?}"))?,
        }
    }
    #[cfg(not(feature = "identity"))]
    {
        ctx.writeln_line("login unavailable (identity feature not enabled)")?;
    }
    Ok(())
}

pub fn run_logout(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    #[cfg(feature = "identity")]
    {
        match ctx.kernel.identity_logout(ctx.caller) {
            Ok(()) => ctx.writeln_line("logout ok")?,
            Err(err) => ctx.writeln_line(&format!("logout failed: {err:?}"))?,
        }
    }
    #[cfg(not(feature = "identity"))]
    {
        ctx.writeln_line("logout unavailable (identity feature not enabled)")?;
    }
    Ok(())
}