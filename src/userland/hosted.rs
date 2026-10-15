//! Hosted OS state — kernel, terminal I/O, shell, and VM programs.

use crate::kernel::event_log::ProcessId;
use crate::kernel::syscall::IoBuffer;
use crate::kernel::Kernel;
use crate::platform::{HashMap, String};
use crate::userland::run::ProgramLoader;
use crate::userland::shell::Shell;
use crate::vm::Cpu;

pub struct Hosted {
    pub kernel: Kernel,
    pub io: IoBuffer,
    pub programs: ProgramLoader,
    pub shell: Shell,
    pub session_state: crate::userland::tools::ShellSessionState,
    pub vms: HashMap<ProcessId, Cpu>,
    pub init_pid: Option<ProcessId>,
}

impl Default for Hosted {
    fn default() -> Self {
        Self::new()
    }
}

impl Hosted {
    pub fn new() -> Self {
        Self {
            kernel: Kernel::new(),
            io: IoBuffer::default(),
            programs: ProgramLoader::new(),
            shell: Shell::new(),
            session_state: crate::userland::tools::ShellSessionState::new(),
            vms: HashMap::new(),
            init_pid: None,
        }
    }

    pub fn with_input<I, S>(lines: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut hosted = Self::new();
        hosted.io = IoBuffer::with_input(lines);
        hosted
    }

    pub fn output_lines(&self) -> &[String] {
        &self.io.output
    }

    pub fn execute_line(
        &mut self,
        pid: ProcessId,
        line: &str,
    ) -> Result<crate::userland::shell::ShellAction, crate::userland::shell::ShellError> {
        self.shell.execute_line(
            &mut self.kernel,
            pid,
            &mut self.io,
            &self.programs,
            &mut self.vms,
            &mut self.session_state,
            line,
        )
    }
}
