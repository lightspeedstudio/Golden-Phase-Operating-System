//! `fsj` — PhaseFS persistent journal research commands.

use crate::kernel::Kernel;
use crate::platform::String;

#[cfg(all(feature = "phasefs-journal", any(feature = "std", feature = "baremetal")))]
fn root_hex(root: &[u8; 32]) -> String {
    let mut s = String::new();
    for b in root {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

pub fn status(kernel: &Kernel) -> String {
    #[cfg(all(feature = "phasefs-journal", any(feature = "std", feature = "baremetal")))]
    {
        let r = kernel.journal_last;
        let b = kernel.journal_backend;
        let (start, p1, dev) = match b {
            crate::fs::journal::JournalBackend::Sd {
                start_lba,
                p1_sectors,
                device_sectors,
                ..
            } => (start_lba, p1_sectors, device_sectors),
            _ => (0, 0, 0),
        };
        format!(
            "fsj status\n\
             backend: {}\n\
             durable: {}\n\
             p1_start_lba: {}\n\
             p1_sectors: {}\n\
             device_sectors: {}\n\
             recover_class: {}\n\
             crash_armed: {}\n\
             last_replayed: {}\n\
             last_discarded: {}\n\
             generation: {}\n\
             txn: {}\n\
             fs_root: {}",
            b.label(),
            if b.durable() { "YES" } else { "NO" },
            start,
            p1,
            dev,
            r.class.as_str(),
            kernel.journal_crash.as_u32(),
            r.replayed as u8,
            r.discarded as u8,
            r.generation,
            r.txn_id,
            root_hex(&r.fs_root),
        )
    }
    #[cfg(not(all(feature = "phasefs-journal", any(feature = "std", feature = "baremetal"))))]
    {
        let _ = kernel;
        "fsj unavailable (phasefs-journal feature not enabled)".into()
    }
}

#[cfg(all(feature = "phasefs-journal", feature = "std"))]
pub fn recover(kernel: &mut Kernel) -> String {
    match kernel.fs_journal.recover(&mut kernel.fs) {
        Ok(r) => {
            kernel.journal_last = r;
            kernel.journal_post_recover();
            format!(
                "fsj recover: class={} replayed={} discarded={} gen={}",
                r.class.as_str(),
                r.replayed as u8,
                r.discarded as u8,
                r.generation
            )
        }
        Err(e) => format!("fsj recover: {e}"),
    }
}

#[cfg(all(
    feature = "phasefs-journal",
    feature = "baremetal",
    target_arch = "arm"
))]
pub fn recover(kernel: &mut Kernel) -> String {
    match crate::board::zynq7000::journal_sd::recover_now(kernel) {
        Ok(r) => format!(
            "fsj recover: class={} replayed={} discarded={} gen={}",
            r.class.as_str(),
            r.replayed as u8,
            r.discarded as u8,
            r.generation
        ),
        Err(e) => format!("fsj recover: {e}"),
    }
}

#[cfg(all(
    feature = "phasefs-journal",
    not(feature = "std"),
    not(all(feature = "baremetal", target_arch = "arm"))
))]
pub fn recover(kernel: &mut Kernel) -> String {
    let _ = kernel;
    "fsj recover: no backend".into()
}

#[cfg(not(feature = "phasefs-journal"))]
pub fn recover(kernel: &mut Kernel) -> String {
    let _ = kernel;
    "fsj unavailable".into()
}

#[cfg(all(feature = "phasefs-journal", any(feature = "std", feature = "baremetal")))]
pub fn check(kernel: &Kernel) -> String {
    match crate::fs::journal::check_invariants(&kernel.fs) {
        Ok(()) => {
            if kernel.journal_backend.durable() {
                "fsj check: PASS\nbackend: SD_PARTITION_1\nDURABLE: YES".into()
            } else {
                format!(
                    "fsj check: invariants OK ({}; not a durability PASS)\nDURABLE: NO",
                    kernel.journal_backend.label()
                )
            }
        }
        Err(e) => format!("fsj check: FAIL {e}"),
    }
}

#[cfg(not(all(feature = "phasefs-journal", any(feature = "std", feature = "baremetal"))))]
pub fn check(kernel: &Kernel) -> String {
    let _ = kernel;
    "fsj unavailable".into()
}

#[cfg(all(feature = "phasefs-journal", any(feature = "std", feature = "baremetal")))]
pub fn crash(kernel: &mut Kernel, n: u32) -> String {
    match crate::fs::journal::CrashPoint::from_u32(n) {
        Some(p) => {
            kernel.journal_crash = p;
            if n == 0 {
                "fsj crash off".into()
            } else {
                format!("fsj crash armed {}", n)
            }
        }
        None => "fsj crash: id 0..=11 or off".into(),
    }
}

#[cfg(not(all(feature = "phasefs-journal", any(feature = "std", feature = "baremetal"))))]
pub fn crash(kernel: &mut Kernel, n: u32) -> String {
    let _ = (kernel, n);
    "fsj unavailable".into()
}

#[cfg(all(feature = "phasefs-journal", feature = "std"))]
pub fn format_cmd(kernel: &mut Kernel, confirm: bool) -> String {
    if !confirm {
        return "fsj format refused: type CONFIRM\nbackend: RAM_FALLBACK (host)".into();
    }
    match crate::fs::journal::PhaseFsJournal::format_ram(&kernel.fs) {
        Ok(j) => {
            kernel.fs_journal = j;
            kernel.journal_backend = crate::fs::journal::JournalBackend::RamFallback;
            "fsj format: ram device".into()
        }
        Err(e) => format!("fsj format: {e}"),
    }
}

#[cfg(all(
    feature = "phasefs-journal",
    feature = "baremetal",
    target_arch = "arm"
))]
pub fn format_cmd(kernel: &mut Kernel, confirm: bool) -> String {
    let probe = crate::board::zynq7000::journal_sd::probe_text(kernel);
    if !confirm {
        return format!("fsj format refused: type CONFIRM\n{probe}");
    }
    match crate::board::zynq7000::journal_sd::format_region(kernel) {
        Ok(()) => format!("fsj format: partition 1\n{probe}"),
        Err(e) => format!("fsj format: {e}\n{probe}"),
    }
}

#[cfg(all(
    feature = "phasefs-journal",
    not(feature = "std"),
    not(all(feature = "baremetal", target_arch = "arm"))
))]
pub fn format_cmd(kernel: &mut Kernel, confirm: bool) -> String {
    let _ = (kernel, confirm);
    "fsj format: no backend".into()
}

#[cfg(not(feature = "phasefs-journal"))]
pub fn format_cmd(kernel: &mut Kernel, confirm: bool) -> String {
    let _ = (kernel, confirm);
    "fsj unavailable".into()
}

#[cfg(all(feature = "phasefs-journal", feature = "std"))]
pub fn p6_arm(kernel: &mut Kernel, confirm: bool) -> String {
    if !confirm {
        return "fsj p6 refused: type CONFIRM".into();
    }
    let run = kernel.fs_journal.next_txn_id();
    match kernel.fs_journal.arm_p6(run) {
        Ok(id) => {
            kernel.journal_crash = crate::fs::journal::CrashPoint::AfterCommitFlush;
            let extra = kernel.fs_journal.p6_lines.last().cloned().unwrap_or_default();
            format!("{extra}\nfsj p6 armed run={id} (next write uses crash 5)")
        }
        Err(e) => format!("fsj p6: {e}"),
    }
}

#[cfg(all(
    feature = "phasefs-journal",
    feature = "baremetal",
    target_arch = "arm"
))]
pub fn p6_arm(kernel: &mut Kernel, confirm: bool) -> String {
    if !confirm {
        return "fsj p6 refused: type CONFIRM".into();
    }
    match crate::board::zynq7000::journal_sd::arm_p6(kernel) {
        Ok(id) => format!("P6_ARMED run={id}\nfsj p6 armed run={id} (next write uses crash 5)"),
        Err(e) => format!("fsj p6: {e}"),
    }
}

#[cfg(all(
    feature = "phasefs-journal",
    not(feature = "std"),
    not(all(feature = "baremetal", target_arch = "arm"))
))]
pub fn p6_arm(kernel: &mut Kernel, confirm: bool) -> String {
    let _ = (kernel, confirm);
    "fsj p6: no backend".into()
}

#[cfg(not(feature = "phasefs-journal"))]
pub fn p6_arm(kernel: &mut Kernel, confirm: bool) -> String {
    let _ = (kernel, confirm);
    "fsj unavailable".into()
}

#[cfg(all(
    feature = "phasefs-journal",
    feature = "baremetal",
    target_arch = "arm"
))]
pub fn p7_run(kernel: &mut Kernel, run_id: u64, token_ok: bool) -> String {
    if !token_ok {
        return "P7_FAIL wrong token".into();
    }
    match crate::board::zynq7000::journal_sd::p7_execute(
        kernel,
        run_id,
        crate::fs::journal::P7_TOKEN,
    ) {
        Ok(()) => "P7_FAIL did not reset".into(),
        Err(e) => format!("P7_FAIL {}", e.as_str()),
    }
}

#[cfg(all(feature = "phasefs-journal", feature = "std"))]
pub fn p7_run(kernel: &mut Kernel, run_id: u64, token_ok: bool) -> String {
    let _ = kernel;
    if !token_ok {
        return "P7_FAIL wrong token".into();
    }
    format!("P7_FAIL host cannot SLCR run={run_id}")
}

#[cfg(all(
    feature = "phasefs-journal",
    not(feature = "std"),
    not(all(feature = "baremetal", target_arch = "arm"))
))]
pub fn p7_run(kernel: &mut Kernel, run_id: u64, token_ok: bool) -> String {
    let _ = (kernel, run_id, token_ok);
    "P7_FAIL no backend".into()
}

#[cfg(not(feature = "phasefs-journal"))]
pub fn p7_run(kernel: &mut Kernel, run_id: u64, token_ok: bool) -> String {
    let _ = (kernel, run_id, token_ok);
    "fsj unavailable".into()
}

#[cfg(all(
    feature = "phasefs-journal",
    feature = "baremetal",
    target_arch = "arm"
))]
pub fn p8_run(kernel: &mut Kernel, run_id: u64, token_ok: bool) -> String {
    if !token_ok {
        return "P8_FAIL wrong token".into();
    }
    match crate::board::zynq7000::journal_sd::p8_execute(kernel, run_id) {
        Ok(()) => "P8_FAIL did not reset".into(),
        Err(e) => format!("P8_FAIL {e}"),
    }
}

#[cfg(all(feature = "phasefs-journal", feature = "std"))]
pub fn p8_run(kernel: &mut Kernel, run_id: u64, token_ok: bool) -> String {
    if !token_ok {
        return "P8_FAIL wrong token".into();
    }
    match kernel.fs_journal.p8_wrap_commit(&mut kernel.fs, run_id) {
        Ok((h0, h1, u)) => format!("P8_WRAPPED host head0={h0} head={h1} used={u} (no SLCR)"),
        Err(e) => format!("P8_FAIL {e}"),
    }
}

#[cfg(all(
    feature = "phasefs-journal",
    not(feature = "std"),
    not(all(feature = "baremetal", target_arch = "arm"))
))]
pub fn p8_run(kernel: &mut Kernel, run_id: u64, token_ok: bool) -> String {
    let _ = (kernel, run_id, token_ok);
    "P8_FAIL no backend".into()
}

#[cfg(not(feature = "phasefs-journal"))]
pub fn p8_run(kernel: &mut Kernel, run_id: u64, token_ok: bool) -> String {
    let _ = (kernel, run_id, token_ok);
    "fsj unavailable".into()
}

#[cfg(all(
    feature = "phasefs-journal",
    feature = "baremetal",
    target_arch = "arm"
))]
pub fn p10_run(kernel: &mut Kernel, run_id: u64, token_ok: bool) -> String {
    if !token_ok {
        return "P10_FAIL wrong token".into();
    }
    match crate::board::zynq7000::journal_sd::p10_execute(
        kernel,
        run_id,
        crate::fs::journal::P10_TOKEN,
    ) {
        Ok(()) => "P10_FAIL did not halt".into(),
        Err(e) => format!("P10_FAIL {}", e.as_str()),
    }
}

#[cfg(all(feature = "phasefs-journal", feature = "std"))]
pub fn p10_run(kernel: &mut Kernel, run_id: u64, token_ok: bool) -> String {
    if !token_ok {
        return "P10_FAIL wrong token".into();
    }
    match kernel
        .fs_journal
        .p10_stage_uncommitted(&mut kernel.fs, run_id, crate::fs::journal::P10_TOKEN)
    {
        Ok(()) => "P10_UNCOMMITTED_WRITTEN host (no physical power cut)".into(),
        Err(e) => format!("P10_FAIL {}", e.as_str()),
    }
}

#[cfg(all(
    feature = "phasefs-journal",
    not(feature = "std"),
    not(all(feature = "baremetal", target_arch = "arm"))
))]
pub fn p10_run(kernel: &mut Kernel, run_id: u64, token_ok: bool) -> String {
    let _ = (kernel, run_id, token_ok);
    "P10_FAIL no backend".into()
}

#[cfg(not(feature = "phasefs-journal"))]
pub fn p10_run(kernel: &mut Kernel, run_id: u64, token_ok: bool) -> String {
    let _ = (kernel, run_id, token_ok);
    "fsj unavailable".into()
}

#[cfg(all(
    feature = "phasefs-journal",
    feature = "baremetal",
    target_arch = "arm"
))]
pub fn p11_run(kernel: &mut Kernel, run_id: u64, token_ok: bool) -> String {
    if !token_ok {
        return "P11_FAIL wrong token".into();
    }
    match crate::board::zynq7000::journal_sd::p11_execute(
        kernel,
        run_id,
        crate::fs::journal::P11_TOKEN,
    ) {
        Ok(()) => "P11_FAIL did not halt".into(),
        Err(e) => format!("P11_FAIL {}", e.as_str()),
    }
}

#[cfg(all(feature = "phasefs-journal", feature = "std"))]
pub fn p11_run(kernel: &mut Kernel, run_id: u64, token_ok: bool) -> String {
    if !token_ok {
        return "P11_FAIL wrong token".into();
    }
    match kernel
        .fs_journal
        .p11_commit(&mut kernel.fs, run_id, crate::fs::journal::P11_TOKEN)
    {
        Ok(txn) => format!("P11_COMMITTED host txn={txn} (no physical power cut)"),
        Err(e) => format!("P11_FAIL {}", e.as_str()),
    }
}

#[cfg(all(
    feature = "phasefs-journal",
    not(feature = "std"),
    not(all(feature = "baremetal", target_arch = "arm"))
))]
pub fn p11_run(kernel: &mut Kernel, run_id: u64, token_ok: bool) -> String {
    let _ = (kernel, run_id, token_ok);
    "P11_FAIL no backend".into()
}

#[cfg(not(feature = "phasefs-journal"))]
pub fn p11_run(kernel: &mut Kernel, run_id: u64, token_ok: bool) -> String {
    let _ = (kernel, run_id, token_ok);
    "fsj unavailable".into()
}

#[cfg(all(
    feature = "phasefs-journal",
    feature = "baremetal",
    target_arch = "arm"
))]
pub fn p1_reset(kernel: &mut Kernel) -> String {
    let _ = kernel;
    crate::board::zynq7000::journal_sd::p1_reset()
}

#[cfg(all(feature = "phasefs-journal", feature = "std"))]
pub fn p1_reset(kernel: &mut Kernel) -> String {
    let _ = kernel;
    "P1_FAIL host cannot SLCR".into()
}

#[cfg(all(
    feature = "phasefs-journal",
    not(feature = "std"),
    not(all(feature = "baremetal", target_arch = "arm"))
))]
pub fn p1_reset(kernel: &mut Kernel) -> String {
    let _ = kernel;
    "P1_FAIL no backend".into()
}

#[cfg(not(feature = "phasefs-journal"))]
pub fn p1_reset(kernel: &mut Kernel) -> String {
    let _ = kernel;
    "fsj unavailable".into()
}
