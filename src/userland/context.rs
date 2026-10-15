//! Shell execution context — terminal I/O without direct kernel mutation.

use crate::kernel::event_log::ProcessId;
use crate::kernel::syscall::IoBuffer;
use crate::kernel::Kernel;
use crate::platform::HashMap;
use crate::userland::run::ProgramLoader;
use crate::userland::shell::ShellError;
use crate::userland::syscall;
use crate::vm::Cpu;

pub struct ShellContext<'a> {
    pub kernel: &'a mut Kernel,
    pub caller: ProcessId,
    pub io: &'a mut IoBuffer,
    pub loader: &'a ProgramLoader,
    pub vms: &'a mut HashMap<ProcessId, Cpu>,
}

impl<'a> ShellContext<'a> {
    pub fn new(
        kernel: &'a mut Kernel,
        caller: ProcessId,
        io: &'a mut IoBuffer,
        loader: &'a ProgramLoader,
        vms: &'a mut HashMap<ProcessId, Cpu>,
    ) -> Self {
        Self {
            kernel,
            caller,
            io,
            loader,
            vms,
        }
    }

    pub fn ensure_running(&mut self) -> Result<(), ShellError> {
        syscall::ensure_running(self.kernel, self.caller).map_err(ShellError::Syscall)
    }

    pub fn writeln(&mut self, text: &str) -> Result<(), ShellError> {
        self.ensure_running()?;
        syscall::write(self.kernel, self.caller, self.io, text)?;
        Ok(())
    }

    pub fn writeln_line(&mut self, text: &str) -> Result<(), ShellError> {
        self.writeln(&format!("{text}\n"))
    }

    pub fn log_error(&mut self, message: impl Into<crate::platform::String>) {
        let message = message.into();
        self.kernel
            .trace
            .record(crate::kernel::trace::TraceRecord::Error {
                message: message.clone(),
            });
        let _ = self.writeln_line(&message);
    }
}
