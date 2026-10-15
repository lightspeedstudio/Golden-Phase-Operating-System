//! Read-only AI tools executed only after KairOS authorization.

use crate::core::gphase_core::PhaseState;
use crate::kernel::ai::error::AiError;
use crate::kernel::ai::permissions::tool_permission;
use crate::kernel::ai::types::{AiBridgeController, ToolPermission};
use crate::kernel::Kernel;
use crate::platform::String;

pub fn execute_tool(
    kernel: &Kernel,
    controller: &AiBridgeController,
    tool: &str,
    arguments: &str,
) -> Result<String, AiError> {
    match tool_permission(controller, tool) {
        Some(ToolPermission::Allowed) => {}
        Some(ToolPermission::Denied) | None => return Err(AiError::ToolDenied),
    }
    match tool {
        "system.state" => Ok(format!(
            "authority_anom={} processes={}",
            kernel.authority.is_anom_protected(),
            kernel.scheduler.process_ids().len()
        )),
        "system.version" => Ok("Golden Phase Operating System 0.1.0 (GPOS)".into()),
        "process.list" => {
            let mut out = String::new();
            for pid in kernel.scheduler.process_ids() {
                if let Ok(proc) = kernel.scheduler.get_process(pid) {
                    out.push_str(&format!(
                        "pid={} name={} phase={:?}\n",
                        proc.pid, proc.name, proc.state
                    ));
                }
            }
            Ok(out)
        }
        "process.show" => {
            let pid = arguments
                .trim()
                .parse::<u32>()
                .map_err(|_| AiError::MalformedResponse)?;
            let proc = kernel
                .scheduler
                .get_process(pid)
                .map_err(|_| AiError::JobNotFound)?;
            Ok(format!(
                "pid={} name={} phase={:?}",
                proc.pid, proc.name, proc.state
            ))
        }
        "phase.counts" => {
            let mut counts = [0u64; 4];
            for pid in kernel.scheduler.process_ids() {
                if let Ok(proc) = kernel.scheduler.get_process(pid) {
                    counts[proc.state.as_u8() as usize] += 1;
                }
            }
            Ok(format!(
                "Phi0={} Phi1={} Phi2={} Phi3={}",
                counts[0], counts[1], counts[2], counts[3]
            ))
        }
        "trace.read" => {
            let mut out = String::new();
            for rec in kernel.trace.recent().iter().take(16) {
                out.push_str(&format!("{rec:?}\n"));
            }
            Ok(out)
        }
        "topology.read" => {
            let mut out = String::new();
            for event in kernel.scheduler.topology_report().iter().take(16) {
                out.push_str(&format!("{event:?}\n"));
            }
            Ok(out)
        }
        "memory.summary" => Ok(format!(
            "pages_tracked={}",
            kernel.memory.tag_report().len()
        )),
        _ => Err(AiError::ToolNotFound),
    }
}

pub fn authorize_tool_gate(job_phase: PhaseState) -> Result<(), AiError> {
    if job_phase != PhaseState::Phi3 {
        return Err(AiError::InvalidPhase);
    }
    Ok(())
}
