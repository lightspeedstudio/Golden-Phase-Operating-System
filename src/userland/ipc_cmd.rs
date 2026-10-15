//! IPC and message-queue shell commands via syscalls (`feature = "ipc"`).
//! Mirrors `fs_cmd`'s `open`/`close` pattern: minting/resolving/destroying
//! goes through `captable`, so this module never touches `kernel.ipc`
//! directly (see `kernel::syscall`'s `IPC_*` handlers).

use crate::kernel::event_log::ProcessId;
use crate::kernel::syscall::{
    IoBuffer, SyscallArgs, SyscallResult, IPC_CREATE_QUEUE, IPC_DESTROY_QUEUE, IPC_RECV, IPC_SEND,
};
use crate::kernel::Kernel;
use crate::platform::{String, ToString};
use crate::userland::syscall::{self, SyscallError};

/// `RIGHT_READ | RIGHT_WRITE | RIGHT_DELETE | RIGHT_GRANT` from
/// `kernel::captable`, inlined by value (matching `fs_cmd::rights_from_mode`'s
/// own precedent) so this file compiles without requiring the optional
/// `captable` feature at this call site.
const OWNER_RIGHTS: u32 = 0b1111;

/// `IPC_CREATE_QUEUE`: mint a new bounded queue of `capacity` slots and an
/// owning capability (read+write+delete+grant) for `caller` on it. Returns
/// `SyscallResult::Handle(slot, generation)` when the `ipc` feature is
/// enabled, or plain `Ok` (no queue created) when it is disabled.
pub fn create(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    capacity: usize,
) -> Result<SyscallResult, SyscallError> {
    syscall::ensure_running(kernel, caller)?;
    let args = SyscallArgs {
        ipc_capacity: capacity,
        rights: OWNER_RIGHTS,
        ..Default::default()
    };
    syscall::dispatch(kernel, caller, io, IPC_CREATE_QUEUE, &args)
}

/// `IPC_SEND`: authorize `RIGHT_WRITE` on `(slot, generation)`, then enqueue
/// `text`'s bytes.
pub fn send(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    slot: u32,
    generation: u32,
    text: &str,
) -> Result<(), SyscallError> {
    syscall::ensure_running(kernel, caller)?;
    let args = SyscallArgs {
        cap_slot: slot,
        cap_generation: generation,
        text: text.to_string(),
        ..Default::default()
    };
    match syscall::dispatch(kernel, caller, io, IPC_SEND, &args)? {
        SyscallResult::Ok => Ok(()),
        _ => Err(SyscallError::BadSyscall),
    }
}

/// `IPC_RECV`: authorize `RIGHT_READ` on `(slot, generation)`, then dequeue
/// the oldest message. On an empty queue this really blocks the caller in
/// `kernel.sched2` and returns `SyscallError::Ipc(IpcError::QueueEmpty)`
/// (see `kernel::ipc`'s module docs — not a fatal condition).
pub fn recv(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    slot: u32,
    generation: u32,
) -> Result<(ProcessId, crate::platform::Vec<u8>), SyscallError> {
    syscall::ensure_running(kernel, caller)?;
    let args = SyscallArgs {
        cap_slot: slot,
        cap_generation: generation,
        ..Default::default()
    };
    match syscall::dispatch(kernel, caller, io, IPC_RECV, &args)? {
        SyscallResult::Message(sender, data) => Ok((sender, data)),
        _ => Err(SyscallError::BadSyscall),
    }
}

/// `IPC_DESTROY_QUEUE`: authorize `RIGHT_DELETE` on `(slot, generation)`,
/// destroy its `captable` object, then destroy the real `kernel.ipc` queue.
pub fn destroy(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    slot: u32,
    generation: u32,
) -> Result<(), SyscallError> {
    syscall::ensure_running(kernel, caller)?;
    let args = SyscallArgs {
        cap_slot: slot,
        cap_generation: generation,
        ..Default::default()
    };
    match syscall::dispatch(kernel, caller, io, IPC_DESTROY_QUEUE, &args)? {
        SyscallResult::Ok => Ok(()),
        _ => Err(SyscallError::BadSyscall),
    }
}

/// `kernel.ipc`'s live queue-count and send/recv counters (mirrors
/// `sched2_cmd::render`).
pub fn render(kernel: &Kernel) -> String {
    #[cfg(all(feature = "ipc", any(feature = "std", feature = "baremetal")))]
    {
        let stats = kernel.ipc.stats();
        format!(
            "ipc\n\
             queues_live: {}\n\
             queues_created: {}\n\
             queues_destroyed: {}\n\
             generation_retired: {}\n\
             sent: {}\n\
             send_rejected_full: {}\n\
             send_rejected_too_large: {}\n\
             received: {}\n\
             recv_would_block: {}",
            kernel.ipc.queue_count(),
            stats.queues_created,
            stats.queues_destroyed,
            stats.generation_retired,
            stats.sent,
            stats.send_rejected_full,
            stats.send_rejected_too_large,
            stats.received,
            stats.recv_would_block,
        )
    }
    #[cfg(not(all(feature = "ipc", any(feature = "std", feature = "baremetal"))))]
    {
        let _ = kernel;
        "ipc unavailable (ipc feature not enabled)".into()
    }
}

pub fn format_ipc_error(err: &SyscallError) -> String {
    format!("error: {err}")
}
