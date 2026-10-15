//! shell — hosted command loop.

use crate::kernel::event_log::ProcessId;
use crate::kernel::syscall::IoBuffer;
use crate::platform::HashMap;
use crate::platform::String;
use crate::userland::commands::dispatch;
use crate::userland::context::ShellContext;
use crate::userland::parser::parse;
use crate::userland::run::ProgramLoader;
use crate::userland::syscall;
use crate::vm::Cpu;
use core::fmt;

pub struct ShellSession<'a> {
    pub kernel: &'a mut crate::kernel::Kernel,
    pub caller: ProcessId,
    pub io: &'a mut IoBuffer,
    pub loader: &'a ProgramLoader,
    pub vms: &'a mut HashMap<ProcessId, Cpu>,
    pub state: &'a mut crate::userland::tools::ShellSessionState,
}

pub struct Shell {
    pub prompt: String,
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

impl Shell {
    pub fn new() -> Self {
        Self {
            prompt: crate::brand::SHELL_PROMPT.into(),
        }
    }

    pub fn execute_line(
        &mut self,
        kernel: &mut crate::kernel::Kernel,
        caller: ProcessId,
        io: &mut IoBuffer,
        loader: &ProgramLoader,
        vms: &mut HashMap<ProcessId, Cpu>,
        session_state: &mut crate::userland::tools::ShellSessionState,
        line: &str,
    ) -> Result<ShellAction, ShellError> {
        self.execute_session(
            &mut ShellSession {
                kernel,
                caller,
                io,
                loader,
                vms,
                state: session_state,
            },
            line,
        )
    }

    pub fn execute_session(
        &mut self,
        session: &mut ShellSession<'_>,
        line: &str,
    ) -> Result<ShellAction, ShellError> {
        syscall::ensure_running(session.kernel, session.caller).map_err(ShellError::Syscall)?;
        session.kernel.advance_chronos();
        session.state.record_history(line);
        if let Some(result) = crate::userland::tools::try_execute(session, line) {
            return result;
        }
        let cmd = parse(line);
        let mut ctx = ShellContext::new(
            session.kernel,
            session.caller,
            session.io,
            session.loader,
            session.vms,
        );
        dispatch::execute(&mut ctx, cmd)
    }

    pub fn run_session(
        &mut self,
        kernel: &mut crate::kernel::Kernel,
        caller: ProcessId,
        io: &mut IoBuffer,
        loader: &ProgramLoader,
        vms: &mut HashMap<ProcessId, Cpu>,
        session_state: &mut crate::userland::tools::ShellSessionState,
    ) -> Result<(), ShellError> {
        let mut session = ShellSession {
            kernel,
            caller,
            io,
            loader,
            vms,
            state: session_state,
        };
        syscall::ensure_running(session.kernel, session.caller).map_err(ShellError::Syscall)?;
        syscall::write(
            session.kernel,
            session.caller,
            session.io,
            &format!(
                "{}\ntype help for commands",
                crate::userland::splash::render_with_equation()
            ),
        )?;
        while let Some(line) = session.io.read_line() {
            syscall::ensure_running(session.kernel, session.caller).map_err(ShellError::Syscall)?;
            syscall::write(
                session.kernel,
                session.caller,
                session.io,
                &format!("{}{line}", self.prompt),
            )?;
            match self.execute_session(&mut session, &line)? {
                ShellAction::Continue => {}
                ShellAction::RunSshServer { .. } => {
                    // Bare-metal SSH is launched by the board boot loop only.
                }
                ShellAction::RunAiPhaseLink { .. } => {}
                ShellAction::Exit => {
                    syscall::ensure_running(session.kernel, session.caller)
                        .map_err(ShellError::Syscall)?;
                    syscall::exit(session.kernel, session.caller, session.io)
                        .map_err(ShellError::Syscall)?;
                    return Ok(());
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellAction {
    Continue,
    Exit,
    /// Launch the bare-metal SSH server (handled by the board boot loop, which
    /// owns the shell session — avoids re-entering `execute_session`).
    RunSshServer {
        octets: [u8; 4],
        port: u16,
        secs: u32,
    },
    /// Bounded PhaseLink AI inbound poll (board boot loop).
    RunAiPhaseLink {
        secs: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellError {
    Syscall(crate::kernel::syscall::SyscallError),
    Run(crate::userland::run::RunError),
    Unknown(String),
    Fs(String),
}

impl From<crate::kernel::syscall::SyscallError> for ShellError {
    fn from(value: crate::kernel::syscall::SyscallError) -> Self {
        ShellError::Syscall(value)
    }
}

impl From<crate::userland::run::RunError> for ShellError {
    fn from(value: crate::userland::run::RunError) -> Self {
        ShellError::Run(value)
    }
}

/// A single failed command's own error message — never a kernel-fatal
/// condition (see the `run_serial_shell`/`Runtime::run` loops, which print
/// this and return to the prompt rather than halting).
impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShellError::Syscall(err) => write!(f, "{err}"),
            ShellError::Run(err) => write!(f, "{err}"),
            ShellError::Unknown(msg) => write!(f, "{msg}"),
            ShellError::Fs(msg) => write!(f, "{msg}"),
        }
    }
}
