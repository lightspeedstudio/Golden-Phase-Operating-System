//! dump — combined debug snapshot.

use crate::platform::String;
use crate::userland::commands::{regs, state_cmd, trace_cmd};
use crate::userland::context::ShellContext;
use crate::userland::ps;
use crate::userland::shell::ShellError;

pub fn run(ctx: &mut ShellContext<'_>) -> Result<(), ShellError> {
    let mut out = String::from("=== dump ===\n");
    out.push_str(&state_cmd::render(ctx.kernel));
    out.push_str("\n--- processes ---\n");
    out.push_str(&ps::render(ctx.kernel)?);
    out.push_str("\n--- registers ---\n");
    if ctx.vms.is_empty() {
        out.push_str("register system not initialized\n");
    } else {
        out.push_str(&regs::render_vms(ctx.vms));
    }
    out.push_str("\n--- trace ---\n");
    out.push_str(&trace_cmd::render(ctx.kernel.get_trace_log()));
    ctx.writeln(&out)?;
    Ok(())
}
