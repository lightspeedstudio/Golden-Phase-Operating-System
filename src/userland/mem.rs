//! mem — list page tags.

use crate::core::gphase_core::PhaseState;
use crate::kernel::event_log::ProcessId;
use crate::kernel::syscall::IoBuffer;
use crate::kernel::Kernel;
use crate::platform::{String, Vec};
use crate::userland::syscall;

pub fn run(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    let mut pages: Vec<_> = kernel.tag_report().into_iter().collect();
    pages.sort_by_key(|(id, _)| *id);
    let lines: Vec<String> = pages
        .iter()
        .map(|(page, tag)| format!("{page}\t{}", tag_name(*tag)))
        .collect();
    let output = lines.join("\n");
    syscall::ensure_running(kernel, caller)?;
    syscall::write(kernel, caller, io, &output)?;
    Ok(output)
}

fn tag_name(tag: PhaseState) -> &'static str {
    match tag {
        PhaseState::Phi0 => "free",
        PhaseState::Phi1 => "user",
        PhaseState::Phi2 => "kernel",
        PhaseState::Phi3 => "locked",
    }
}
