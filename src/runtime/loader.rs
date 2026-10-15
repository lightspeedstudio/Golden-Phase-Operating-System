//! Program and init loader for the hosted runtime.

use crate::kernel::event_log::ProcessId;
use crate::platform::{String, ToString};
use crate::userland::hosted::Hosted;
use crate::userland::init;
use crate::vm::Cpu;
use core::fmt;

pub struct Loader;

impl Loader {
    pub fn boot_init(hosted: &mut Hosted) -> ProcessId {
        init::boot(hosted)
    }

    pub fn attach_program(
        hosted: &mut Hosted,
        pid: ProcessId,
        name: &str,
    ) -> Result<(), LoaderError> {
        let program = hosted
            .programs
            .get(name)
            .ok_or_else(|| LoaderError::UnknownProgram(name.to_string()))?
            .to_vec();
        #[cfg(feature = "phasesentinel")]
        {
            let bytes = crate::vm::instruction::encode_program(&program);
            hosted
                .kernel
                .sentinel_authorize_launch(pid, &bytes)
                .map_err(LoaderError::Sentinel)?;
        }
        hosted.vms.insert(pid, Cpu::new(256));
        if let Some(cpu) = hosted.vms.get_mut(&pid) {
            cpu.run(&program, 10_000).map_err(LoaderError::Vm)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoaderError {
    UnknownProgram(String),
    Vm(crate::vm::VmError),
    #[cfg(feature = "phasesentinel")]
    Sentinel(crate::kernel::sentinel::SentinelError),
}

impl fmt::Display for LoaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoaderError::UnknownProgram(name) => write!(f, "unknown program {name}"),
            LoaderError::Vm(err) => write!(f, "{err}"),
            #[cfg(feature = "phasesentinel")]
            LoaderError::Sentinel(err) => write!(f, "{}", err.as_str()),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for LoaderError {}
