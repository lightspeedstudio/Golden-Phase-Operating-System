//! clear — reset the terminal view.

use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

pub fn run(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    ctx.writeln("\x1b[2J\x1b[H")?;
    Ok(())
}
