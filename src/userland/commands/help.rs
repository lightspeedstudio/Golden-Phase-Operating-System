//! help — list available terminal commands.

use crate::platform::String;
use crate::userland::commands::dispatch::COMMAND_CATALOG;
use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

pub fn run(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let mut out = String::from("Golden Phase Operating System terminal commands:\n");
    for entry in COMMAND_CATALOG {
        out.push_str(&format!("  {:<12} {}\n", entry.name, entry.description));
    }
    ctx.writeln(&out)?;
    Ok(())
}
