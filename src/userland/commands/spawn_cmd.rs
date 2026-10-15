//! spawn — create a named test process.

use crate::userland::context::ShellContext;
use crate::userland::shell::ShellError;

pub fn run(ctx: &mut ShellContext<'_>, name: &str) -> Result<(), ShellError> {
    let pid = ctx.kernel.spawn_process(name);
    ctx.kernel.wake(pid).map_err(|_| {
        ShellError::Syscall(crate::kernel::syscall::SyscallError::Kernel(
            "wake rejected",
        ))
    })?;
    ctx.writeln_line(&format!("spawned {name} pid {pid}"))?;
    Ok(())
}
