//! Kernel metric collectors for terminal views.

use crate::core::gphase_core::{PhaseState, TopologyKind};
use crate::kernel::Kernel;
use crate::userland::terminal_fmt::bump_phase;

pub fn process_phase_counts(kernel: &Kernel) -> [usize; 4] {
    let mut counts = [0usize; 4];
    for pid in kernel.process_ids() {
        if let Ok(proc) = kernel.get_process(pid) {
            bump_phase(&mut counts, proc.state);
        }
    }
    counts
}

pub fn memory_phase_counts(kernel: &Kernel) -> [usize; 4] {
    let mut counts = [0usize; 4];
    for tag in kernel.tag_report().values() {
        bump_phase(&mut counts, *tag);
    }
    counts
}

pub fn reflection_count(kernel: &Kernel) -> usize {
    kernel
        .topology_report()
        .iter()
        .filter(|e| e.kind == TopologyKind::Reflection)
        .count()
}

pub fn violation_count(kernel: &Kernel) -> usize {
    kernel
        .process_ids()
        .iter()
        .filter_map(|pid| kernel.get_process(*pid).ok())
        .map(|p| p.violations.len())
        .sum()
}

pub fn stuck_phi2(kernel: &Kernel) -> usize {
    kernel
        .process_ids()
        .iter()
        .filter(|pid| {
            kernel
                .get_process(**pid)
                .map(|p| p.state == PhaseState::Phi2)
                .unwrap_or(false)
        })
        .count()
}

pub fn stuck_phi3(kernel: &Kernel) -> usize {
    let proc_stuck = kernel
        .process_ids()
        .iter()
        .filter(|pid| {
            kernel
                .get_process(**pid)
                .map(|p| p.state == PhaseState::Phi3)
                .unwrap_or(false)
        })
        .count();
    let page_stuck = kernel
        .tag_report()
        .values()
        .filter(|t| **t == PhaseState::Phi3)
        .count();
    proc_stuck + page_stuck
}

pub fn locked_object_count(kernel: &Kernel) -> usize {
    let page_locked = kernel
        .tag_report()
        .values()
        .filter(|t| **t == PhaseState::Phi3)
        .count();
    page_locked + kernel.fs.locked_object_count()
}
