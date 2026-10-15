//! T5: Cortex-A9 CPU1 bring-up on Zynq-7000.
//!
//! Sequence (CPU0): fill identity L1 → write mailbox + `0xFFFFFFF0` while
//! D-cache is still off → SCU enable → ACTLR.SMP → MMU/caches on CPU0 →
//! DSB + SEV → poll `CPU1_MAILBOX` for MPIDR.
//!
//! Deviations from the bring-up sketch, required by this tree / ARM ARM:
//! - `core::arch::arm::{__dsb,__sev,__wfe}` are not in this crate; use `asm!`.
//! - CPU1 cannot MRC CPU0's TTBR0. The physical table address is in the mailbox.
//! - Short-descriptor S (shareable) is bit 16, not bit 14 (TEX[2]).
//! - CPU1 stack and L1 table are DDR linker reservations, not OCM `0x00034000`.
//!   This kernel is linked at DDR `0x00100000`; OCM after FSBL is not ours.
//! - `0xFFFFFFF0` is still the BootROM jump register (UG585).
//!
//! T6: after ONLINE, CPU1 runs the SPSC pop/WFE loop. No tensor worker (T7).

use core::sync::atomic::{AtomicU32, Ordering};

use crate::kernel::spsc_mailbox::{
    Cpu1Channels, MailboxTask, OPCODE_PING, OPCODE_PONG, OPCODE_UNK,
};

use super::memory_map::DDR_HIGH;
use super::uart;

/// SCU base = same MPCore periphbase as the global timer at `0xF8F00200`.
const SCU_BASE: u32 = 0xF8F0_0000;
const SCU_CTRL: u32 = SCU_BASE;
const SCU_INV_ALL: u32 = SCU_BASE + 0x0C;
const SCU_EN: u32 = 1;

/// Zynq BootROM CPU1 vector (UG585 APU). High OCM.
const CPU1_BOOT_VECTOR: u32 = 0xFFFF_FFF0;

/// SLCR keys match `gem.rs` / `usb.rs`. `A9_CPU_RST_CTRL` is UG585 §B.28.
const SLCR_LOCK: u32 = 0xF800_0004;
const SLCR_UNLOCK: u32 = 0xF800_0008;
const SLCR_LOCK_KEY: u32 = 0x0000_767B;
const SLCR_UNLOCK_KEY: u32 = 0x0000_DF0D;
const A9_CPU_RST_CTRL: u32 = 0xF800_0244;
const A9_CPU1_RST: u32 = 1 << 1;
const A9_CPU1_CLKSTOP: u32 = 1 << 5;

/// Section descriptor [1:0] = 0b10.
const DESC_SECTION: u32 = 0b10;
const DESC_B: u32 = 1 << 2;
const DESC_C: u32 = 1 << 3;
const DESC_XN: u32 = 1 << 4;
const DESC_AP_FULL: u32 = 0b11 << 10;
const DESC_TEX0: u32 = 1 << 12;
/// Shareable. ARM ARM short-descriptor S is bit 16, not 14.
const DESC_S: u32 = 1 << 16;

/// Normal Inner/Outer WBWA, Inner Shareable: TEX,C,B = 001,1,1 and S=1.
const SECTION_NORMAL_SHAREABLE: u32 =
    DESC_SECTION | DESC_B | DESC_C | DESC_AP_FULL | DESC_TEX0 | DESC_S;
/// Shareable Device, XN: TEX,C,B = 000,0,1.
const SECTION_DEVICE_XN: u32 = DESC_SECTION | DESC_B | DESC_AP_FULL | DESC_XN;

/// TTBR0: inner WB (C), shareable (S), outer WBWA (RGN=01). Table is 16 KiB aligned.
const TTBR_ATTR: u32 = (1 << 0) | (1 << 1) | (1 << 3);

const SCTLR_M: u32 = 1 << 0;
const SCTLR_C: u32 = 1 << 2;
const SCTLR_Z: u32 = 1 << 11;
const SCTLR_I: u32 = 1 << 12;

const ACTLR_FW: u32 = 1 << 0;
const ACTLR_SMP: u32 = 1 << 6;

pub const CPU1_READY: u32 = 0xA5A5_5A5A;

#[repr(C, align(32))]
pub struct Cpu1Mailbox {
    pub magic: AtomicU32,
    pub ttbr0: AtomicU32,
    pub status: AtomicU32,
    pub mpidr: AtomicU32,
}

#[no_mangle]
#[used]
pub static CPU1_MAILBOX: Cpu1Mailbox = Cpu1Mailbox {
    magic: AtomicU32::new(0),
    ttbr0: AtomicU32::new(0),
    status: AtomicU32::new(0),
    mpidr: AtomicU32::new(0xFFFF_FFFF),
};

/// Request ring CPU0→CPU1, result ring CPU1→CPU0. Not the T5 boot mailbox.
pub static CPU1_CHANNELS: Cpu1Channels = Cpu1Channels::new();

const T6_PING_COUNT: u32 = 32;
const T6_BURST: u32 = 16;
const T6_SPIN: u32 = 4_000_000;

extern "C" {
    static mut _mmu_l1: u32;
    fn _cpu1_entry();
}

#[inline]
fn dsb() {
    unsafe { core::arch::asm!("dsb", options(nomem, nostack, preserves_flags)) }
}

#[inline]
fn isb() {
    unsafe { core::arch::asm!("isb", options(nomem, nostack, preserves_flags)) }
}

#[inline]
fn sev() {
    unsafe { core::arch::asm!("sev", options(nomem, nostack, preserves_flags)) }
}

#[inline]
fn wfe() {
    unsafe { core::arch::asm!("wfe", options(nomem, nostack, preserves_flags)) }
}

#[inline]
unsafe fn mrc_sctlr() -> u32 {
    let v: u32;
    core::arch::asm!("mrc p15, 0, {}, c1, c0, 0", out(reg) v, options(nomem, nostack, preserves_flags));
    v
}

#[inline]
unsafe fn mcr_sctlr(v: u32) {
    core::arch::asm!("mcr p15, 0, {}, c1, c0, 0", in(reg) v, options(nomem, nostack, preserves_flags));
}

#[inline]
unsafe fn mrc_actlr() -> u32 {
    let v: u32;
    core::arch::asm!("mrc p15, 0, {}, c1, c0, 1", out(reg) v, options(nomem, nostack, preserves_flags));
    v
}

#[inline]
unsafe fn mcr_actlr(v: u32) {
    core::arch::asm!("mcr p15, 0, {}, c1, c0, 1", in(reg) v, options(nomem, nostack, preserves_flags));
}

unsafe fn fill_identity_l1() -> u32 {
    let table = core::ptr::addr_of_mut!(_mmu_l1) as *mut u32;
    let ddr_last_sec = (DDR_HIGH >> 20) as u32;
    for i in 0u32..4096 {
        let pa = i << 20;
        let attr = if i <= ddr_last_sec {
            SECTION_NORMAL_SHAREABLE
        } else {
            SECTION_DEVICE_XN
        };
        core::ptr::write_volatile(table.add(i as usize), pa | attr);
    }
    table as u32
}

unsafe fn invalidate_l1_dcache() {
    let zero = 0u32;
    core::arch::asm!("mcr p15, 2, {}, c0, c0, 0", in(reg) zero, options(nomem, nostack, preserves_flags));
    isb();
    let ccsidr: u32;
    core::arch::asm!("mrc p15, 1, {}, c0, c0, 0", out(reg) ccsidr, options(nomem, nostack, preserves_flags));
    let line_log2 = (ccsidr & 7) + 4;
    let assoc = ((ccsidr >> 3) & 0x3FF) + 1;
    let numsets = ((ccsidr >> 13) & 0x7FFF) + 1;
    if assoc == 0 || numsets == 0 || assoc > 64 || numsets > 4096 {
        return;
    }
    let way_shift = 32 - (31 - assoc.leading_zeros());
    for way in 0..assoc {
        for set in 0..numsets {
            let r = (way << way_shift) | (set << line_log2);
            core::arch::asm!("mcr p15, 0, {}, c7, c14, 2", in(reg) r, options(nomem, nostack, preserves_flags));
        }
    }
    dsb();
}

unsafe fn invalidate_icache_tlb() {
    let zero = 0u32;
    core::arch::asm!("mcr p15, 0, {}, c7, c5, 0", in(reg) zero, options(nomem, nostack, preserves_flags));
    core::arch::asm!("mcr p15, 0, {}, c8, c7, 0", in(reg) zero, options(nomem, nostack, preserves_flags));
    dsb();
    isb();
}

/// Identity-map DDR as Normal shareable, MMIO as Device; enable SCU; MMU on CPU0.
/// Does not send SEV. Mailbox + `0xFFFFFFF0` are written before D-cache on.
pub fn enable_mmu_scu_cpu0() {
    unsafe {
        let ttb_phys = fill_identity_l1();
        CPU1_MAILBOX
            .ttbr0
            .store(ttb_phys | TTBR_ATTR, Ordering::Relaxed);
        CPU1_MAILBOX.magic.store(0x4331_5531, Ordering::Relaxed);
        CPU1_MAILBOX.status.store(0, Ordering::Relaxed);
        CPU1_MAILBOX.mpidr.store(0xFFFF_FFFF, Ordering::Relaxed);

        let entry = _cpu1_entry as u32;
        core::ptr::write_volatile(CPU1_BOOT_VECTOR as *mut u32, entry);
        dsb();

        core::ptr::write_volatile(SCU_INV_ALL as *mut u32, 0xFFFF);
        let scu = core::ptr::read_volatile(SCU_CTRL as *const u32);
        core::ptr::write_volatile(SCU_CTRL as *mut u32, scu | SCU_EN);
        dsb();

        mcr_actlr(mrc_actlr() | ACTLR_SMP | ACTLR_FW);
        isb();

        invalidate_l1_dcache();
        invalidate_icache_tlb();

        core::arch::asm!("mcr p15, 0, {}, c2, c0, 0", in(reg) ttb_phys | TTBR_ATTR, options(nomem, nostack, preserves_flags));
        let zero = 0u32;
        core::arch::asm!("mcr p15, 0, {}, c2, c0, 2", in(reg) zero, options(nomem, nostack, preserves_flags));
        let dacr = 0xFFFF_FFFFu32;
        core::arch::asm!("mcr p15, 0, {}, c3, c0, 0", in(reg) dacr, options(nomem, nostack, preserves_flags));
        isb();

        mcr_sctlr(mrc_sctlr() | SCTLR_M | SCTLR_C | SCTLR_Z | SCTLR_I);
        isb();
        dsb();
    }
    uart::write_str("MMU: CPU0 identity (DDR Normal-S WBWA, MMIO Device); SCU on\n");
}

fn release_cpu1_if_held() {
    unsafe {
        let rst = core::ptr::read_volatile(A9_CPU_RST_CTRL as *const u32);
        if rst & (A9_CPU1_RST | A9_CPU1_CLKSTOP) == 0 {
            return;
        }
        core::ptr::write_volatile(SLCR_UNLOCK as *mut u32, SLCR_UNLOCK_KEY);
        let cleared = rst & !(A9_CPU1_RST | A9_CPU1_CLKSTOP);
        core::ptr::write_volatile(A9_CPU_RST_CTRL as *mut u32, cleared);
        core::ptr::write_volatile(SLCR_LOCK as *mut u32, SLCR_LOCK_KEY);
        dsb();
    }
}

/// SEV after CPU0 MMU/SCU. Report ONLINE only if CPU1 stored its MPIDR.
pub fn wake_cpu1_and_report() {
    release_cpu1_if_held();
    dsb();
    sev();

    let mut saw = false;
    for _ in 0..4_000_000u32 {
        if CPU1_MAILBOX.status.load(Ordering::Acquire) == CPU1_READY {
            saw = true;
            break;
        }
        core::hint::spin_loop();
    }

    let mpidr = CPU1_MAILBOX.mpidr.load(Ordering::Acquire);
    let status = CPU1_MAILBOX.status.load(Ordering::Acquire);
    if saw && mpidr == 1 {
        uart::write_str(&alloc::format!(
            "CPU1: ONLINE mpidr={mpidr} status=0x{status:08X}\n"
        ));
        run_spsc_probe();
    } else {
        uart::write_str(&alloc::format!(
            "CPU1: TIMEOUT parked mpidr=0x{mpidr:08X} status=0x{status:08X} (no MPIDR handshake)\n"
        ));
        uart::write_str("T6: SKIP (CPU1 not online)\n");
    }
}

fn spin_push(q: &crate::kernel::spsc_mailbox::SpscMailbox, task: MailboxTask) -> bool {
    for _ in 0..T6_SPIN {
        if q.push(task).is_ok() {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

fn spin_pop(q: &crate::kernel::spsc_mailbox::SpscMailbox) -> Option<MailboxTask> {
    for _ in 0..T6_SPIN {
        if let Some(t) = q.pop() {
            return Some(t);
        }
        core::hint::spin_loop();
    }
    None
}

/// CPU0 producer / CPU1 consumer ping. Order, wrap (32 > N=16), no silent loss.
fn run_spsc_probe() {
    for id in 0..T6_PING_COUNT {
        let task = MailboxTask::ping(id, id ^ 0xA5A5);
        if !spin_push(&CPU1_CHANNELS.requests, task) {
            uart::write_str(&alloc::format!("T6: FAIL push timeout seq={id}\n"));
            return;
        }
        match spin_pop(&CPU1_CHANNELS.results) {
            Some(got)
                if got.task_id == id && got.opcode == OPCODE_PONG && got.arg0 == task.arg0 => {}
            Some(got) => {
                uart::write_str(&alloc::format!(
                    "T6: FAIL ping seq={id} got id={} op=0x{:02X} arg0=0x{:08X}\n",
                    got.task_id,
                    got.opcode,
                    got.arg0
                ));
                return;
            }
            None => {
                uart::write_str(&alloc::format!("T6: FAIL result timeout seq={id}\n"));
                return;
            }
        }
    }

    let base = T6_PING_COUNT;
    for i in 0..T6_BURST {
        let id = base + i;
        let task = MailboxTask::ping(id, id.wrapping_mul(3));
        if !spin_push(&CPU1_CHANNELS.requests, task) {
            uart::write_str(&alloc::format!("T6: FAIL burst push timeout seq={id}\n"));
            return;
        }
    }
    for i in 0..T6_BURST {
        let id = base + i;
        match spin_pop(&CPU1_CHANNELS.results) {
            Some(got)
                if got.task_id == id
                    && got.opcode == OPCODE_PONG
                    && got.arg0 == id.wrapping_mul(3) => {}
            Some(got) => {
                uart::write_str(&alloc::format!(
                    "T6: FAIL burst seq={id} got id={} op=0x{:02X}\n",
                    got.task_id, got.opcode
                ));
                return;
            }
            None => {
                uart::write_str(&alloc::format!("T6: FAIL burst result timeout seq={id}\n"));
                return;
            }
        }
    }

    uart::write_str("T6: ping 32/32 order ok; burst 16/16 order ok (N=16 wrap)\n");
    #[cfg(feature = "tensor")]
    run_t7_probe();
}

#[cfg(feature = "tensor")]
fn run_t7_probe() {
    use crate::kernel::crypto::phase_hash::ph_tensor_proof;
    use crate::kernel::fault_syndrome::Health;
    use crate::kernel::phase_hash_parallel::{
        compute_split, sample_input, T7_SPLIT_COUNT, T7_WAIT_SPINS,
    };

    for i in 0..T7_SPLIT_COUNT {
        let a = sample_input(0x98, i as u64);
        let b = sample_input(0x99, i as u64);
        let got = compute_split(
            &CPU1_CHANNELS,
            &a,
            &b,
            Health::Valid,
            true,
            true,
            T7_WAIT_SPINS,
        );
        let ra = ph_tensor_proof(&a);
        let rb = ph_tensor_proof(&b);
        if !(got.used_cpu1 && got.hash_a == ra && got.hash_b == rb) {
            uart::write_str(&alloc::format!(
                "T7: FAIL seq={i} used_cpu1={} hash match a={} b={}\n",
                got.used_cpu1,
                got.hash_a == ra,
                got.hash_b == rb
            ));
            return;
        }
    }

    let before = CPU1_CHANNELS.phash_dispatched.load(Ordering::Acquire);
    let cases = [
        (Health::Suspect, true),
        (Health::Fault, true),
        (Health::Isolate, true),
        (Health::Valid, false),
    ];
    for (health, proof_ok) in cases {
        let a = sample_input(0x98, 100);
        let b = sample_input(0x99, 100);
        let got = compute_split(&CPU1_CHANNELS, &a, &b, health, proof_ok, true, 8);
        if got.used_cpu1 {
            uart::write_str(&alloc::format!(
                "T7: FAIL health fallback dispatched ({health:?} proof_ok={proof_ok})\n"
            ));
            return;
        }
        if got.hash_a != ph_tensor_proof(&a) || got.hash_b != ph_tensor_proof(&b) {
            uart::write_str("T7: FAIL health fallback digest mismatch\n");
            return;
        }
    }
    if CPU1_CHANNELS.phash_dispatched.load(Ordering::Acquire) != before {
        uart::write_str("T7: FAIL health fallback still incremented dispatch count\n");
        return;
    }
    uart::write_str("T7: parallel match 32/32, health policy fallback ok\n");
}

fn cpu1_handle(task: MailboxTask) -> MailboxTask {
    match task.opcode {
        OPCODE_PING => MailboxTask::pong(task.task_id, task.arg0),
        #[cfg(feature = "tensor")]
        crate::kernel::spsc_mailbox::OPCODE_PHASH => {
            crate::kernel::phase_hash_parallel::cpu1_eval_phash(task)
        }
        _ => MailboxTask {
            opcode: OPCODE_UNK,
            ..task
        },
    }
}

fn cpu1_task_loop() -> ! {
    loop {
        if let Some(task) = CPU1_CHANNELS.requests.pop() {
            let reply = cpu1_handle(task);
            while CPU1_CHANNELS.results.push(reply).is_err() {
                core::hint::spin_loop();
            }
        } else {
            wfe();
        }
    }
}

#[no_mangle]
pub extern "C" fn cpu1_rust_main(cpu_id: u32) -> ! {
    if cpu_id != 1 {
        loop {
            wfe();
        }
    }
    CPU1_MAILBOX.mpidr.store(cpu_id, Ordering::Release);
    CPU1_MAILBOX.status.store(CPU1_READY, Ordering::Release);
    cpu1_task_loop()
}
