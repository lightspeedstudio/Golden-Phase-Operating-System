//! log — show topology / reflection events.

use crate::core::gphase_core::TopologyKind;
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
    let events = syscall::reflection_log(kernel, caller, io)?;
    let lines: Vec<String> = events
        .iter()
        .map(|e| {
            let kind = match e.kind {
                TopologyKind::Same => "same",
                TopologyKind::Reflection => "reflection",
            };
            format!(
                "pid={} triple=({},{},{}) {kind} L={:?} R={:?}",
                e.pid,
                e.triple.0.as_u8(),
                e.triple.1.as_u8(),
                e.triple.2.as_u8(),
                e.left,
                e.right
            )
        })
        .collect();
    let output = lines.join("\n");
    syscall::ensure_running(kernel, caller)?;
    syscall::write(kernel, caller, io, &output)?;
    Ok(output)
}
