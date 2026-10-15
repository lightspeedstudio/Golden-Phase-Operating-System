//! Shared command context.

use crate::kernel::authority::{AuthorityMode, AuthorityState};
use crate::kernel::event_log::ProcessId;
use crate::kernel::syscall::IoBuffer;
use crate::kernel::Kernel;
use crate::platform::HashMap;
use crate::userland::run::ProgramLoader;
use crate::userland::syscall;
use crate::userland::tools::error::ToolError;
use crate::userland::tools::session_state::ShellSessionState;
use crate::userland::tools::types::{AuthorityRequirement, RuntimeTarget};
use crate::vm::Cpu;

pub struct CommandContext<'a> {
    pub kernel: &'a mut Kernel,
    pub caller: ProcessId,
    pub io: &'a mut IoBuffer,
    pub loader: &'a ProgramLoader,
    pub vms: &'a mut HashMap<ProcessId, Cpu>,
    pub session: &'a mut ShellSessionState,
    pub target: RuntimeTarget,
    bytes_written: usize,
}

impl<'a> CommandContext<'a> {
    pub fn new(
        kernel: &'a mut Kernel,
        caller: ProcessId,
        io: &'a mut IoBuffer,
        loader: &'a ProgramLoader,
        vms: &'a mut HashMap<ProcessId, Cpu>,
        session: &'a mut ShellSessionState,
    ) -> Self {
        Self {
            kernel,
            caller,
            io,
            loader,
            vms,
            session,
            target: crate::userland::tools::types::current_runtime_target(),
            bytes_written: 0,
        }
    }

    pub fn ensure_running(&mut self) -> Result<(), ToolError> {
        syscall::ensure_running(self.kernel, self.caller)
            .map_err(|e| ToolError::InternalError(format!("{e:?}")))
    }

    pub fn writeln(&mut self, text: &str) -> Result<(), ToolError> {
        if self.bytes_written.saturating_add(text.len()) > self.session.output_limit {
            return Err(ToolError::OutputLimitExceeded);
        }
        self.ensure_running()?;
        syscall::write(self.kernel, self.caller, self.io, text)
            .map_err(|e| ToolError::InternalError(format!("{e:?}")))?;
        self.bytes_written = self.bytes_written.saturating_add(text.len());
        Ok(())
    }

    pub fn writeln_line(&mut self, text: &str) -> Result<(), ToolError> {
        self.writeln(&format!("{text}\n"))
    }

    pub fn session_authority(&self) -> AuthorityRequirement {
        authority_from_state(&self.kernel.authority)
    }

    pub fn check_authority(&self, need: AuthorityRequirement) -> Result<(), ToolError> {
        if need.allows(self.session_authority()) {
            Ok(())
        } else {
            Err(ToolError::AuthorityDenied)
        }
    }

    pub fn check_target(
        &self,
        support: crate::userland::tools::types::TargetSupport,
    ) -> Result<(), ToolError> {
        if support.available_on(self.target) {
            Ok(())
        } else {
            Err(ToolError::UnsupportedTarget)
        }
    }

    pub fn record_statechain(&mut self, event: &str, detail: &[u8]) {
        #[cfg(any(feature = "std", feature = "baremetal"))]
        {
            let _ = self.kernel.statechain.record_transition(
                crate::kernel::state_proof::ObjectKind::Driver,
                11,
                crate::kernel::state_root::SubsystemKind::Driver,
                crate::core::gphase_core::PhaseState::Phi2,
                crate::core::gphase_core::PhaseState::Phi3,
                event,
                None,
                detail,
            );
        }
        let _ = event;
        let _ = detail;
    }
}

pub fn authority_from_state(state: &AuthorityState) -> AuthorityRequirement {
    match state.authority {
        AuthorityMode::Root => AuthorityRequirement::Root,
        AuthorityMode::Anom => AuthorityRequirement::AnomAdministrator,
    }
}
