//! `top` — live phase monitor (single-shot or repeated frames).

use crate::core::gphase_core::{PhaseState, TopologyKind};
use crate::kernel::event_log::ProcessId;
use crate::kernel::Kernel;
use crate::platform::{HashMap, String, Vec};
use crate::userland::terminal_fmt::{format_phase_totals, phi_label, process_mode};
use crate::userland::terminal_metrics::process_phase_counts;
use crate::vm::Cpu;

#[cfg(feature = "std")]
use crate::kernel::state_proof::ObjectKind;

#[derive(Debug, Clone, Copy, Default)]
pub struct TopOptions {
    pub frames: u32,
}

pub fn render(kernel: &Kernel, vms: &HashMap<ProcessId, Cpu>, opts: TopOptions) -> String {
    let frames = opts.frames.max(1);
    let mut out = String::new();
    for frame in 0..frames {
        if frame > 0 {
            out.push_str("\n--- refresh ---\n");
            #[cfg(feature = "std")]
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        out.push_str(&render_frame(kernel, vms));
    }
    out
}

fn render_frame(kernel: &Kernel, vms: &HashMap<ProcessId, Cpu>) -> String {
    let mut out = String::new();
    out.push_str("GPOS top — Kairos State monitor\n");
    out.push_str("PID   STATE   MODE     TICKS   SYSCALLS   CHAIN   TOPO        VIOL\n");

    let mut pids: Vec<_> = kernel.process_ids();
    pids.sort_unstable();

    for pid in pids {
        let proc = kernel.get_process(pid).ok();
        let state = proc.map(|p| p.state).unwrap_or(PhaseState::Phi0);
        let ticks = proc.map(|p| p.history.len()).unwrap_or(0);
        let viol = proc.map(|p| p.violations.len()).unwrap_or(0);
        let topo = proc
            .map(|p| {
                p.topology_events
                    .last()
                    .map(|e| match e.kind {
                        TopologyKind::Same => "same",
                        TopologyKind::Reflection => "reflection",
                    })
                    .unwrap_or("none")
            })
            .unwrap_or("none");
        let (syscalls, chain) = syscall_chain_status(kernel, pid);
        let state_col = if kernel.authority.is_anom_protected() {
            kernel.public_process_view(pid).render_short()
        } else {
            String::from(phi_label(state))
        };
        let mode = if vms.contains_key(&pid) {
            "vm"
        } else {
            process_mode(state)
        };
        out.push_str(&format!(
            "{pid:<5} {:<7} {:<8} {ticks:<7} {syscalls:<10} {chain:<7} {topo:<11} {viol}\n",
            state_col, mode,
        ));
    }

    out.push('\n');
    out.push_str(&format_phase_totals(
        "process phase totals:",
        process_phase_counts(kernel),
    ));
    if !vms.is_empty() {
        out.push_str(&format!("\nvm instances: {}\n", vms.len()));
    }
    out
}

#[cfg(feature = "std")]
fn syscall_chain_status(kernel: &Kernel, pid: ProcessId) -> (usize, &'static str) {
    let syscalls = kernel
        .statechain
        .chain(ObjectKind::Syscall, pid)
        .map(|c| c.events.len())
        .unwrap_or(0);
    let chain = kernel
        .statechain
        .chain(ObjectKind::Process, pid)
        .map(|c| c.verify().is_ok())
        .unwrap_or(true);
    let chain_word = if chain { "OK" } else { "BROKEN" };
    (syscalls, chain_word)
}

#[cfg(not(feature = "std"))]
fn syscall_chain_status(_kernel: &Kernel, _pid: ProcessId) -> (usize, &'static str) {
    (0, "N/A")
}
