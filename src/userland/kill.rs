//! kill — exit a process through the legal kernel path.

use crate::kernel::event_log::ProcessId;
use crate::kernel::syscall::IoBuffer;
use crate::kernel::Kernel;
use crate::userland::syscall;

pub fn run(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    target: ProcessId,
) -> Result<(), syscall::SyscallError> {
    syscall::ensure_running(kernel, caller)?;
    match kernel.kill_process(target) {
        Ok(()) => {
            syscall::ensure_running(kernel, caller)?;
            syscall::write(kernel, caller, io, &format!("killed {target}\n"))?;
            Ok(())
        }
        Err(msg) => {
            kernel
                .trace
                .record(crate::kernel::trace::TraceRecord::Error {
                    message: format!("kill {target}: {msg}"),
                });
            syscall::ensure_running(kernel, caller)?;
            syscall::write(kernel, caller, io, &format!("error: {msg}\n"))?;
            Ok(())
        }
    }
}
