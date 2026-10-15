//! Hosted OS runner — integrates kernel, VM, timer, shell, and PhaseFS.

use crate::core::gphase_core::PhaseState;
use crate::kernel::event_log::ProcessId;
use crate::kernel::syscall::IoBuffer;
use crate::platform::{String, Vec};
use crate::runtime::loader::Loader;
use crate::runtime::timer::Timer;
use crate::runtime::trace::{TraceEvent, TraceLog};
use crate::userland::hosted::Hosted;
use crate::userland::shell::{ShellAction, ShellError};
use crate::userland::syscall::{self, SyscallError};
use core::fmt;

const VM_CYCLES_PER_STEP: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    Shell(ShellError),
    Syscall(SyscallError),
    Kernel(&'static str),
    Boot(&'static str),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::Shell(e) => write!(f, "shell: {e:?}"),
            RuntimeError::Syscall(e) => write!(f, "syscall: {e}"),
            RuntimeError::Kernel(msg) => write!(f, "kernel: {msg}"),
            RuntimeError::Boot(msg) => write!(f, "boot: {msg}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RuntimeError {}

impl From<ShellError> for RuntimeError {
    fn from(value: ShellError) -> Self {
        RuntimeError::Shell(value)
    }
}

impl From<SyscallError> for RuntimeError {
    fn from(value: SyscallError) -> Self {
        RuntimeError::Syscall(value)
    }
}

pub struct Runtime {
    pub hosted: Hosted,
    pub timer: Timer,
    pub trace: TraceLog,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            hosted: Hosted::new(),
            timer: Timer::default(),
            trace: TraceLog::default(),
        }
    }

    pub fn with_input<I, S>(lines: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut rt = Self::new();
        rt.hosted.io = IoBuffer::with_input(lines);
        rt
    }

    #[cfg(feature = "std")]
    pub fn interactive() -> Self {
        let mut rt = Self::new();
        rt.hosted.io = IoBuffer::std_interactive();
        rt
    }

    pub fn output_lines(&self) -> &[String] {
        self.hosted.output_lines()
    }

    pub fn boot(&mut self) -> Result<ProcessId, RuntimeError> {
        self.init_memory();
        self.mount_phasefs();
        let pid = Loader::boot_init(&mut self.hosted);
        #[cfg(all(feature = "identity", any(feature = "std", feature = "baremetal")))]
        {
            self.hosted.kernel.authority.boot_session_root =
                crate::kernel::boot_session::compute_boot_session_root(
                    &self.hosted.kernel.statechain.equation_root(),
                    pid,
                    &self.hosted.kernel.statechain.kernel_root(),
                    self.hosted.kernel.statechain.tick(),
                );
        }
        self.trace.push(TraceEvent::Boot { init_pid: pid });
        Ok(pid)
    }

    fn init_memory(&mut self) {
        for _ in 0..4 {
            let page = self.hosted.kernel.alloc_page();
            let _ = self.hosted.kernel.map_user(page);
        }
    }

    fn mount_phasefs(&mut self) {
        let _ = self.hosted.kernel.fs.mkdir("/tmp");
    }

    pub fn run(&mut self) -> Result<(), RuntimeError> {
        let pid = self
            .hosted
            .init_pid
            .ok_or(RuntimeError::Boot("call boot() first"))?;

        syscall::ensure_running(&mut self.hosted.kernel, pid).map_err(RuntimeError::Syscall)?;
        let init_pid = pid;
        let info = crate::userland::splash::BootSplashInfo::from_kernel(
            &self.hosted.kernel,
            init_pid,
            true,
        );
        syscall::write(
            &mut self.hosted.kernel,
            pid,
            &mut self.hosted.io,
            &crate::userland::splash::render_boot_screen(&info),
        )
        .map_err(RuntimeError::Syscall)?;

        while let Some(line) = self.hosted.io.read_line() {
            if self.timer.tick() {
                self.timer_preempt()?;
            }
            self.run_vm_cycles(VM_CYCLES_PER_STEP)?;

            syscall::ensure_running(&mut self.hosted.kernel, pid).map_err(RuntimeError::Syscall)?;
            syscall::write(
                &mut self.hosted.kernel,
                pid,
                &mut self.hosted.io,
                &format!("{}{line}", self.hosted.shell.prompt),
            )
            .map_err(RuntimeError::Syscall)?;

            let mut session = crate::userland::shell::ShellSession {
                kernel: &mut self.hosted.kernel,
                caller: pid,
                io: &mut self.hosted.io,
                loader: &self.hosted.programs,
                vms: &mut self.hosted.vms,
                state: &mut self.hosted.session_state,
            };
            // A single failed command (e.g. `cat` of a path that does not
            // exist) is never kernel-fatal — print it and return to the
            // prompt, instead of propagating it out of this loop and exiting
            // the whole runtime.
            let action = match self.hosted.shell.execute_session(&mut session, &line) {
                Ok(action) => action,
                Err(err) => {
                    session.io.write_line(format!("error: {err}"));
                    ShellAction::Continue
                }
            };
            match action {
                ShellAction::Continue => {}
                ShellAction::RunSshServer { .. } => {
                    // Bare-metal SSH server is a board-only action.
                }
                ShellAction::RunAiPhaseLink { .. } => {}
                ShellAction::Exit => {
                    syscall::ensure_running(&mut self.hosted.kernel, pid)
                        .map_err(RuntimeError::Syscall)?;
                    syscall::exit(&mut self.hosted.kernel, pid, &mut self.hosted.io)
                        .map_err(RuntimeError::Syscall)?;
                    self.record_topology_events();
                    return Ok(());
                }
            }
            self.record_topology_events();
        }
        Ok(())
    }

    pub fn spawn_background(&mut self) -> Result<ProcessId, RuntimeError> {
        let parent = self
            .hosted
            .init_pid
            .ok_or(RuntimeError::Boot("not booted"))?;
        syscall::ensure_running(&mut self.hosted.kernel, parent).map_err(RuntimeError::Syscall)?;
        let child = syscall::spawn(&mut self.hosted.kernel, parent, &mut self.hosted.io)
            .map_err(RuntimeError::Syscall)?;
        self.hosted
            .kernel
            .wake(child)
            .map_err(RuntimeError::Kernel)?;
        self.hosted
            .kernel
            .schedule(parent)
            .map_err(RuntimeError::Kernel)?
            .then_some(())
            .ok_or(RuntimeError::Kernel("schedule rejected"))?;
        Ok(child)
    }

    pub fn force_timer_tick(&mut self) -> Result<(), RuntimeError> {
        self.timer.ticks += 1;
        self.timer_preempt()
    }

    pub fn run_command(&mut self, line: &str) -> Result<(), RuntimeError> {
        let pid = self
            .hosted
            .init_pid
            .ok_or(RuntimeError::Boot("not booted"))?;
        syscall::ensure_running(&mut self.hosted.kernel, pid).map_err(RuntimeError::Syscall)?;
        let mut session = crate::userland::shell::ShellSession {
            kernel: &mut self.hosted.kernel,
            caller: pid,
            io: &mut self.hosted.io,
            loader: &self.hosted.programs,
            vms: &mut self.hosted.vms,
            state: &mut self.hosted.session_state,
        };
        self.hosted
            .shell
            .execute_session(&mut session, line)
            .map_err(RuntimeError::Shell)?;
        self.record_topology_events();
        Ok(())
    }

    fn timer_preempt(&mut self) -> Result<(), RuntimeError> {
        self.hosted.kernel.advance_chronos();
        self.trace.push(TraceEvent::TimerTick {
            tick: self.timer.ticks,
        });

        let running: Vec<ProcessId> = self
            .hosted
            .kernel
            .process_ids()
            .into_iter()
            .filter(|pid| {
                self.hosted
                    .kernel
                    .get_process(*pid)
                    .map(|p| p.state == PhaseState::Phi2)
                    .unwrap_or(false)
            })
            .collect();

        for pid in running {
            self.hosted
                .kernel
                .preempt(pid)
                .map_err(RuntimeError::Kernel)?
                .then_some(())
                .ok_or(RuntimeError::Kernel("preempt rejected"))?;
            self.trace.push(TraceEvent::Preempt { pid });
        }

        if let Some(next) = self.hosted.kernel.process_ids().into_iter().find(|pid| {
            self.hosted
                .kernel
                .get_process(*pid)
                .map(|p| p.state == PhaseState::Phi1)
                .unwrap_or(false)
        }) {
            self.hosted
                .kernel
                .schedule(next)
                .map_err(RuntimeError::Kernel)?
                .then_some(())
                .ok_or(RuntimeError::Kernel("schedule rejected"))?;
            self.trace.push(TraceEvent::Schedule { pid: next });
        }

        Ok(())
    }

    fn run_vm_cycles(&mut self, max: usize) -> Result<(), RuntimeError> {
        for &pid in self.hosted.vms.keys() {
            self.trace.push(TraceEvent::VmCycles { pid, cycles: max });
        }
        Ok(())
    }

    fn record_topology_events(&mut self) {
        for event in self.hosted.kernel.topology_report() {
            if !self
                .trace
                .events
                .iter()
                .any(|e| matches!(e, TraceEvent::Topology(t) if t == &event))
            {
                self.trace.push(TraceEvent::Topology(event));
            }
        }
    }
}
