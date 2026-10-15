//! CPU0 isolation MMU enable, user enter, SVC, abort.

use crate::arch::armv7::mmu::{
    abort_disposition, align_up_l1, fill_kernel_isolation_l1, fault_class, l1_base_ok,
    AbortDisposition, KernelLayout, DACR_CLIENT_D0, L1_ALLOC_PAGES, L1_BYTES, L1_ENTRIES,
    L2_BYTES, L2_ENTRIES, PAGE_SIZE, SCTLR_C, SCTLR_I, SCTLR_M, SCTLR_Z, SVC_STACK_BYTES,
    TTBR_ATTR, TTBCR_N0,
};
use crate::arch::armv7::payloads::{self, LEGAL_MAGIC, SYS_EXIT, SYS_WRITE};
use crate::board::zynq7000::memory_map::DDR_HIGH;
use crate::board::zynq7000::uart;
use crate::kernel::aspace::{install_user_window, unmap_user_window, AddressSpace, Perm, Range};
use crate::kernel::hash::StateHash;
use crate::kernel::Kernel;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

const ACTLR_FW: u32 = 1 << 0;
const ACTLR_SMP: u32 = 1 << 6;
const SCU_BASE: u32 = 0xF8F0_0000;
const SCU_CTRL: u32 = SCU_BASE;
const SCU_INV_ALL: u32 = SCU_BASE + 0x0C;
const SCU_EN: u32 = 1;
const L2_SLOTS: usize = 32;

#[repr(C)]
pub struct UserEntry {
    pub pc: u32,
    pub sp: u32,
    pub r0: u32,
    pub r1: u32,
}

extern "C" {
    fn leave_user(entry: *const UserEntry) -> u32;
    static mut kairos_kernel_ttbr: u32;
    static mut _mmu_l1: u32;
    static mut _mmu_l1_cpu1: u32;
    static mut _mmu_l2_kernel: u32;
    static mut _mmu_l2_cpu1: u32;
    static vectors: u32;
    static _text_end: u32;
    static _rodata_start: u32;
    static _rodata_end: u32;
    static _data_start: u32;
    static __bss_end: u32;
    static _stack_top: u32;
    static _irq_stack_top: u32;
    static _abort_stack_top: u32;
    static __heap_start: u32;
    static __heap_end: u32;
}

static ENABLED: AtomicBool = AtomicBool::new(false);
static LAST_FAR: AtomicU32 = AtomicU32::new(0);
static LAST_FSR: AtomicU32 = AtomicU32::new(0);
static LAST_PC: AtomicU32 = AtomicU32::new(0);
static LAST_IFETCH: AtomicU32 = AtomicU32::new(0);
static LAST_PID: AtomicU32 = AtomicU32::new(0);
static LAST_ASID: AtomicU32 = AtomicU32::new(0);
static LAST_GEN: AtomicU32 = AtomicU32::new(0);
static LAST_UNMAPPED: AtomicU32 = AtomicU32::new(0);
static NEXT_ASID: AtomicU32 = AtomicU32::new(1);

struct Active {
    kernel: *mut Kernel,
    space: AddressSpace,
}

static mut ACTIVE: Option<Active> = None;
static mut LAST_SPACE: Option<AddressSpace> = None;
static mut LAST_PACKAGE: StateHash = [0u8; 32];

#[inline]
fn dsb() {
    unsafe { core::arch::asm!("dsb", options(nomem, nostack, preserves_flags)) }
}

#[inline]
fn isb() {
    unsafe { core::arch::asm!("isb", options(nomem, nostack, preserves_flags)) }
}

unsafe fn mrc_sctlr() -> u32 {
    let v: u32;
    core::arch::asm!("mrc p15, 0, {}, c1, c0, 0", out(reg) v, options(nomem, nostack, preserves_flags));
    v
}

unsafe fn mcr_sctlr(v: u32) {
    core::arch::asm!("mcr p15, 0, {}, c1, c0, 0", in(reg) v, options(nomem, nostack, preserves_flags));
}

unsafe fn mrc_actlr() -> u32 {
    let v: u32;
    core::arch::asm!("mrc p15, 0, {}, c1, c0, 1", out(reg) v, options(nomem, nostack, preserves_flags));
    v
}

unsafe fn mcr_actlr(v: u32) {
    core::arch::asm!("mcr p15, 0, {}, c1, c0, 1", in(reg) v, options(nomem, nostack, preserves_flags));
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

fn live_layout() -> KernelLayout {
    unsafe {
        KernelLayout {
            vectors: core::ptr::addr_of!(vectors) as u32,
            text_start: core::ptr::addr_of!(vectors) as u32,
            text_end: core::ptr::addr_of!(_text_end) as u32,
            rodata_start: core::ptr::addr_of!(_rodata_start) as u32,
            rodata_end: core::ptr::addr_of!(_rodata_end) as u32,
            data_start: core::ptr::addr_of!(_data_start) as u32,
            bss_end: core::ptr::addr_of!(__bss_end) as u32,
            stack_top: core::ptr::addr_of!(_stack_top) as u32,
            irq_stack_top: core::ptr::addr_of!(_irq_stack_top) as u32,
            abort_stack_top: core::ptr::addr_of!(_abort_stack_top) as u32,
            mmu_l1: core::ptr::addr_of_mut!(_mmu_l1) as u32,
            heap_start: core::ptr::addr_of!(__heap_start) as u32,
            heap_end: core::ptr::addr_of!(__heap_end) as u32,
            ddr_high: DDR_HIGH,
        }
    }
}

pub fn enable() {
    unsafe {
        let layout = live_layout();
        let l1 = core::slice::from_raw_parts_mut(
            core::ptr::addr_of_mut!(_mmu_l1) as *mut u32,
            L1_ENTRIES,
        );
        let l2_kernel = core::slice::from_raw_parts_mut(
            core::ptr::addr_of_mut!(_mmu_l2_kernel) as *mut [u32; L2_ENTRIES],
            L2_SLOTS,
        );
        let l2k_phys = core::ptr::addr_of_mut!(_mmu_l2_kernel) as u32;
        fill_kernel_isolation_l1(l1, l2_kernel, l2k_phys, &layout, true)
            .expect("kernel L2 pool");

        let l1c = core::slice::from_raw_parts_mut(
            core::ptr::addr_of_mut!(_mmu_l1_cpu1) as *mut u32,
            L1_ENTRIES,
        );
        let l2_cpu1 = core::slice::from_raw_parts_mut(
            core::ptr::addr_of_mut!(_mmu_l2_cpu1) as *mut [u32; L2_ENTRIES],
            L2_SLOTS,
        );
        let l2c_phys = core::ptr::addr_of_mut!(_mmu_l2_cpu1) as u32;
        fill_kernel_isolation_l1(l1c, l2_cpu1, l2c_phys, &layout, false)
            .expect("cpu1 L2 pool");

        let cpu0_ttbr = (core::ptr::addr_of_mut!(_mmu_l1) as u32) | TTBR_ATTR;
        let cpu1_ttbr = (core::ptr::addr_of_mut!(_mmu_l1_cpu1) as u32) | TTBR_ATTR;
        kairos_kernel_ttbr = cpu0_ttbr;

        crate::board::zynq7000::cpu1::CPU1_MAILBOX
            .ttbr0
            .store(cpu1_ttbr, Ordering::Relaxed);
        crate::board::zynq7000::cpu1::CPU1_MAILBOX
            .magic
            .store(0x4331_5531, Ordering::Relaxed);
        crate::board::zynq7000::cpu1::CPU1_MAILBOX
            .status
            .store(0, Ordering::Relaxed);
        crate::board::zynq7000::cpu1::CPU1_MAILBOX
            .mpidr
            .store(0xFFFF_FFFF, Ordering::Relaxed);
        extern "C" {
            fn _cpu1_entry();
        }
        core::ptr::write_volatile(0xFFFF_FFF0u32 as *mut u32, _cpu1_entry as u32);
        dsb();

        core::ptr::write_volatile(SCU_INV_ALL as *mut u32, 0xFFFF);
        let scu = core::ptr::read_volatile(SCU_CTRL as *const u32);
        core::ptr::write_volatile(SCU_CTRL as *mut u32, scu | SCU_EN);
        dsb();
        mcr_actlr(mrc_actlr() | ACTLR_SMP | ACTLR_FW);
        isb();
        invalidate_l1_dcache();
        invalidate_icache_tlb();
        core::arch::asm!("mcr p15, 0, {}, c2, c0, 0", in(reg) cpu0_ttbr, options(nomem, nostack, preserves_flags));
        let n0 = TTBCR_N0;
        core::arch::asm!("mcr p15, 0, {}, c2, c0, 2", in(reg) n0, options(nomem, nostack, preserves_flags));
        let dacr = DACR_CLIENT_D0;
        core::arch::asm!("mcr p15, 0, {}, c3, c0, 0", in(reg) dacr, options(nomem, nostack, preserves_flags));
        isb();
        mcr_sctlr(mrc_sctlr() | SCTLR_M | SCTLR_C | SCTLR_Z | SCTLR_I);
        isb();
        dsb();
    }
    ENABLED.store(true, Ordering::SeqCst);
    uart::write_str("MMU: CPU0 isolation (Client D0, L2 kernel W^X, PMM priv); CPU1 kernel-only L1\n");
    uart::write_str("MMU: HARDWARE_BOUND apps=CPU0-only; SMP TLB shootdown=NO\n");
    // Printed at enable so a boot that never reaches `gpos>` still shows the
    // map the fault-classifier was built from.
    let l = live_layout();
    uart::write_str(&alloc::format!(
        "MMU: ro={:08x}..{:08x} data={:08x} aligned={}\n",
        l.rodata_start,
        l.rodata_end,
        l.data_start,
        if l.boundaries_page_aligned() { "YES" } else { "NO" },
    ));
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

pub fn status_lines() -> crate::platform::String {
    use crate::platform::String;
    let mut s = String::new();
    s.push_str("MMU_ISOLATION: ");
    s.push_str(if enabled() { "ON\n" } else { "OFF\n" });
    s.push_str("KEY_BACKEND: N/A\n");
    s.push_str("DACR: CLIENT_D0\n");
    s.push_str("TTBCR.N: 0 (TTBR0 Model A)\n");
    s.push_str("APP_PIN: CPU0\n");
    s.push_str("DMA_PROTECTION: NO\n");
    let l = live_layout();
    s.push_str(&alloc::format!(
        "LAYOUT text={:08x}..{:08x} ro={:08x}..{:08x} data={:08x}..{:08x} heap={:08x}..{:08x}\n",
        l.text_start, l.text_end, l.rodata_start, l.rodata_end, l.data_start, l.bss_end,
        l.heap_start, l.heap_end,
    ));
    s.push_str("PAGE_ALIGNED_BOUNDARIES: ");
    s.push_str(if l.boundaries_page_aligned() { "YES\n" } else { "NO\n" });
    s.push_str(&alloc::format!(
        "LAST_FAULT far={:08x} fsr={:08x} pc={:08x} class={} prefetch={} pid={} asid={} gen={}\n",
        LAST_FAR.load(Ordering::SeqCst),
        LAST_FSR.load(Ordering::SeqCst),
        LAST_PC.load(Ordering::SeqCst),
        fault_class(LAST_FSR.load(Ordering::SeqCst)).as_str(),
        LAST_IFETCH.load(Ordering::SeqCst),
        LAST_PID.load(Ordering::SeqCst),
        LAST_ASID.load(Ordering::SeqCst),
        LAST_GEN.load(Ordering::SeqCst),
    ));
    s.push_str("LAST_UNMAP: ");
    s.push_str(if LAST_UNMAPPED.load(Ordering::SeqCst) != 0 {
        "YES\n"
    } else {
        "NO\n"
    });
    s
}

/// Invalidate the last user window for `pid` and flush TLB. No-op if no
/// matching window. Not a scheduler AS switch (H02). CPU0 only (H03).
pub fn unmap_contained(pid: u32) -> bool {
    unsafe { unmap_contained_inner(pid) }
}

unsafe fn unmap_contained_inner(pid: u32) -> bool {
    let Some(space) = LAST_SPACE else {
        return false;
    };
    if space.process_id != pid {
        return false;
    }
    let l1 = core::slice::from_raw_parts_mut(
        space.translation_table_root as *mut u32,
        L1_ENTRIES,
    );
    let l2 = core::slice::from_raw_parts_mut(space.l2_phys as *mut u32, L2_ENTRIES);
    unmap_user_window(l1, l2, &space);
    clean_dcache_range(space.l2_phys, L2_BYTES as u32);
    clean_dcache_range(space.translation_table_root, L1_BYTES as u32);
    switch_ttbr(kairos_kernel_ttbr);
    LAST_SPACE = None;
    LAST_UNMAPPED.store(1, Ordering::SeqCst);
    uart::write_str(&alloc::format!(
        "MMUISO: unmap pid={} asid={} l1={:08x}\n",
        pid,
        space.asid,
        space.translation_table_root,
    ));
    true
}

unsafe fn clean_dcache_range(start: u32, len: u32) {
    let mut p = start & !31;
    let end = start.wrapping_add(len);
    while p < end {
        core::arch::asm!("mcr p15, 0, {}, c7, c10, 1", in(reg) p, options(nostack, preserves_flags));
        p = p.wrapping_add(32);
    }
    dsb();
}

fn switch_ttbr(ttbr: u32) {
    unsafe {
        core::arch::asm!("mcr p15, 0, {}, c2, c0, 0", in(reg) ttbr, options(nomem, nostack, preserves_flags));
        dsb();
        let z = 0u32;
        core::arch::asm!("mcr p15, 0, {}, c8, c7, 0", in(reg) z, options(nomem, nostack, preserves_flags));
        dsb();
        isb();
    }
}

pub fn run_named(kernel: &mut Kernel, name: &str, peer_data: Option<u32>) -> Result<bool, &'static str> {
    let payload = payloads::matrix()
        .into_iter()
        .find(|p| p.name == name)
        .ok_or("unknown payload")?;
    run_blob(
        kernel,
        &payload.code,
        payload.expect_fault,
        payload.data_is_pc,
        peer_data,
        None,
        [0u8; 32],
        0,
    )
}

pub fn run_raw_a32(
    kernel: &mut Kernel,
    code: &[u8],
    pid: u32,
    package_id: StateHash,
) -> Result<bool, &'static str> {
    run_blob(kernel, code, false, false, None, Some(pid), package_id, 0)
}

pub fn run_elf(
    kernel: &mut Kernel,
    load: &[u8],
    entry_off: u32,
    pid: u32,
    package_id: StateHash,
) -> Result<bool, &'static str> {
    if load.is_empty() || load.len() > PAGE_SIZE as usize {
        return Err("elf load");
    }
    if (entry_off as usize) >= load.len() {
        return Err("elf entry");
    }
    run_blob(
        kernel,
        load,
        false,
        false,
        None,
        Some(pid),
        package_id,
        entry_off,
    )
}

fn run_blob(
    kernel: &mut Kernel,
    code: &[u8],
    expect_fault: bool,
    data_is_pc: bool,
    peer_data: Option<u32>,
    existing_pid: Option<u32>,
    package_id: StateHash,
    pc_off: u32,
) -> Result<bool, &'static str> {
    if !enabled() {
        return Err("mmu isolation off");
    }
    let copy_n = code.len().min(PAGE_SIZE as usize);
    if copy_n == 0 || (pc_off as usize) >= copy_n {
        return Err("elf entry");
    }
    let pages = kernel
        .physical_memory
        .alloc(4)
        .ok_or("pmm alloc")?;
    let code_pa = pages as u32;
    let data_pa = code_pa + PAGE_SIZE;
    let guard_pa = data_pa + PAGE_SIZE;
    let stack_pa = guard_pa + PAGE_SIZE;
    // The PMM base is `__heap_end` (0x01321000 on this image), which is 4 KiB
    // but not 16 KiB aligned, so a plain `alloc(4)` can never return a legal
    // TTBR0 base. Over-allocate and carve the aligned table out of the block;
    // `l1_alloc` stays the allocator-owned address for quarantine.
    let l1_alloc = kernel
        .physical_memory
        .alloc(L1_ALLOC_PAGES)
        .ok_or("pmm l1")?;
    let l1_pa = align_up_l1(l1_alloc as u32) as usize;
    if !l1_base_ok(l1_pa as u32) {
        let _ = kernel.physical_memory.quarantine(l1_alloc, L1_ALLOC_PAGES);
        return Err("l1 align");
    }
    let l2_pa = kernel.physical_memory.alloc(1).ok_or("pmm l2")?;
    unsafe {
        let dst = l1_pa as *mut u32;
        let src = core::ptr::addr_of_mut!(_mmu_l1) as *const u32;
        core::ptr::copy_nonoverlapping(src, dst, L1_ENTRIES);
        let l2 = core::slice::from_raw_parts_mut(l2_pa as *mut u32, L2_ENTRIES);
        let l1 = core::slice::from_raw_parts_mut(dst, L1_ENTRIES);
        core::ptr::write_bytes(code_pa as *mut u8, 0, PAGE_SIZE as usize);
        core::ptr::write_bytes(data_pa as *mut u8, 0, PAGE_SIZE as usize);
        core::ptr::copy_nonoverlapping(code.as_ptr(), code_pa as *mut u8, copy_n);
        core::ptr::write_volatile(data_pa as *mut u32, LEGAL_MAGIC);
        let msg = payloads::legal_message();
        core::ptr::copy_nonoverlapping(msg.as_ptr(), (data_pa as *mut u8).add(8), msg.len());
        let asid = NEXT_ASID.fetch_add(1, Ordering::SeqCst) as u8;
        let space = AddressSpace {
            process_id: 1,
            package_id,
            payload_root: package_id,
            translation_table_root: l1_pa as u32,
            l2_phys: l2_pa as u32,
            code: Range {
                base: code_pa,
                len: PAGE_SIZE,
            },
            data: Range {
                base: data_pa,
                len: PAGE_SIZE,
            },
            stack: Range {
                base: stack_pa,
                len: PAGE_SIZE,
            },
            guard: Range {
                base: guard_pa,
                len: PAGE_SIZE,
            },
            asid: if asid == 0 { 1 } else { asid },
            generation: 1,
        };
        install_user_window(l1, l2, l2_pa as u32, &space).map_err(|_| "map user")?;
        clean_dcache_range(l1_pa as u32, (L1_ENTRIES * 4) as u32);
        clean_dcache_range(l2_pa as u32, (L2_ENTRIES * 4) as u32);
        clean_dcache_range(code_pa, PAGE_SIZE * 4);
        let pid = match existing_pid {
            Some(p) => p,
            None => kernel.spawn_process("mmuiso"),
        };
        let mut space = space;
        space.process_id = pid;
        ACTIVE = Some(Active {
            kernel: kernel as *mut Kernel,
            space,
        });
        LAST_PACKAGE = space.package_id;
        let arg0 = if let Some(peer) = peer_data {
            peer
        } else if data_is_pc {
            data_pa
        } else {
            data_pa
        };
        let entry = UserEntry {
            pc: code_pa.wrapping_add(pc_off),
            sp: stack_pa + PAGE_SIZE,
            r0: arg0,
            r1: LEGAL_MAGIC,
        };
        // Live markers: `run_matrix` accumulates its report and only prints at
        // the end, so without these a hang anywhere in the user round trip
        // produces no output at all and cannot be localised.
        uart::write_str(&alloc::format!(
            "MMUISO: l1={:08x} code={:08x} sp={:08x} enter\n",
            l1_pa as u32,
            entry.pc,
            entry.sp,
        ));
        switch_ttbr((l1_pa as u32) | TTBR_ATTR);
        let rc = leave_user(&entry);
        switch_ttbr(kairos_kernel_ttbr);
        uart::write_str(&alloc::format!("MMUISO: rc={} back\n", rc));
        ACTIVE = None;
        LAST_SPACE = Some(space);
        let faulted = rc == 2;
        if faulted {
            isolate_active(kernel, pid, space);
        } else {
            let _ = kernel.kill_process(pid);
        }
        let _ = kernel.physical_memory.quarantine(pages, 4);
        let _ = kernel.physical_memory.quarantine(l1_alloc, L1_ALLOC_PAGES);
        let _ = kernel.physical_memory.quarantine(l2_pa, 1);
        Ok(if expect_fault { faulted } else { !faulted })
    }
}

fn isolate_active(kernel: &mut Kernel, pid: u32, space: AddressSpace) {
    LAST_PID.store(pid, Ordering::SeqCst);
    LAST_ASID.store(space.asid as u32, Ordering::SeqCst);
    LAST_GEN.store(space.generation, Ordering::SeqCst);
    #[cfg(feature = "phasesentinel")]
    {
        let _ = kernel.sentinel_isolate_task(pid);
    }
    #[cfg(not(feature = "phasesentinel"))]
    {
        let _ = unmap_contained(pid);
        let _ = kernel.kill_process(pid);
    }
    uart::write_str("MMU_VIOLATION: ISOLATED\n");
}

pub fn run_matrix(kernel: &mut Kernel) -> crate::platform::String {
    use crate::platform::String;
    let mut out = String::new();
    let peer_pages = kernel.physical_memory.alloc(1);
    let peer = peer_pages.unwrap_or(0) as u32;
    if peer != 0 {
        unsafe {
            core::ptr::write_volatile(peer as *mut u32, 0xDEAD_BEEF);
        }
    }
    for p in payloads::matrix() {
        let peer_arg = if p.needs_peer { Some(peer) } else { None };
        match run_blob(kernel, &p.code, p.expect_fault, p.data_is_pc, peer_arg, None, [0u8; 32], 0) {
            Ok(true) => {
                out.push_str(p.name);
                out.push_str(": PASS\n");
            }
            Ok(false) => {
                out.push_str(p.name);
                out.push_str(": FAIL\n");
            }
            Err(e) => {
                out.push_str(p.name);
                out.push_str(": ERR ");
                out.push_str(e);
                out.push('\n');
            }
        }
    }
    // Status goes last so `LAST_FAULT` reflects this run. Printing it first
    // reported far=00000000 even after every probe had faulted.
    out.push_str(&status_lines());
    out.push_str("mmuiso matrix done\n");
    out
}

#[no_mangle]
pub extern "C" fn kairos_svc_dispatch(frame: *mut u32) -> u32 {
    unsafe {
        let spsr = *frame;
        let r0 = *frame.add(1);
        let r1 = *frame.add(2);
        let r7 = *frame.add(8);
        let _ = spsr;
        match r7 {
            SYS_WRITE => {
                if let Some(active) = ACTIVE.as_ref() {
                    let space = active.space;
                    if space.range_valid(r0, r1, Perm::Read) {
                        let len = r1.min(64) as usize;
                        let mut buf = [0u8; 64];
                        core::ptr::copy_nonoverlapping(r0 as *const u8, buf.as_mut_ptr(), len);
                        if let Ok(s) = core::str::from_utf8(&buf[..len]) {
                            uart::write_str(s);
                        }
                    }
                }
                0
            }
            SYS_EXIT => 1,
            _ => 1,
        }
    }
}

#[no_mangle]
pub extern "C" fn kairos_user_abort(far: u32, fsr: u32, pc: u32, ifetch: u32) {
    LAST_FAR.store(far, Ordering::SeqCst);
    LAST_FSR.store(fsr, Ordering::SeqCst);
    LAST_PC.store(pc, Ordering::SeqCst);
    LAST_IFETCH.store(ifetch, Ordering::SeqCst);
    unsafe {
        if let Some(active) = ACTIVE.as_ref() {
            LAST_PID.store(active.space.process_id, Ordering::SeqCst);
            LAST_ASID.store(active.space.asid as u32, Ordering::SeqCst);
            LAST_GEN.store(active.space.generation, Ordering::SeqCst);
        }
    }
    uart::write_str(&alloc::format!(
        "MMU abort class={} far={:08x} fsr={:08x} pc={:08x}\n",
        fault_class(fsr).as_str(),
        far,
        fsr,
        pc
    ));
}

#[repr(C)]
pub struct AbortFrame {
    pub far: u32,
    pub fsr: u32,
    pub pc: u32,
    pub ifetch: u32,
    pub saved_sp: u32,
    pub spsr: u32,
    pub depth: u32,
    pub stack_top: u32,
}

/// 0 = restore `leave_user` frame. 1 = nested abort SLCR. 2 = first abort halt. Never `RECOVERED`.
#[no_mangle]
pub extern "C" fn kairos_abort_decide(frame: *const AbortFrame) -> u32 {
    let f = unsafe { &*frame };
    kairos_user_abort(f.far, f.fsr, f.pc, f.ifetch);
    match abort_disposition(f.depth, f.saved_sp, f.stack_top, SVC_STACK_BYTES, f.spsr) {
        AbortDisposition::ReturnToKernel => 0,
        AbortDisposition::WatchdogReset => {
            uart::write_str("MMU_VIOLATION: ISOLATED\n");
            uart::write_str("C10_WATCHDOG_RESET\n");
            1
        }
        AbortDisposition::HaltIsolated => {
            uart::write_str("MMU_VIOLATION: ISOLATED\n");
            uart::write_str("C10_HALT\n");
            2
        }
    }
}

fn drain_uart_bounded() {
    const SR: u32 = 0xE000_002C;
    const TXEMPTY: u32 = 1 << 3;
    for _ in 0..100_000 {
        let sr = unsafe { core::ptr::read_volatile(SR as *const u32) };
        if sr & TXEMPTY != 0 {
            break;
        }
    }
}

#[no_mangle]
pub extern "C" fn kairos_c10_watchdog() -> ! {
    drain_uart_bounded();
    unsafe {
        core::ptr::write_volatile(0xF800_0008 as *mut u32, 0x0000_DF0D);
        core::ptr::write_volatile(0xF800_0200 as *mut u32, 1);
    }
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}

pub fn last_fault_prefetch() -> bool {
    LAST_IFETCH.load(Ordering::SeqCst) != 0
}
