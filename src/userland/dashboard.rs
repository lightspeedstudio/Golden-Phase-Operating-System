//! `dashboard` — one-screen operator overview.

use crate::kernel::Kernel;
use crate::platform::String;
#[cfg(feature = "std")]
use crate::userland::terminal_fmt::short_hex;
use crate::userland::terminal_fmt::EQUATION_LINE;
use crate::userland::terminal_metrics::{locked_object_count, reflection_count, violation_count};
#[cfg(feature = "std")]
use crate::userland::terminal_metrics::{memory_phase_counts, process_phase_counts};

#[cfg(feature = "std")]
use crate::kernel::statechain_watchdog::StatechainWatchdog;

pub fn render(kernel: &Kernel) -> String {
    let proc_n = kernel.process_ids().len();
    let mem_n = kernel.tag_report().len();
    let (inode_h, block_h) = kernel.fs.phase_histograms();
    let fs_n = inode_h.iter().sum::<usize>();
    let block_n: usize = block_h.iter().sum();
    let reflections = reflection_count(kernel);
    let violations = violation_count(kernel);

    let mut out = String::new();
    out.push_str("=== Golden Phase Operating System dashboard ===\n");
    out.push_str(&format!(
        "authority: {}\nvisibility: {}\n",
        crate::kernel::anom::authority_label(kernel.authority.authority),
        crate::kernel::anom::visibility_label(kernel.authority.visibility),
    ));
    if kernel.authority.is_anom_protected() {
        out.push_str("phase view: commitments\n");
    } else {
        out.push_str("phase view: visible\n");
    }
    out.push_str(&format!("equation: {EQUATION_LINE}\n"));
    let clock = kernel.get_clock();
    out.push_str(&format!(
        "clock: proper={} state={} sector={} x={:.3} dilation={:.3} skew={}\n",
        clock.proper_tick,
        clock.state_tick,
        clock.sector_id,
        clock.x,
        clock.dilation_factor,
        clock.skew(),
    ));

    #[cfg(feature = "std")]
    {
        let eq = short_hex(&kernel.statechain.equation_root());
        let kr = short_hex(&kernel.statechain.kernel_root());
        let dog = StatechainWatchdog::new();
        let audit = dog.audit(&kernel.statechain);
        let wd = if audit.tamper_detected { "ALERT" } else { "OK" };
        out.push_str(&format!("EquationRoot:    {eq}\n"));
        out.push_str(&format!("KernelStateRoot: {kr}\n"));
        if let Some(bsr) = boot_session_short(kernel) {
            out.push_str(&format!("BootSessionRoot: {bsr}\n"));
        }
        if let Some(ar) = kernel.anom.anom_root_short() {
            out.push_str(&format!("ANOM root:       {}\n", short_hex(&ar)));
        }
        if kernel.authority.is_anom_protected() {
            out.push_str("statechain: OK\n");
        }
        out.push_str(&format!(
            "processes: {proc_n}  memory pages: {mem_n}  fs inodes: {fs_n}  blocks: {block_n}\n"
        ));
        out.push_str(&format!(
            "proc Kairos States: Kairos0={} Kairos1={} Kairos2={} Kairos3={}\n",
            process_phase_counts(kernel)[0],
            process_phase_counts(kernel)[1],
            process_phase_counts(kernel)[2],
            process_phase_counts(kernel)[3],
        ));
        out.push_str(&format!(
            "mem Kairos States:  Kairos0={} Kairos1={} Kairos2={} Kairos3={}\n",
            memory_phase_counts(kernel)[0],
            memory_phase_counts(kernel)[1],
            memory_phase_counts(kernel)[2],
            memory_phase_counts(kernel)[3],
        ));
        out.push_str(&format!(
            "topology: {reflections} reflection(s)  violations: {violations}\n"
        ));
        out.push_str(&format!(
            "locked objects: {}\n",
            locked_object_count(kernel)
        ));
        out.push_str(&format!(
            "phase math: {} backend={} workers={}\n",
            "enabled",
            kernel.phase_math.backend.label(),
            kernel.phase_math.workers.active_worker_count,
        ));
        out.push_str(&format!("watchdog: {wd}\n"));
    }

    #[cfg(not(feature = "std"))]
    {
        out.push_str("EquationRoot:    (hosted build)\n");
        out.push_str("KernelStateRoot: (hosted build)\n");
        out.push_str(&format!(
            "processes: {proc_n}  memory pages: {mem_n}  fs inodes: {fs_n}  blocks: {block_n}\n"
        ));
        out.push_str(&format!(
            "topology: {reflections} reflection(s)  violations: {violations}\n"
        ));
        out.push_str(&format!(
            "locked objects: {}\n",
            locked_object_count(kernel)
        ));
    }

    out
}

#[cfg(feature = "std")]
fn boot_session_short(kernel: &Kernel) -> Option<String> {
    let bsr = kernel.authority.boot_session_root;
    if bsr == [0u8; 32] {
        None
    } else {
        Some(short_hex(&bsr))
    }
}
