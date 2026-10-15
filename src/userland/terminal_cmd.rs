//! Terminal command dispatch helpers.

use crate::kernel::event_log::ProcessId;
use crate::kernel::syscall::IoBuffer;
use crate::kernel::Kernel;
use crate::platform::{HashMap, String};
use crate::userland::audit_cmd;
use crate::userland::captable_cmd;
use crate::userland::checkpoint_cmd;
use crate::userland::dashboard;
use crate::userland::extmap_cmd;
use crate::userland::irqreg_cmd;
use crate::userland::netsess_cmd;
use crate::userland::phase_cache_cmd;
use crate::userland::phase_view;
use crate::userland::pmm_cmd;
use crate::userland::root_view;
use crate::userland::sched2_cmd;
use crate::userland::syscall;
use crate::userland::top::{self, TopOptions};
use crate::userland::topo_view;
use crate::userland::txlog_cmd;
use crate::userland::watchdog_view;
use crate::vm::Cpu;

pub fn run_dashboard(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    emit(kernel, caller, io, &dashboard::render(kernel))
}

pub fn run_top(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    vms: &HashMap<ProcessId, Cpu>,
    opts: TopOptions,
) -> Result<String, syscall::SyscallError> {
    emit(kernel, caller, io, &top::render(kernel, vms, opts))
}

pub fn run_watchdog(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    emit(kernel, caller, io, &watchdog_view::render(kernel))
}

pub fn run_phase(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    emit(kernel, caller, io, &phase_view::render(kernel))
}

pub fn run_topo(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    emit(kernel, caller, io, &topo_view::render(kernel))
}

pub fn run_root(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    emit(kernel, caller, io, &root_view::render(kernel))
}

pub fn run_audit(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    emit(kernel, caller, io, &audit_cmd::render(kernel))
}

pub fn run_checkpoint(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    match checkpoint_cmd::render(kernel) {
        Ok(text) => emit(kernel, caller, io, &text),
        Err(msg) => emit(kernel, caller, io, &msg),
    }
}

pub fn run_phase_cache(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    let text = phase_cache_cmd::render(kernel);
    emit(kernel, caller, io, &text)
}

pub fn run_pmm(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    let text = pmm_cmd::render(kernel);
    emit(kernel, caller, io, &text)
}

pub fn run_captable(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    let text = captable_cmd::render(kernel);
    emit(kernel, caller, io, &text)
}

pub fn run_irqreg(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    let text = irqreg_cmd::render(kernel);
    emit(kernel, caller, io, &text)
}

pub fn run_sched2(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    let text = sched2_cmd::render(kernel);
    emit(kernel, caller, io, &text)
}

pub fn run_extmap(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    let text = extmap_cmd::render(kernel);
    emit(kernel, caller, io, &text)
}

pub fn run_txlog(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    let text = txlog_cmd::render(kernel);
    emit(kernel, caller, io, &text)
}

pub fn run_txlog_demo(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    let text = txlog_cmd::demo(kernel);
    emit(kernel, caller, io, &text)
}

pub fn run_txlog_fault(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    let text = txlog_cmd::fault(kernel);
    emit(kernel, caller, io, &text)
}

pub fn run_txlog_recover(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    let text = txlog_cmd::recover(kernel);
    emit(kernel, caller, io, &text)
}

pub fn run_netsess(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    let text = netsess_cmd::render(kernel);
    emit(kernel, caller, io, &text)
}

pub fn run_netsess_demo(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    let text = netsess_cmd::demo(kernel, caller);
    emit(kernel, caller, io, &text)
}

pub fn run_netsess_fault(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    let text = netsess_cmd::fault(kernel, caller);
    emit(kernel, caller, io, &text)
}

pub fn run_sleep(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    ticks: u64,
) -> Result<String, syscall::SyscallError> {
    syscall::proc_sleep(kernel, caller, io, ticks)?;
    emit(
        kernel,
        caller,
        io,
        &format!("sleeping for {ticks} sched2 tick(s)"),
    )
}

pub fn run_pmm_selftest(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    #[cfg(all(feature = "pmm", any(feature = "std", feature = "baremetal")))]
    let text = pmm_cmd::selftest(kernel);
    #[cfg(not(all(feature = "pmm", any(feature = "std", feature = "baremetal"))))]
    let text = String::from("pmm unavailable (pmm feature not enabled)");
    emit(kernel, caller, io, &text)
}

fn emit(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
    text: &str,
) -> Result<String, syscall::SyscallError> {
    syscall::ensure_running(kernel, caller)?;
    syscall::write(kernel, caller, io, text)?;
    Ok(text.into())
}
