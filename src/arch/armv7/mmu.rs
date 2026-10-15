//! ARMv7-A short-descriptor constructors (Cortex-A9).
//!
//! Host-safe: no coprocessor access. Board code in `isolation` programs TTBR.

#![allow(dead_code)]

pub const SECTION_SIZE: u32 = 0x0010_0000;
pub const PAGE_SIZE: u32 = 0x1000;
pub const L1_ENTRIES: usize = 4096;
pub const L2_ENTRIES: usize = 256;
pub const L1_BYTES: usize = L1_ENTRIES * 4;
pub const L2_BYTES: usize = L2_ENTRIES * 4;

/// TTBR0 with `TTBCR.N == 0` takes the L1 base from bits[31:14]; bits[13:0] are
/// attribute bits. An L1 table that is only 4 KiB aligned therefore has its low
/// address bits reinterpreted as attributes and the MMU walks a *different*
/// 16 KiB block. Every L1 base must satisfy this alignment.
pub const L1_ALIGN: u32 = 0x4000;

/// Frames to request so an aligned L1 can be carved out of the result. A 16 KiB
/// table needs at most 12 KiB of lead-in padding when the allocator base is only
/// 4 KiB aligned, so 32 KiB always contains an aligned 16 KiB block.
pub const L1_ALLOC_PAGES: u32 = 8;

/// Round `addr` up to the next valid L1 table base.
pub fn align_up_l1(addr: u32) -> u32 {
    (addr.wrapping_add(L1_ALIGN - 1)) & !(L1_ALIGN - 1)
}

/// True if `addr` is a legal TTBR0 table base for `TTBCR.N == 0`.
pub fn l1_base_ok(addr: u32) -> bool {
    addr != 0 && addr % L1_ALIGN == 0
}

pub const DACR_CLIENT_D0: u32 = 0b01;
pub const DACR_MANAGER_ALL: u32 = 0xFFFF_FFFF;

pub const TTBR_IRGN_WB: u32 = 1 << 0;
pub const TTBR_S: u32 = 1 << 1;
pub const TTBR_RGN_WBWA: u32 = 1 << 3;
pub const TTBR_ATTR: u32 = TTBR_IRGN_WB | TTBR_S | TTBR_RGN_WBWA;

pub const SCTLR_M: u32 = 1 << 0;
pub const SCTLR_C: u32 = 1 << 2;
pub const SCTLR_Z: u32 = 1 << 11;
pub const SCTLR_I: u32 = 1 << 12;

pub const TTBCR_N0: u32 = 0;

const DESC_SECTION: u32 = 0b10;
const DESC_SECTION_PXN: u32 = 0b11;
const DESC_PAGE_TABLE: u32 = 0b01;
const DESC_SMALL_PAGE: u32 = 0b10;

const DESC_B: u32 = 1 << 2;
const DESC_C: u32 = 1 << 3;
const DESC_XN_SECTION: u32 = 1 << 4;
const DESC_DOMAIN0: u32 = 0;
const DESC_TEX0: u32 = 1 << 12;
const DESC_S_SECTION: u32 = 1 << 16;
const DESC_NG_SECTION: u32 = 1 << 17;
const DESC_PXN_PAGETABLE: u32 = 1 << 2;

const AP2_SECTION: u32 = 1 << 15;
const AP10_SECTION_SHIFT: u32 = 10;

const AP2_SMALL: u32 = 1 << 9;
const AP10_SMALL_SHIFT: u32 = 4;
const TEX0_SMALL: u32 = 1 << 6;
const S_SMALL: u32 = 1 << 10;
const XN_SMALL: u32 = 1 << 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    PrivRwUserNone,
    PrivRoUserNone,
    PrivRoUserRo,
    PrivRwUserRw,
}

impl Access {
    fn ap2_ap10(self) -> (bool, u32) {
        match self {
            Self::PrivRwUserNone => (false, 0b01),
            Self::PrivRoUserNone => (true, 0b01),
            Self::PrivRoUserRo => (true, 0b10),
            Self::PrivRwUserRw => (false, 0b11),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemType {
    NormalShareableWbwa,
    DeviceShareable,
}

impl MemType {
    fn tex_c_b_s(self) -> (bool, bool, bool, bool) {
        match self {
            Self::NormalShareableWbwa => (true, true, true, true),
            Self::DeviceShareable => (false, false, true, true),
        }
    }
}

fn section_ap(access: Access) -> u32 {
    let (ap2, ap10) = access.ap2_ap10();
    ((ap10) << AP10_SECTION_SHIFT) | if ap2 { AP2_SECTION } else { 0 }
}

fn small_ap(access: Access) -> u32 {
    let (ap2, ap10) = access.ap2_ap10();
    ((ap10) << AP10_SMALL_SHIFT) | if ap2 { AP2_SMALL } else { 0 }
}

fn section_mem(mem: MemType) -> u32 {
    let (tex0, c, b, s) = mem.tex_c_b_s();
    (if tex0 { DESC_TEX0 } else { 0 })
        | (if c { DESC_C } else { 0 })
        | (if b { DESC_B } else { 0 })
        | (if s { DESC_S_SECTION } else { 0 })
}

fn small_mem(mem: MemType) -> u32 {
    let (tex0, c, b, s) = mem.tex_c_b_s();
    (if tex0 { TEX0_SMALL } else { 0 })
        | (if c { DESC_C } else { 0 })
        | (if b { DESC_B } else { 0 })
        | (if s { S_SMALL } else { 0 })
}

pub fn section_desc(pa: u32, access: Access, mem: MemType, xn: bool, pxn: bool) -> u32 {
    let ty = if pxn { DESC_SECTION_PXN } else { DESC_SECTION };
    (pa & 0xFFF0_0000)
        | section_mem(mem)
        | section_ap(access)
        | DESC_DOMAIN0
        | ty
        | if xn { DESC_XN_SECTION } else { 0 }
}

pub fn kernel_text_section(pa: u32) -> u32 {
    section_desc(pa, Access::PrivRoUserNone, MemType::NormalShareableWbwa, false, false)
}

pub fn kernel_ro_section(pa: u32) -> u32 {
    section_desc(pa, Access::PrivRoUserNone, MemType::NormalShareableWbwa, true, false)
}

pub fn kernel_rw_section(pa: u32) -> u32 {
    section_desc(pa, Access::PrivRwUserNone, MemType::NormalShareableWbwa, true, false)
}

pub fn device_section(pa: u32) -> u32 {
    section_desc(pa, Access::PrivRwUserNone, MemType::DeviceShareable, true, false)
}

pub fn l1_page_table(l2_phys: u32, pxn: bool) -> u32 {
    (l2_phys & 0xFFFF_FC00) | DESC_DOMAIN0 | DESC_PAGE_TABLE | if pxn { DESC_PXN_PAGETABLE } else { 0 }
}

pub fn small_page(pa: u32, access: Access, mem: MemType, xn: bool) -> u32 {
    (pa & 0xFFFF_F000)
        | small_mem(mem)
        | small_ap(access)
        | DESC_SMALL_PAGE
        | if xn { XN_SMALL } else { 0 }
}

pub fn kernel_text_page(pa: u32) -> u32 {
    small_page(pa, Access::PrivRoUserNone, MemType::NormalShareableWbwa, false)
}

pub fn kernel_ro_page(pa: u32) -> u32 {
    small_page(pa, Access::PrivRoUserNone, MemType::NormalShareableWbwa, true)
}

pub fn kernel_rw_page(pa: u32) -> u32 {
    small_page(pa, Access::PrivRwUserNone, MemType::NormalShareableWbwa, true)
}

pub fn user_rx_page(pa: u32) -> u32 {
    small_page(pa, Access::PrivRoUserRo, MemType::NormalShareableWbwa, false)
}

pub fn user_rw_page(pa: u32) -> u32 {
    small_page(pa, Access::PrivRwUserRw, MemType::NormalShareableWbwa, true)
}

pub fn invalid_desc() -> u32 {
    0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    Unaligned,
    CrossesSection,
    Overlap,
    OutOfRange,
    NoL2,
    WrongPerm,
}

pub fn section_index(va: u32) -> usize {
    (va / SECTION_SIZE) as usize
}

pub fn page_index_in_section(va: u32) -> usize {
    ((va / PAGE_SIZE) % (L2_ENTRIES as u32)) as usize
}

pub fn ranges_overlap(a: u32, a_len: u32, b: u32, b_len: u32) -> bool {
    if a_len == 0 || b_len == 0 {
        return false;
    }
    let a_end = a.wrapping_add(a_len);
    let b_end = b.wrapping_add(b_len);
    if a_end < a || b_end < b {
        return true;
    }
    a < b_end && b < a_end
}

pub fn add_len(ptr: u32, len: u32) -> Option<u32> {
    ptr.checked_add(len)
}

/// Install 4 KiB mappings into one L2. Rejects a span that crosses a 1 MiB section.
pub fn map_pages(
    l1: &mut [u32],
    l2: &mut [u32],
    l2_phys: u32,
    va: u32,
    pa: u32,
    bytes: u32,
    desc: u32,
    l1_pxn: bool,
) -> Result<(), MapError> {
    if va % PAGE_SIZE != 0 || pa % PAGE_SIZE != 0 || bytes % PAGE_SIZE != 0 || bytes == 0 {
        return Err(MapError::Unaligned);
    }
    let end = add_len(va, bytes).ok_or(MapError::OutOfRange)?;
    if section_index(va) != section_index(end.saturating_sub(1)) {
        return Err(MapError::CrossesSection);
    }
    if l1.len() != L1_ENTRIES || l2.len() != L2_ENTRIES {
        return Err(MapError::OutOfRange);
    }
    let si = section_index(va);
    let existing = l1[si];
    if (existing & 0b11) == DESC_SECTION || (existing & 0b11) == DESC_SECTION_PXN {
        return Err(MapError::Overlap);
    }
    if (existing & 0b11) == 0 {
        l1[si] = l1_page_table(l2_phys, l1_pxn);
    } else if (existing & 0xFFFF_FC00) != (l2_phys & 0xFFFF_FC00) {
        return Err(MapError::NoL2);
    }
    let n = (bytes / PAGE_SIZE) as usize;
    for i in 0..n {
        let idx = page_index_in_section(va + (i as u32) * PAGE_SIZE);
        if l2[idx] != 0 {
            return Err(MapError::Overlap);
        }
        l2[idx] = (desc & 0xFFF) | ((pa + (i as u32) * PAGE_SIZE) & 0xFFFF_F000);
    }
    Ok(())
}

pub fn lookup_perm(l1: &[u32], l2_for: impl Fn(u32) -> Option<[u32; L2_ENTRIES]>, va: u32) -> Option<u32> {
    let si = section_index(va);
    if si >= l1.len() {
        return None;
    }
    let d = l1[si];
    let ty = d & 0b11;
    if ty == DESC_SECTION || ty == DESC_SECTION_PXN {
        return Some(d);
    }
    if ty == DESC_PAGE_TABLE {
        let l2_phys = d & 0xFFFF_FC00;
        let l2 = l2_for(l2_phys)?;
        return Some(l2[page_index_in_section(va)]);
    }
    None
}

pub fn fault_class(fsr: u32) -> FaultClass {
    match fsr & 0xF {
        0x1 => FaultClass::Alignment,
        0x5 | 0x7 => FaultClass::Translation,
        0xD | 0xF => FaultClass::Permission,
        0x3 => FaultClass::AccessFlag,
        _ => FaultClass::Other,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultClass {
    Alignment,
    Translation,
    Permission,
    AccessFlag,
    Other,
}

impl FaultClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Alignment => "ALIGNMENT",
            Self::Translation => "TRANSLATION",
            Self::Permission => "PERMISSION",
            Self::AccessFlag => "ACCESS_FLAG",
            Self::Other => "OTHER",
        }
    }
}

/// SVC stack size in `linker_iso.ld` (`. += 0x10000; _stack_top = .`).
pub const SVC_STACK_BYTES: u32 = 0x10000;
/// `leave_user` pushes `{r4-r11, lr}` (9 words) before USR.
pub const KERNEL_FRAME_BYTES: u32 = 36;
pub const CPSR_MODE_MASK: u32 = 0x1F;
pub const CPSR_MODE_USR: u32 = 0x10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbortDisposition {
    /// User abort, saved SVC frame is in range. Return through `leave_user`.
    ReturnToKernel,
    /// First kernel/bad-frame abort. Print ISOLATED and halt. Never `RECOVERED`.
    HaltIsolated,
    /// Nested abort only. SLCR reset. Never `RECOVERED`.
    WatchdogReset,
}

/// Saved SVC SP must be 4-byte aligned, non-zero, and hold the 9-word frame
/// inside `[_stack_top - SVC_STACK_BYTES, _stack_top)`.
pub fn saved_kernel_sp_ok(sp: u32, stack_top: u32, stack_bytes: u32) -> bool {
    if sp == 0 || (sp & 3) != 0 {
        return false;
    }
    let Some(stack_base) = stack_top.checked_sub(stack_bytes) else {
        return false;
    };
    let Some(frame_end) = sp.checked_add(KERNEL_FRAME_BYTES) else {
        return false;
    };
    sp >= stack_base && frame_end <= stack_top
}

/// C10 abort decision. `depth` is 1 on the first abort, >=2 if nested.
pub fn abort_disposition(
    depth: u32,
    saved_sp: u32,
    stack_top: u32,
    stack_bytes: u32,
    spsr: u32,
) -> AbortDisposition {
    if depth > 1 {
        return AbortDisposition::WatchdogReset;
    }
    if !saved_kernel_sp_ok(saved_sp, stack_top, stack_bytes) {
        return AbortDisposition::HaltIsolated;
    }
    if (spsr & CPSR_MODE_MASK) != CPSR_MODE_USR {
        return AbortDisposition::HaltIsolated;
    }
    AbortDisposition::ReturnToKernel
}

/// Measured Utility ELF `8ae130c` layout (Gate 1). Used as the host fixture
/// and as the live symbol contract on the board.
pub struct KernelLayout {
    pub vectors: u32,
    pub text_start: u32,
    pub text_end: u32,
    pub rodata_start: u32,
    pub rodata_end: u32,
    pub data_start: u32,
    pub bss_end: u32,
    pub stack_top: u32,
    pub irq_stack_top: u32,
    pub abort_stack_top: u32,
    pub mmu_l1: u32,
    pub heap_start: u32,
    pub heap_end: u32,
    pub ddr_high: u32,
}

pub const MEASURED_UTILITY_8AE130C: KernelLayout = KernelLayout {
    vectors: 0x0010_0040,
    text_start: 0x0010_0040,
    text_end: 0x002A_FB24,
    rodata_start: 0x002A_FB40,
    rodata_end: 0x002E_1918,
    data_start: 0x002E_1920,
    bss_end: 0x002E_A118,
    stack_top: 0x002F_A120,
    irq_stack_top: 0x002F_C120,
    abort_stack_top: 0,
    mmu_l1: 0x0030_0000,
    heap_start: 0x0030_9000,
    heap_end: 0x0130_9000,
    ddr_high: 0x1FFF_FFFF,
};

/// Symbols read with `arm-none-eabi-nm` from the isolation ELF
/// `B30D943511FC4684814C31D228603EF394D5F69B7D594C6CF1CA4AEB695233F2`
/// (`BOOT_MMU_ISOLATION_20260815`), the image that reboot-looped on the board.
///
/// `_rodata_end` and `_data_start` land in the same 4 KiB page (`0x002e5000`),
/// which is what mapped `.data` read-only. Kept as a fixture so the regression
/// is checked against a layout that really shipped, not a synthetic one.
pub const MEASURED_ISO_20260815: KernelLayout = KernelLayout {
    vectors: 0x0010_0040,
    text_start: 0x0010_0040,
    text_end: 0x002B_2DD4,
    rodata_start: 0x002B_2DE0,
    rodata_end: 0x002E_51F8,
    data_start: 0x002E_5200,
    bss_end: 0x002E_D128,
    stack_top: 0x002F_D130,
    irq_stack_top: 0x002F_F130,
    abort_stack_top: 0x0030_1130,
    mmu_l1: 0x0030_4000,
    heap_start: 0x0032_1000,
    heap_end: 0x0132_1000,
    ddr_high: 0x1FFF_FFFF,
};

/// Data address that took the boot-time permission abort on that image
/// (`MMU abort class=PERMISSION far=002e5f90 fsr=0000080f`).
pub const ISO_20260815_ABORT_FAR: u32 = 0x002E_5F90;

/// True if a privileged (SVC) write to this small-page descriptor is allowed.
pub fn priv_may_write(desc: u32) -> bool {
    if (desc & 0b11) != DESC_SMALL_PAGE && (desc & 0b11) != (DESC_SMALL_PAGE | XN_SMALL) {
        return false;
    }
    let ap2 = (desc & AP2_SMALL) != 0;
    let ap10 = (desc >> AP10_SMALL_SHIFT) & 0b11;
    !ap2 && ap10 != 0
}

/// True if a privileged instruction fetch from this small-page descriptor is allowed.
pub fn priv_may_execute(desc: u32) -> bool {
    if (desc & 0b11) != DESC_SMALL_PAGE && (desc & 0b11) != (DESC_SMALL_PAGE | XN_SMALL) {
        return false;
    }
    (desc & XN_SMALL) == 0
}

impl KernelLayout {
    /// True when every window boundary the 4 KiB mapping cares about is
    /// page-aligned, so no page carries bytes from two permission classes.
    /// `linker_iso.ld` guarantees this; `page_kind` stays correct without it,
    /// but only by widening a mixed page's permissions.
    pub fn boundaries_page_aligned(&self) -> bool {
        let a = |v: u32| v % PAGE_SIZE == 0;
        a(self.rodata_start) && a(self.rodata_end) && a(self.data_start)
    }
}

pub fn fill_identity_manager_l1(l1: &mut [u32], ddr_high: u32) {
    let last = ddr_high / SECTION_SIZE;
    for i in 0..L1_ENTRIES {
        let pa = (i as u32) * SECTION_SIZE;
        l1[i] = if (i as u32) <= last {
            section_desc(
                pa,
                Access::PrivRwUserRw,
                MemType::NormalShareableWbwa,
                false,
                false,
            )
        } else {
            device_section(pa)
        };
    }
}

/// Classify one 4 KiB page by what it *overlaps*, not by where it starts.
///
/// Linker section boundaries are byte-aligned, so a single page can hold the
/// tail of `.rodata` and the head of `.data`. Classifying such a page from its
/// base address alone maps live `.data` privileged-read-only, and the first
/// ordinary kernel write into it takes a permission abort (FSR `0x80f`) — the
/// boot-time fault seen on `BOOT_MMU_ISOLATION_20260815`, where
/// `_rodata_end = 0x002e51f8` and `_data_start = 0x002e5200` share page
/// `0x002e5000`.
///
/// A page holding any writable byte must be writable, so `Rw` is tested first.
/// `linker_iso.ld` page-aligns `.rodata` and `.data` so the windows are in fact
/// disjoint per page and no page has to trade W^X against a spurious abort;
/// this ordering is the safety net if that alignment is ever lost.
fn page_kind(layout: &KernelLayout, va: u32) -> KernelPage {
    if va < 0x0010_0000 {
        return KernelPage::Unmapped;
    }
    if va >= 0x2000_0000 || va > layout.ddr_high {
        return KernelPage::Device;
    }
    if va >= layout.heap_end {
        return KernelPage::Pmm;
    }
    let page_end = va.saturating_add(PAGE_SIZE);
    let overlaps = |lo: u32, hi: u32| lo < page_end && va < hi;
    if overlaps(layout.data_start, layout.heap_end) {
        return KernelPage::Rw;
    }
    let vec_page = layout.vectors & !(PAGE_SIZE - 1);
    if overlaps(vec_page, layout.text_end) {
        return KernelPage::Text;
    }
    if overlaps(layout.rodata_start, layout.rodata_end) {
        return KernelPage::Ro;
    }
    KernelPage::Rw
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum KernelPage {
    Unmapped,
    Text,
    Ro,
    Rw,
    Pmm,
    Device,
}

/// Fill CPU0 isolation L1: L2 for kernel image 1 MiB windows, sections for
/// the rest of DDR (privileged RW+XN) and Device MMIO. User pages are not mapped.
pub fn fill_kernel_isolation_l1(
    l1: &mut [u32],
    l2_pool: &mut [[u32; L2_ENTRIES]],
    l2_phys_base: u32,
    layout: &KernelLayout,
    map_pmm: bool,
) -> Result<usize, MapError> {
    if l1.len() != L1_ENTRIES {
        return Err(MapError::OutOfRange);
    }
    for e in l1.iter_mut() {
        *e = 0;
    }
    let mut l2_used = 0usize;
    let kernel_last = (layout.heap_end.saturating_sub(1)) / SECTION_SIZE;
    let first = 0x0010_0000 / SECTION_SIZE;
    for sec in first..=kernel_last {
        if l2_used >= l2_pool.len() {
            return Err(MapError::NoL2);
        }
        let l2_phys = l2_phys_base + (l2_used as u32) * (L2_BYTES as u32);
        l2_pool[l2_used] = [0; L2_ENTRIES];
        l1[sec as usize] = l1_page_table(l2_phys, false);
        for p in 0..L2_ENTRIES {
            let va = sec * SECTION_SIZE + (p as u32) * PAGE_SIZE;
            l2_pool[l2_used][p] = match page_kind(layout, va) {
                KernelPage::Text => kernel_text_page(va),
                KernelPage::Ro => kernel_ro_page(va),
                KernelPage::Rw => kernel_rw_page(va),
                KernelPage::Pmm => {
                    if map_pmm {
                        kernel_rw_page(va)
                    } else {
                        0
                    }
                }
                KernelPage::Device => {
                    small_page(va, Access::PrivRwUserNone, MemType::DeviceShareable, true)
                }
                KernelPage::Unmapped => 0,
            };
        }
        l2_used += 1;
    }
    let ddr_last = layout.ddr_high / SECTION_SIZE;
    for sec in (kernel_last + 1)..=ddr_last {
        let pa = sec * SECTION_SIZE;
        l1[sec as usize] = if map_pmm {
            kernel_rw_section(pa)
        } else {
            0
        };
    }
    for sec in (ddr_last + 1)..(L1_ENTRIES as u32) {
        l1[sec as usize] = device_section(sec * SECTION_SIZE);
    }
    Ok(l2_used)
}

pub fn is_user_executable(desc: u32) -> bool {
    if desc == 0 {
        return false;
    }
    let ty = desc & 0b11;
    if ty == DESC_SMALL_PAGE || ty == (DESC_SMALL_PAGE | XN_SMALL) {
        let xn = (desc & XN_SMALL) != 0;
        let ap10 = (desc >> AP10_SMALL_SHIFT) & 0b11;
        let ap2 = (desc & AP2_SMALL) != 0;
        !xn && (ap10 == 0b10 || ap10 == 0b11 || (ap2 && ap10 != 0b01 && ap10 != 0))
    } else {
        false
    }
}

pub fn user_may_read(desc: u32) -> bool {
    if desc == 0 {
        return false;
    }
    if (desc & 0b11) == DESC_SMALL_PAGE || (desc & 0b11) == (DESC_SMALL_PAGE | XN_SMALL) {
        let ap10 = (desc >> AP10_SMALL_SHIFT) & 0b11;
        ap10 == 0b10 || ap10 == 0b11
    } else {
        false
    }
}

pub fn user_may_write(desc: u32) -> bool {
    if desc == 0 {
        return false;
    }
    if (desc & 0b11) == DESC_SMALL_PAGE || (desc & 0b11) == (DESC_SMALL_PAGE | XN_SMALL) {
        let ap2 = (desc & AP2_SMALL) != 0;
        let ap10 = (desc >> AP10_SMALL_SHIFT) & 0b11;
        !ap2 && ap10 == 0b11
    } else {
        false
    }
}
