//! Userland syscall wrappers.

use crate::core::gphase_core::PhaseState;
use crate::kernel::event_log::{PageId, ProcessId};
pub use crate::kernel::syscall::SyscallError;
use crate::kernel::syscall::{
    self, IoBuffer, SyscallArgs, SyscallResult, ALLOC_PAGE, CONTENTION_CHECK, EXIT, FREE_PAGE,
    PHASE_STATE, PROC_SLEEP, READ, READ_SECRET, REFLECTION_LOG, SPAWN, TOPOLOGY_LOG, WRITE, YIELD,
};
use crate::kernel::Kernel;
use crate::platform::{String, ToString, Vec};

pub fn ensure_running(kernel: &mut Kernel, caller: ProcessId) -> Result<(), SyscallError> {
    match kernel
        .get_process(caller)
        .map_err(|_| SyscallError::BadTarget)?
        .state
    {
        PhaseState::Phi1 => kernel
            .schedule(caller)
            .map_err(SyscallError::Kernel)?
            .then_some(())
            .ok_or(SyscallError::Kernel("schedule rejected")),
        PhaseState::Phi2 => Ok(()),
        PhaseState::Phi3 => {
            kernel.kreturn(caller).map_err(SyscallError::Kernel)?;
            kernel
                .schedule(caller)
                .map_err(SyscallError::Kernel)?
                .then_some(())
                .ok_or(SyscallError::Kernel("schedule rejected"))
        }
        PhaseState::Phi0 => Err(SyscallError::NotRunning),
    }
}

pub fn write(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    text: &str,
) -> Result<(), SyscallError> {
    let args = SyscallArgs {
        text: text.to_string(),
        ..Default::default()
    };
    match dispatch(kernel, caller, io, WRITE, &args)? {
        SyscallResult::Ok => Ok(()),
        _ => Err(SyscallError::BadSyscall),
    }
}

pub fn read(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, SyscallError> {
    match dispatch(kernel, caller, io, READ, &SyscallArgs::default())? {
        SyscallResult::Text(line) => Ok(line),
        _ => Err(SyscallError::BadSyscall),
    }
}

/// Read one secret-line (e.g. a password) via [`READ_SECRET`] — real
/// no-echo terminal input where a real terminal exists to protect. See
/// `kernel::syscall::IoBuffer::read_secret_line`'s docs.
pub fn read_secret(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, SyscallError> {
    match dispatch(kernel, caller, io, READ_SECRET, &SyscallArgs::default())? {
        SyscallResult::Text(line) => Ok(line),
        _ => Err(SyscallError::BadSyscall),
    }
}

pub fn spawn(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<ProcessId, SyscallError> {
    match dispatch(kernel, caller, io, SPAWN, &SyscallArgs::default())? {
        SyscallResult::Pid(pid) => Ok(pid),
        _ => Err(SyscallError::BadSyscall),
    }
}

pub fn exit(kernel: &mut Kernel, caller: ProcessId, io: &mut IoBuffer) -> Result<(), SyscallError> {
    match dispatch(kernel, caller, io, EXIT, &SyscallArgs::default())? {
        SyscallResult::Ok => Ok(()),
        _ => Err(SyscallError::BadSyscall),
    }
}

pub fn yield_cpu(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<(), SyscallError> {
    match dispatch(kernel, caller, io, YIELD, &SyscallArgs::default())? {
        SyscallResult::Ok => Ok(()),
        _ => Err(SyscallError::BadSyscall),
    }
}

/// `PROC_SLEEP`: block the caller in `kernel.sched2` for `ticks` sched2
/// ticks (see `kernel::sched2`'s module docs for the real time base). A
/// no-op when the `sched2` feature is disabled.
pub fn proc_sleep(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    ticks: u64,
) -> Result<(), SyscallError> {
    let args = SyscallArgs {
        sleep_ticks: ticks,
        ..Default::default()
    };
    match dispatch(kernel, caller, io, PROC_SLEEP, &args)? {
        SyscallResult::Ok => Ok(()),
        _ => Err(SyscallError::BadSyscall),
    }
}

pub fn alloc_page(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<PageId, SyscallError> {
    match dispatch(kernel, caller, io, ALLOC_PAGE, &SyscallArgs::default())? {
        SyscallResult::PageId(page) => Ok(page),
        _ => Err(SyscallError::BadSyscall),
    }
}

pub fn free_page(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    page_id: PageId,
) -> Result<(), SyscallError> {
    let args = SyscallArgs {
        page_id,
        ..Default::default()
    };
    match dispatch(kernel, caller, io, FREE_PAGE, &args)? {
        SyscallResult::Ok => Ok(()),
        _ => Err(SyscallError::BadSyscall),
    }
}

pub fn phase_state(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    target: ProcessId,
) -> Result<PhaseState, SyscallError> {
    let args = SyscallArgs {
        target_pid: target,
        ..Default::default()
    };
    match dispatch(kernel, caller, io, PHASE_STATE, &args)? {
        SyscallResult::PhaseState(state) => Ok(state),
        _ => Err(SyscallError::BadSyscall),
    }
}

pub fn topology_log(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<Vec<crate::kernel::event_log::TopologyEvent>, SyscallError> {
    match dispatch(kernel, caller, io, TOPOLOGY_LOG, &SyscallArgs::default())? {
        SyscallResult::Events(events) => Ok(events),
        _ => Err(SyscallError::BadSyscall),
    }
}

pub fn reflection_log(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<Vec<crate::kernel::event_log::TopologyEvent>, SyscallError> {
    match dispatch(kernel, caller, io, REFLECTION_LOG, &SyscallArgs::default())? {
        SyscallResult::Events(events) => Ok(events),
        _ => Err(SyscallError::BadSyscall),
    }
}

pub fn contention_check(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    other: ProcessId,
) -> Result<PhaseState, SyscallError> {
    let args = SyscallArgs {
        target_pid: other,
        ..Default::default()
    };
    match dispatch(kernel, caller, io, CONTENTION_CHECK, &args)? {
        SyscallResult::PhaseState(state) => Ok(state),
        _ => Err(SyscallError::BadSyscall),
    }
}

pub fn dispatch(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    nr: u32,
    args: &SyscallArgs,
) -> Result<SyscallResult, SyscallError> {
    syscall::dispatch(kernel, caller, io, nr, args)
}

pub fn kill(kernel: &mut Kernel, caller: ProcessId, target: ProcessId) -> Result<(), SyscallError> {
    syscall::kill_process(kernel, caller, target)
}
