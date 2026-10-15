//! ps — list process states.

use crate::kernel::event_log::ProcessId;
use crate::kernel::syscall::IoBuffer;
use crate::kernel::Kernel;
use crate::platform::{String, Vec};
use crate::userland::syscall;

pub fn render(kernel: &Kernel) -> Result<String, syscall::SyscallError> {
    let anom = kernel.authority.is_anom_protected();
    #[cfg(feature = "std")]
    let obj_prot = kernel.object_protection.registry.protected_count() > 0;
    #[cfg(not(feature = "std"))]
    let obj_prot = false;
    let show_commitments = anom || obj_prot;
    let mut lines = Vec::new();
    if show_commitments {
        if obj_prot {
            lines.push("PID\tNAME\tPUBLIC_STATE\tPROTECTION\tCHAIN".into());
        } else {
            lines.push("PID\tNAME\tPUBLIC_STATE\tCHAIN".into());
        }
    } else {
        lines.push("PID\tNAME\tSTATE\tTICKS\tSTATUS".into());
    }
    for pid in kernel.process_ids() {
        let proc = kernel
            .get_process(pid)
            .map_err(|_| syscall::SyscallError::BadTarget)?;
        if show_commitments {
            let public = kernel.public_process_view(pid).render_short();
            let chain = chain_status(kernel, pid);
            #[cfg(feature = "std")]
            let protection = kernel
                .object_protection
                .protection_label_for_process(pid, anom);
            if obj_prot {
                #[cfg(feature = "std")]
                lines.push(format!(
                    "{}\t{}\t{}\t{}\t{}",
                    pid, proc.name, public, protection, chain
                ));
                #[cfg(not(feature = "std"))]
                lines.push(format!("{}\t{}\t{}\t{}", pid, proc.name, public, chain));
            } else {
                lines.push(format!("{}\t{}\t{}\t{}", pid, proc.name, public, chain));
            }
        } else {
            let ticks = proc.history.len().saturating_sub(1);
            lines.push(format!(
                "{}\t{}\t{}\t{}\t{}",
                pid,
                proc.name,
                phase_name(proc.state),
                ticks,
                status_label(proc.state)
            ));
        }
    }
    Ok(lines.join("\n"))
}

#[cfg(feature = "std")]
fn chain_status(kernel: &Kernel, pid: ProcessId) -> &'static str {
    use crate::kernel::state_proof::ObjectKind;
    kernel
        .statechain
        .chain(ObjectKind::Process, pid)
        .map(|c| if c.verify().is_ok() { "OK" } else { "FAIL" })
        .unwrap_or("—")
}

#[cfg(not(feature = "std"))]
fn chain_status(_kernel: &Kernel, _pid: ProcessId) -> &'static str {
    "—"
}

pub fn run(
    kernel: &mut Kernel,
    caller: ProcessId,
    io: &mut IoBuffer,
) -> Result<String, syscall::SyscallError> {
    let output = render(kernel)?;
    syscall::ensure_running(kernel, caller)?;
    syscall::write(kernel, caller, io, &output)?;
    Ok(output)
}

fn phase_name(state: crate::core::gphase_core::PhaseState) -> &'static str {
    state.kairos_label()
}

fn status_label(state: crate::core::gphase_core::PhaseState) -> &'static str {
    match state {
        crate::core::gphase_core::PhaseState::Phi0 => "sleeping",
        crate::core::gphase_core::PhaseState::Phi1 => "ready",
        crate::core::gphase_core::PhaseState::Phi2 => "running",
        crate::core::gphase_core::PhaseState::Phi3 => "kernel",
    }
}
