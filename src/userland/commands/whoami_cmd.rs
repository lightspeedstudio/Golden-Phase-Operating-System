//! whoami — current authority mode (root / anom).

use crate::kernel::anom::{authority_label, visibility_label};
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

pub fn run(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.writeln_line(&format!(
        "authority: {}",
        authority_label(ctx.kernel.authority.authority)
    ))?;
    ctx.writeln_line(&format!(
        "visibility: {}",
        visibility_label(ctx.kernel.authority.visibility)
    ))?;
    if ctx.kernel.authority.is_anom_protected() {
        ctx.writeln_line("anom: active")?;
    } else {
        ctx.writeln_line("anom: standby")?;
    }
    Ok(())
}
