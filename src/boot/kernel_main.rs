//! Bare-metal entry — boots the same kernel/shell stack over UART.

use crate::boot::arch::riscv64::{self, timer, uart};
use crate::boot::heap;
use crate::boot::serial;
use crate::runtime::Runtime;
use crate::userland::shell::{ShellAction, ShellError};
use crate::userland::splash::{self, BootSplashInfo};
use crate::userland::syscall::{self, SyscallError};
use core::fmt;

pub fn kernel_main() -> ! {
    riscv64::init();
    heap::init();

    let mut runtime = Runtime::new();
    match runtime.boot() {
        Ok(init_pid) => {
            #[cfg(all(feature = "bm-net", target_arch = "riscv64"))]
            {
                runtime.hosted.kernel.network = riscv64::virtio_net::probe();
            }
            let info = BootSplashInfo::from_kernel(&runtime.hosted.kernel, init_pid, false);
            uart::write_str(&splash::render_boot_screen(&info));
        }
        Err(err) => {
            uart::write_str(&alloc::format!("boot failed: {err}\n"));
            halt();
        }
    }

    riscv64::late_init();

    if let Err(err) = run_serial_shell(&mut runtime) {
        uart::write_str(&alloc::format!("runtime failed: {err}\n"));
    }

    uart::write_str("KairOS: halt\n");
    halt();
}

fn run_serial_shell(runtime: &mut Runtime) -> Result<(), BootError> {
    uart::write_str(&runtime.hosted.shell.prompt);

    let mut flushed = 0usize;
    loop {
        timer::poll();
        if timer::take_pending() {
            runtime.force_timer_tick().map_err(BootError::Runtime)?;
        }

        let line = serial::read_line();
        if line.is_empty() {
            continue;
        }

        let pid = runtime.hosted.init_pid.ok_or(BootError::Boot("no init"))?;
        runtime.hosted.kernel.advance_chronos();
        let state = &mut runtime.hosted.session_state;
        let mut session = crate::userland::shell::ShellSession {
            kernel: &mut runtime.hosted.kernel,
            caller: pid,
            io: &mut runtime.hosted.io,
            loader: &runtime.hosted.programs,
            vms: &mut runtime.hosted.vms,
            state,
        };

        syscall::ensure_running(session.kernel, pid).map_err(BootError::Syscall)?;
        // A single failed command (e.g. `cat` of a path that does not exist)
        // is never kernel-fatal — print it and return to the prompt, instead
        // of propagating it out of this loop and halting the whole kernel.
        let action = match runtime.hosted.shell.execute_session(&mut session, &line) {
            Ok(action) => action,
            Err(err) => {
                session.io.write_line(alloc::format!("error: {err}"));
                ShellAction::Continue
            }
        };
        match action {
            ShellAction::Continue => {}
            ShellAction::RunSshServer { .. } => {
                // Bare-metal SSH is served by the Zynq board boot loop only.
            }
            ShellAction::RunAiPhaseLink { .. } => {}
            ShellAction::Exit => {
                syscall::ensure_running(&mut runtime.hosted.kernel, pid)
                    .map_err(BootError::Syscall)?;
                syscall::exit(&mut runtime.hosted.kernel, pid, &mut runtime.hosted.io)
                    .map_err(BootError::Syscall)?;
                serial::flush_new(&runtime.hosted.io, flushed);
                return Ok(());
            }
        }
        serial::flush_new(&runtime.hosted.io, flushed);
        flushed = runtime.hosted.io.output.len();
        emit_prompt(runtime, pid, &mut flushed)?;
    }
}

fn emit_prompt(runtime: &mut Runtime, pid: u32, flushed: &mut usize) -> Result<(), BootError> {
    syscall::ensure_running(&mut runtime.hosted.kernel, pid).map_err(BootError::Syscall)?;
    syscall::write(
        &mut runtime.hosted.kernel,
        pid,
        &mut runtime.hosted.io,
        &runtime.hosted.shell.prompt,
    )
    .map_err(BootError::Syscall)?;
    serial::flush_new(&runtime.hosted.io, *flushed);
    *flushed = runtime.hosted.io.output.len();
    Ok(())
}

fn halt() -> ! {
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootError {
    Boot(&'static str),
    Shell(ShellError),
    Syscall(SyscallError),
    Runtime(crate::runtime::host::RuntimeError),
}

impl fmt::Display for BootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BootError::Boot(msg) => write!(f, "boot: {msg}"),
            BootError::Shell(e) => write!(f, "shell: {e:?}"),
            BootError::Syscall(e) => write!(f, "syscall: {e}"),
            BootError::Runtime(e) => write!(f, "runtime: {e}"),
        }
    }
}

impl From<ShellError> for BootError {
    fn from(value: ShellError) -> Self {
        BootError::Shell(value)
    }
}

impl From<SyscallError> for BootError {
    fn from(value: SyscallError) -> Self {
        BootError::Syscall(value)
    }
}

impl From<crate::runtime::host::RuntimeError> for BootError {
    fn from(value: crate::runtime::host::RuntimeError) -> Self {
        BootError::Runtime(value)
    }
}
