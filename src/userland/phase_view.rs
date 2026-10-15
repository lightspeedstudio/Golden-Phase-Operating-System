//! `phase` — phase histograms across subsystems.

use crate::kernel::Kernel;
use crate::platform::String;
use crate::userland::terminal_fmt::format_phase_totals;
use crate::userland::terminal_metrics::{memory_phase_counts, process_phase_counts};

pub fn render(kernel: &Kernel) -> String {
    let (inode_h, block_h) = kernel.fs.phase_histograms();
    let mut out = String::new();
    out.push_str("=== Golden Phase Operating System Kairos State map ===\n\n");
    out.push_str(&format_phase_totals(
        "processes:",
        process_phase_counts(kernel),
    ));
    out.push('\n');
    out.push_str(&format_phase_totals(
        "memory pages:",
        memory_phase_counts(kernel),
    ));
    out.push('\n');
    out.push_str(&format_phase_totals("fs inodes:", inode_h));
    out.push('\n');
    out.push_str(&format_phase_totals("fs blocks:", block_h));
    out.push('\n');
    out.push_str(&format!(
        "phase math backend: {}\nphase math workers: {}\n",
        kernel.phase_math.backend.label(),
        kernel.phase_math.workers.active_worker_count
    ));
    out
}
