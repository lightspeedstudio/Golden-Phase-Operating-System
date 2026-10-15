//! SD partition-1 window for the PhaseFS journal (`feature = "phasefs-journal"`).
//!
//! Gold Utility includes this feature, so this module is compiled into the
//! gold board image (`zynq-a9,utility`). It is the SDHCI window onto
//! partition 1, not a separate research image.
//! Does not write the MBR. Never treats partition 0 as the journal.

use crate::fs::blockdev::{
    BlockDevError, BlockDevice, BlockIoDiag, IO_CLASS_CONTROLLER, IO_CLASS_CRC, IO_CLASS_INHIBIT,
    IO_CLASS_MEDIA, IO_CLASS_NOTREADY, IO_CLASS_TIMEOUT, IO_OP_FLUSH, IO_OP_READ, IO_OP_WRITE,
};
use crate::fs::journal::{
    self, validate_journal_window, JournalBackend, JournalError, MbrSlot, PhaseFsJournal, RedoOp,
    P6_PLAN_SECTORS, REGION_SECTORS,
};
use crate::kernel::Kernel;
use crate::platform::String;

use super::fat::{self, MbrPartition};
use super::sd::{SdCard, SdError};
use super::uart;

pub struct SdWindow {
    sd: SdCard,
    base: u32,
    device_sectors: u32,
}

impl SdWindow {
    fn from_validated(sd: SdCard, p1: MbrSlot) -> Result<Self, JournalError> {
        let device_sectors = sd.device_sectors;
        if device_sectors == 0 {
            return Err(JournalError::CapacityUnknown);
        }
        let end = p1
            .start_lba
            .checked_add(p1.sectors)
            .ok_or(JournalError::PartitionOutOfRange)?;
        if end > device_sectors {
            return Err(JournalError::PartitionOutOfRange);
        }
        let _region_end = p1
            .start_lba
            .checked_add(REGION_SECTORS.saturating_add(P6_PLAN_SECTORS))
            .ok_or(JournalError::PartitionOutOfRange)?;
        Ok(Self {
            sd,
            base: p1.start_lba,
            device_sectors,
        })
    }

    fn abs(&self, lba: u32) -> Result<u32, BlockDevError> {
        let abs = crate::fs::journal::abs_journal_lba(self.base, lba)
            .map_err(|_| BlockDevError::OutOfRange)?;
        if abs >= self.device_sectors {
            return Err(BlockDevError::OutOfRange);
        }
        Ok(abs)
    }

    fn map_sd(&self, op: u8, abs: u32, e: SdError) -> BlockDevError {
        let (head, tail, generation) = journal::journal_io_ctx();
        let (class, present, nistr, eistr) = match e {
            SdError::Detail(d) => (d.class, d.present, d.nistr, d.eistr),
            SdError::Timeout => (IO_CLASS_TIMEOUT, 0, 0, 0),
            SdError::NotReady => (IO_CLASS_NOTREADY, 0, 0, 0),
            SdError::NoCard => (IO_CLASS_MEDIA, 0, 0, 0),
            SdError::CmdError | SdError::NotSdV2 => (IO_CLASS_CONTROLLER, 0, 0, 0),
        };
        let class = match class {
            crate::board::zynq7000::sd::SD_CLASS_CRC => IO_CLASS_CRC,
            crate::board::zynq7000::sd::SD_CLASS_CONTROLLER => IO_CLASS_CONTROLLER,
            crate::board::zynq7000::sd::SD_CLASS_MEDIA => IO_CLASS_MEDIA,
            crate::board::zynq7000::sd::SD_CLASS_INHIBIT => IO_CLASS_INHIBIT,
            crate::board::zynq7000::sd::SD_CLASS_NOTREADY => IO_CLASS_NOTREADY,
            _ => IO_CLASS_TIMEOUT,
        };
        BlockDevError::Detail(BlockIoDiag {
            op,
            lba: abs,
            count: 1,
            present,
            nistr,
            eistr,
            class,
            reinit: super::sd::last_reinit(),
            head,
            tail,
            generation,
        })
    }
}

impl BlockDevice for SdWindow {
    fn read(&mut self, lba: u32, buf: &mut [u8; 512]) -> Result<(), BlockDevError> {
        let abs = self.abs(lba)?;
        self.sd
            .read_block(abs, buf)
            .map_err(|e| self.map_sd(IO_OP_READ, abs, e))
    }

    fn write(&mut self, lba: u32, buf: &[u8; 512]) -> Result<(), BlockDevError> {
        let abs = self.abs(lba)?;
        self.sd
            .write_block(abs, buf)
            .map_err(|e| self.map_sd(IO_OP_WRITE, abs, e))?;
        let _ = self.sd.wait_tran();
        Ok(())
    }

    fn flush(&mut self) -> Result<(), BlockDevError> {
        self.sd
            .wait_ready()
            .map_err(|e| self.map_sd(IO_OP_FLUSH, 0, e))
    }
}

fn slots_from_mbr(parts: [Option<MbrPartition>; 4]) -> [Option<MbrSlot>; 4] {
    let mut out = [None; 4];
    for (i, p) in parts.iter().enumerate() {
        if let Some(p) = p {
            out[i] = Some(MbrSlot {
                start_lba: p.start_lba,
                sectors: p.sectors,
                ptype: p.ptype,
            });
        }
    }
    out
}

struct Opened {
    win: SdWindow,
    start_lba: u32,
    p1_sectors: u32,
    device_sectors: u32,
    p0_start: u32,
    p0_sectors: u32,
}

fn csd_unknown_line(csd: [u32; 4]) -> String {
    alloc::format!(
        "fsj: CSD capacity unknown RESP0={:#010x} RESP1={:#010x} RESP2={:#010x} RESP3={:#010x}\n",
        csd[0],
        csd[1],
        csd[2],
        csd[3]
    )
}

fn p1_line(start: u32, p1_len: u32, dev: u32) -> String {
    match start.checked_add(p1_len) {
        Some(p1_end) => alloc::format!(
            "fsj: partition 1 start_lba={start} sectors={p1_len} p1_end={p1_end} device_sectors={dev}\n"
        ),
        None => alloc::format!(
            "fsj: partition 1 start_lba={start} sectors={p1_len} p1_end=OVERFLOW device_sectors={dev}\n"
        ),
    }
}

fn open_part1() -> Result<Opened, JournalError> {
    let mut sd = SdCard::init().map_err(|e| match e {
        SdError::NoCard => JournalError::MediaUnavailable,
        SdError::Timeout
        | SdError::CmdError
        | SdError::NotReady
        | SdError::NotSdV2
        | SdError::Detail(_) => JournalError::MediaUnavailable,
    })?;
    let device_sectors = sd.device_sectors;
    if device_sectors == 0 {
        uart::write_str(&csd_unknown_line(sd.csd_resp));
        return Err(JournalError::CapacityUnknown);
    }
    let parts = fat::read_mbr_partitions(&mut sd).map_err(|_| JournalError::Io)?;
    let slots = slots_from_mbr(parts);
    let (p0_start, p0_sectors) = match slots[0] {
        Some(s) => (s.start_lba, s.sectors),
        None => (0, 0),
    };
    let p1 = validate_journal_window(slots, device_sectors)?;
    let start_lba = p1.start_lba;
    let p1_sectors = p1.sectors;
    let win = SdWindow::from_validated(sd, p1)?;
    Ok(Opened {
        win,
        start_lba,
        p1_sectors,
        device_sectors,
        p0_start,
        p0_sectors,
    })
}

fn set_backend(kernel: &mut Kernel, opened: &Opened, formatted: bool) {
    kernel.journal_backend = JournalBackend::Sd {
        start_lba: opened.start_lba,
        p1_sectors: opened.p1_sectors,
        device_sectors: opened.device_sectors,
        formatted,
    };
}

pub fn boot_recover(kernel: &mut Kernel) -> String {
    match open_part1() {
        Err(JournalError::NoPartition) => {
            kernel.journal_backend = JournalBackend::Absent;
            "fsj: no partition 1 (RAM PhaseFs only)\n".into()
        }
        Err(e) => {
            kernel.journal_backend = JournalBackend::Absent;
            alloc::format!("fsj: sd error {e}\n")
        }
        Ok(opened) => {
            let start = opened.start_lba;
            let p1_len = opened.p1_sectors;
            let dev = opened.device_sectors;
            let head = p1_line(start, p1_len, dev);
            match PhaseFsJournal::attach(opened.win) {
                Err(JournalError::Unformatted) => {
                    kernel.journal_backend = JournalBackend::Sd {
                        start_lba: start,
                        p1_sectors: p1_len,
                        device_sectors: dev,
                        formatted: false,
                    };
                    alloc::format!(
                        "{head}fsj: partition 1 unformatted (use fsj format CONFIRM on sacrificial card)\n"
                    )
                }
                Err(e) => {
                    kernel.journal_backend = JournalBackend::Sd {
                        start_lba: start,
                        p1_sectors: p1_len,
                        device_sectors: dev,
                        formatted: false,
                    };
                    alloc::format!("{head}fsj: attach error {e}\n")
                }
                Ok(mut j) => {
                    kernel.journal_backend = JournalBackend::Sd {
                        start_lba: start,
                        p1_sectors: p1_len,
                        device_sectors: dev,
                        formatted: true,
                    };
                    j.set_crash(kernel.journal_crash);
                    match j.recover(&mut kernel.fs) {
                        Ok(report) => {
                            kernel.journal_last = report;
                            kernel.journal_post_recover();
                            alloc::format!(
                                "{head}fsj: recovered class={} replayed={} discarded={} gen={} txn={}\n",
                                report.class.as_str(),
                                report.replayed as u8,
                                report.discarded as u8,
                                report.generation,
                                report.txn_id
                            )
                        }
                        Err(JournalError::InjectedCrash) => {
                            physical_crash_halt(100);
                        }
                        Err(e) => alloc::format!("{head}fsj: recover error {e}\n"),
                    }
                }
            }
        }
    }
}

pub fn emit_p6_line(line: &str) {
    uart::write_str(line);
    uart::write_str("\n");
    uart::drain_tx_bounded();
}

fn physical_crash_halt(point: u32) -> ! {
    uart::write_str(&alloc::format!(
        "fsj CRASH point={point} — hardware reset (not in-process recover)\n"
    ));
    uart::drain_tx_bounded();
    unsafe {
        core::ptr::write_volatile(0xF800_0008 as *mut u32, 0x0000_DF0D);
        core::ptr::write_volatile(0xF800_0200 as *mut u32, 1);
    }
    loop {
        core::hint::spin_loop();
    }
}

pub fn run_ops(kernel: &mut Kernel, ops: &[RedoOp]) -> Result<(), JournalError> {
    let opened = open_part1()?;
    set_backend(kernel, &opened, true);
    let mut j = PhaseFsJournal::attach(opened.win)?;
    let crash = kernel.journal_crash;
    j.set_crash(crash);
    kernel.journal_crash = journal::CrashPoint::None;
    match j.run_transaction(&mut kernel.fs, ops) {
        Ok(_) => Ok(()),
        Err(JournalError::InjectedCrash) => {
            let _ = j.dev.flush();
            physical_crash_halt(crash.as_u32());
        }
        Err(e) => Err(e),
    }
}

pub fn format_region(kernel: &mut Kernel) -> Result<(), JournalError> {
    let opened = open_part1()?;
    let start = opened.start_lba;
    let p1_len = opened.p1_sectors;
    let device_sectors = opened.device_sectors;
    let j = PhaseFsJournal::format(opened.win, &kernel.fs)?;
    kernel.journal_last = crate::fs::journal::RecoverReport {
        class: crate::fs::journal::RecoverClass::Clean,
        replayed: false,
        discarded: false,
        generation: j.generation(),
        fs_root: j.fs_root(),
        txn_id: 0,
    };
    let _ = j;
    kernel.journal_backend = JournalBackend::Sd {
        start_lba: start,
        p1_sectors: p1_len,
        device_sectors,
        formatted: true,
    };
    Ok(())
}

pub fn recover_now(kernel: &mut Kernel) -> Result<journal::RecoverReport, JournalError> {
    let opened = match open_part1() {
        Ok(o) => o,
        Err(e) => {
            if matches!(e, JournalError::MediaUnavailable) {
                kernel.journal_backend = JournalBackend::Absent;
            }
            return Err(e);
        }
    };
    set_backend(kernel, &opened, true);
    let mut j = PhaseFsJournal::attach(opened.win)?;
    match j.recover(&mut kernel.fs) {
        Ok(report) => {
            kernel.journal_last = report;
            kernel.journal_post_recover();
            Ok(report)
        }
        Err(JournalError::InjectedCrash) => {
            physical_crash_halt(100);
        }
        Err(e) => Err(e),
    }
}

pub fn arm_p6(kernel: &mut Kernel) -> Result<u64, JournalError> {
    let opened = open_part1()?;
    set_backend(kernel, &opened, true);
    let mut j = PhaseFsJournal::attach(opened.win)?;
    let run = j.next_txn_id();
    j.arm_p6(run)?;
    kernel.journal_crash = journal::CrashPoint::AfterCommitFlush;
    Ok(run)
}

pub fn p7_execute(kernel: &mut Kernel, run_id: u64, token: &str) -> Result<(), journal::P7Fail> {
    use crate::fs::journal::P7Window;
    let opened = open_part1().map_err(|e| match e {
        JournalError::NoPartition | JournalError::BadPartition => journal::P7Fail::MissingPartition,
        JournalError::CapacityUnknown => journal::P7Fail::CapacityUnknown,
        JournalError::MediaUnavailable => journal::P7Fail::Io,
        JournalError::PartitionOverlap => journal::P7Fail::OverlapP0,
        _ => journal::P7Fail::Io,
    })?;
    set_backend(kernel, &opened, true);
    let window = P7Window {
        start_lba: opened.start_lba,
        p1_sectors: opened.p1_sectors,
        device_sectors: opened.device_sectors,
        p0_start: opened.p0_start,
        p0_sectors: opened.p0_sectors,
    };
    let mut j = PhaseFsJournal::attach(opened.win).map_err(|_| journal::P7Fail::Io)?;
    j.p7_inject(run_id, token, window)?;
    uart::write_str("P7_RESETTING\n");
    uart::drain_tx_bounded();
    unsafe {
        core::arch::asm!("dsb", options(nostack, preserves_flags));
    }
    physical_crash_halt(7);
}

pub fn p1_reset() -> ! {
    uart::write_str("P1_RESETTING\n");
    uart::drain_tx_bounded();
    unsafe {
        core::arch::asm!("dsb", options(nostack, preserves_flags));
    }
    physical_crash_halt(1);
}

pub fn p8_execute(kernel: &mut Kernel, run_id: u64) -> Result<(), JournalError> {
    let opened = open_part1()?;
    set_backend(kernel, &opened, true);
    let mut j = PhaseFsJournal::attach(opened.win)?;
    j.p8_wrap_commit(&mut kernel.fs, run_id)?;
    uart::write_str("P8_RESETTING\n");
    uart::drain_tx_bounded();
    unsafe {
        core::arch::asm!("dsb", options(nostack, preserves_flags));
    }
    physical_crash_halt(8);
}

pub fn p10_execute(kernel: &mut Kernel, run_id: u64, token: &str) -> Result<(), journal::P10Fail> {
    let opened = open_part1().map_err(|_| journal::P10Fail::Io)?;
    set_backend(kernel, &opened, true);
    let mut j = PhaseFsJournal::attach(opened.win).map_err(|_| journal::P10Fail::Io)?;
    j.p10_stage_uncommitted(&mut kernel.fs, run_id, token)?;
    uart::write_str(journal::P10_READY);
    uart::write_str("\n");
    uart::drain_tx_bounded();
    unsafe {
        core::arch::asm!("dsb", options(nostack, preserves_flags));
    }
    loop {
        unsafe {
            core::arch::asm!("wfe", options(nomem, nostack, preserves_flags));
        }
    }
}

pub fn p11_execute(kernel: &mut Kernel, run_id: u64, token: &str) -> Result<(), journal::P11Fail> {
    let opened = open_part1().map_err(|_| journal::P11Fail::Io)?;
    set_backend(kernel, &opened, true);
    let mut j = PhaseFsJournal::attach(opened.win).map_err(|_| journal::P11Fail::Io)?;
    j.p11_commit(&mut kernel.fs, run_id, token)?;
    uart::write_str(journal::P11_READY);
    uart::write_str("\n");
    uart::drain_tx_bounded();
    unsafe {
        core::arch::asm!("dsb", options(nostack, preserves_flags));
    }
    loop {
        unsafe {
            core::arch::asm!("wfe", options(nomem, nostack, preserves_flags));
        }
    }
}

pub fn probe_text(kernel: &Kernel) -> String {
    match kernel.journal_backend {
        JournalBackend::Sd {
            start_lba,
            p1_sectors,
            device_sectors,
            ..
        } => alloc::format!(
            "partition 1 start_lba={start_lba} sectors={p1_sectors} p1_end={} device_sectors={device_sectors} (min {REGION_SECTORS})",
            start_lba.checked_add(p1_sectors).unwrap_or(0)
        ),
        _ => match open_part1() {
            Ok(o) => alloc::format!(
                "partition 1 start_lba={} sectors={} p1_end={} device_sectors={} (min {REGION_SECTORS})",
                o.start_lba,
                o.p1_sectors,
                o.start_lba.checked_add(o.p1_sectors).unwrap_or(0),
                o.device_sectors
            ),
            Err(e) => alloc::format!("partition 1 unavailable: {e}"),
        },
    }
}
