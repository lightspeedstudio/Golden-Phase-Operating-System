//! Host tests for ARMv7 short-descriptor isolation. Not Cortex-A9 proof.

use kairos::arch::armv7::mmu::{
    abort_disposition, add_len, device_section, fault_class, fill_kernel_isolation_l1,
    kernel_rw_section, kernel_text_page, map_pages, ranges_overlap, saved_kernel_sp_ok,
    user_may_read, user_may_write, user_rx_page, user_rw_page, Access, AbortDisposition,
    FaultClass, MapError, MemType, CPSR_MODE_USR, DACR_CLIENT_D0, DACR_MANAGER_ALL,
    align_up_l1, is_user_executable, l1_base_ok, priv_may_execute, priv_may_write, KERNEL_FRAME_BYTES,
    L1_ALLOC_PAGES, L1_ENTRIES, L2_BYTES, L2_ENTRIES,
    ISO_20260815_ABORT_FAR, MEASURED_ISO_20260815, MEASURED_UTILITY_8AE130C, PAGE_SIZE,
    SECTION_SIZE, SVC_STACK_BYTES,
};
use kairos::arch::armv7::payloads;
use kairos::kernel::aspace::{
    install_user_window, next_asid, unmap_user_window, AddressSpace, Perm, Range,
};

fn dummy_hash() -> [u8; 32] {
    [0x11; 32]
}

#[test]
fn dacr_client_not_manager() {
    assert_eq!(DACR_CLIENT_D0, 1);
    assert_ne!(DACR_CLIENT_D0, DACR_MANAGER_ALL);
}

#[test]
fn kernel_text_not_user() {
    let d = kernel_text_page(0x0010_0000);
    assert!(!user_may_write(d));
    assert!(!user_may_read(d));
}

#[test]
fn user_rx_not_writable() {
    let d = user_rx_page(0x0140_0000);
    assert!(user_may_read(d));
    assert!(!user_may_write(d));
}

#[test]
fn user_rw_is_xn_bit() {
    let d = user_rw_page(0x0140_1000);
    assert!(user_may_write(d));
    assert_eq!(d & 1, 1);
}

#[test]
fn device_is_privileged() {
    let d = device_section(0xE000_0000);
    assert!(!user_may_read(d));
    assert!(!user_may_write(d));
}

#[test]
fn range_overflow() {
    assert!(add_len(u32::MAX, 1).is_none());
    assert!(add_len(0x10, 4).is_some());
}

#[test]
fn overlap_detect() {
    assert!(ranges_overlap(0x1000, 0x1000, 0x1800, 0x1000));
    assert!(!ranges_overlap(0x1000, 0x1000, 0x2000, 0x1000));
}

#[test]
fn map_pages_rejects_cross_section() {
    let mut l1 = [0u32; L1_ENTRIES];
    let mut l2 = [0u32; L2_ENTRIES];
    let err = map_pages(
        &mut l1,
        &mut l2,
        0x0130_A000,
        0x001F_F000,
        0x001F_F000,
        PAGE_SIZE * 2,
        user_rw_page(0x001F_F000),
        true,
    );
    assert_eq!(err, Err(MapError::CrossesSection));
    assert_eq!(l1[section_of(0x001F_F000)], 0);
}

fn section_of(va: u32) -> usize {
    (va / SECTION_SIZE) as usize
}

#[test]
fn guard_page_invalid() {
    let mut l1 = [0u32; L1_ENTRIES];
    let mut l2 = [0u32; L2_ENTRIES];
    let base = 0x0140_0000;
    let space = AddressSpace {
        process_id: 1,
        package_id: dummy_hash(),
        payload_root: dummy_hash(),
        translation_table_root: 0x0131_0000,
        l2_phys: 0x0131_4000,
        code: Range {
            base,
            len: PAGE_SIZE,
        },
        data: Range {
            base: base + PAGE_SIZE,
            len: PAGE_SIZE,
        },
        stack: Range {
            base: base + PAGE_SIZE * 3,
            len: PAGE_SIZE,
        },
        guard: Range {
            base: base + PAGE_SIZE * 2,
            len: PAGE_SIZE,
        },
        asid: 1,
        generation: 1,
    };
    install_user_window(&mut l1, &mut l2, space.l2_phys, &space).unwrap();
    let gidx = ((space.guard.base / PAGE_SIZE) % (L2_ENTRIES as u32)) as usize;
    assert_eq!(l2[gidx], 0);
    assert!(space.range_valid(space.data.base, 4, Perm::Read));
    assert!(!space.range_valid(space.guard.base, 4, Perm::Read));
    assert!(!space.range_valid(0, 4, Perm::Read));
    assert!(!space.range_valid(0x0010_0000, 4, Perm::Read));
    assert!(!space.range_valid(0xE000_0000, 4, Perm::Write));
    assert!(!space.range_valid(space.code.base, 4, Perm::Write));
}

#[test]
fn unmap_user_window_invalidates_ttbr_slots() {
    let mut l1 = [0u32; L1_ENTRIES];
    let mut l2 = [0u32; L2_ENTRIES];
    let base = 0x0140_0000;
    let space = AddressSpace {
        process_id: 1,
        package_id: dummy_hash(),
        payload_root: dummy_hash(),
        translation_table_root: 0x0131_0000,
        l2_phys: 0x0131_4000,
        code: Range {
            base,
            len: PAGE_SIZE,
        },
        data: Range {
            base: base + PAGE_SIZE,
            len: PAGE_SIZE,
        },
        stack: Range {
            base: base + PAGE_SIZE * 3,
            len: PAGE_SIZE,
        },
        guard: Range {
            base: base + PAGE_SIZE * 2,
            len: PAGE_SIZE,
        },
        asid: 1,
        generation: 1,
    };
    install_user_window(&mut l1, &mut l2, space.l2_phys, &space).unwrap();
    let cidx = ((space.code.base / PAGE_SIZE) % (L2_ENTRIES as u32)) as usize;
    let didx = ((space.data.base / PAGE_SIZE) % (L2_ENTRIES as u32)) as usize;
    let sidx = ((space.stack.base / PAGE_SIZE) % (L2_ENTRIES as u32)) as usize;
    let sec = (space.code.base / SECTION_SIZE) as usize;
    assert!(user_may_read(l2[cidx]));
    assert!(!user_may_write(l2[cidx]));
    assert!(is_user_executable(l2[cidx]));
    assert!(user_may_write(l2[didx]));
    assert_ne!(l1[sec], 0);
    unmap_user_window(&mut l1, &mut l2, &space);
    assert_eq!(l2[cidx], 0);
    assert_eq!(l2[didx], 0);
    assert_eq!(l2[sidx], 0);
    assert_eq!(l1[sec], 0);
    assert!(!user_may_read(l2[cidx]));
    assert!(!user_may_write(l2[didx]));
    assert!(!is_user_executable(l2[cidx]));
    unmap_user_window(&mut l1, &mut l2, &space);
    assert_eq!(l1[sec], 0);
}

#[test]
fn asid_lifecycle() {
    assert_eq!(next_asid(1), 2);
    assert_eq!(next_asid(255), 1);
}

#[test]
fn fault_classes() {
    assert_eq!(fault_class(0x5), FaultClass::Translation);
    assert_eq!(fault_class(0xD), FaultClass::Permission);
    assert_eq!(fault_class(0x1), FaultClass::Alignment);
}

#[test]
fn kernel_l1_uses_l2_for_mixed_section() {
    let mut l1 = [0u32; L1_ENTRIES];
    let mut pool = [[0u32; L2_ENTRIES]; 32];
    let n = fill_kernel_isolation_l1(
        &mut l1,
        &mut pool,
        0x0031_0000,
        &MEASURED_UTILITY_8AE130C,
        true,
    )
    .unwrap();
    assert!(n >= 18);
    let sec2 = l1[0x0020_0000 / SECTION_SIZE as usize];
    assert_eq!(sec2 & 0b11, 0b01);
    let mmio = l1[0xE000_0000 / SECTION_SIZE as usize];
    assert_ne!(mmio, 0);
    let pmm_sec = l1[0x0140_0000 / SECTION_SIZE as usize];
    assert_eq!(pmm_sec, kernel_rw_section(0x0140_0000));
    assert_eq!(pmm_sec & 0b11, 0b10);
    let vec_page = MEASURED_UTILITY_8AE130C.vectors & !(PAGE_SIZE - 1);
    let l2_0 = &pool[0];
    let vidx = ((vec_page / PAGE_SIZE) % (L2_ENTRIES as u32)) as usize;
    assert_eq!(l2_0[vidx], kernel_text_page(vec_page));
    assert_eq!(mmio & 0b11, 0b10);
}

/// Resolve the small-page descriptor a filled isolation table gives `va`.
/// Follows the L1 entry to its L2 rather than assuming a pool ordering.
fn page_desc(
    l1: &[u32; L1_ENTRIES],
    pool: &[[u32; L2_ENTRIES]],
    l2_phys_base: u32,
    va: u32,
) -> u32 {
    let entry = l1[(va / SECTION_SIZE) as usize];
    assert_eq!(entry & 0b11, 0b01, "va {va:#010x} is not a page-table section");
    let slot = ((entry & 0xFFFF_FC00) - l2_phys_base) / (L2_BYTES as u32);
    pool[slot as usize][((va / PAGE_SIZE) % (L2_ENTRIES as u32)) as usize]
}

fn fill_iso(
    layout: &kairos::arch::armv7::mmu::KernelLayout,
) -> ([u32; L1_ENTRIES], Box<[[u32; L2_ENTRIES]; 32]>, u32) {
    let base = 0x0031_1000u32;
    let mut l1 = [0u32; L1_ENTRIES];
    let mut pool = Box::new([[0u32; L2_ENTRIES]; 32]);
    fill_kernel_isolation_l1(&mut l1, &mut pool[..], base, layout, true).unwrap();
    (l1, pool, base)
}

/// Regression for the boot-time abort on `BOOT_MMU_ISOLATION_20260815`:
/// `.data` began at `0x002e5200`, inside the page that also held the tail of
/// `.rodata`, so the page was mapped privileged-read-only and the first kernel
/// write to `0x002e5f90` took `class=PERMISSION fsr=0000080f`.
#[test]
fn shared_rodata_data_page_is_writable() {
    let (l1, pool, base) = fill_iso(&MEASURED_ISO_20260815);
    let desc = page_desc(&l1, &pool[..], base, ISO_20260815_ABORT_FAR);
    assert!(
        priv_may_write(desc),
        "kernel .data at {ISO_20260815_ABORT_FAR:#010x} mapped read-only (desc {desc:#010x})"
    );
}

/// Every byte of `.data`/`.bss` must be privileged-writable and never executable.
#[test]
fn kernel_data_is_writable_and_xn() {
    let layout = &MEASURED_ISO_20260815;
    let (l1, pool, base) = fill_iso(layout);
    let mut va = layout.data_start & !(PAGE_SIZE - 1);
    while va < layout.bss_end {
        let desc = page_desc(&l1, &pool[..], base, va);
        assert!(priv_may_write(desc), "data page {va:#010x} not writable");
        assert!(!priv_may_execute(desc), "data page {va:#010x} is executable");
        va += PAGE_SIZE;
    }
}

/// W^X: no kernel page is both privileged-writable and executable.
#[test]
fn kernel_pages_never_writable_and_executable() {
    for layout in [&MEASURED_ISO_20260815, &MEASURED_UTILITY_8AE130C] {
        let (l1, pool, base) = fill_iso(layout);
        let mut va = 0x0010_0000u32;
        while va < layout.heap_end {
            let desc = page_desc(&l1, &pool[..], base, va);
            assert!(
                !(priv_may_write(desc) && priv_may_execute(desc)),
                "page {va:#010x} is W+X (desc {desc:#010x})"
            );
            va += PAGE_SIZE;
        }
    }
}

/// Kernel text stays executable and read-only, including the last text page,
/// which shares its page with `.rodata` padding in unaligned layouts.
#[test]
fn kernel_text_is_ro_and_executable() {
    let layout = &MEASURED_ISO_20260815;
    let (l1, pool, base) = fill_iso(layout);
    for va in [
        layout.vectors & !(PAGE_SIZE - 1),
        0x0020_0000,
        (layout.text_end - 1) & !(PAGE_SIZE - 1),
    ] {
        let desc = page_desc(&l1, &pool[..], base, va);
        assert!(priv_may_execute(desc), "text page {va:#010x} not executable");
        assert!(!priv_may_write(desc), "text page {va:#010x} is writable");
    }
}

/// The abort handler runs on `_abort_stack_top`; if that stack were read-only
/// the first abort would fault again and escalate to the nested-abort reset.
#[test]
fn abort_and_svc_stacks_are_writable() {
    let layout = &MEASURED_ISO_20260815;
    let (l1, pool, base) = fill_iso(layout);
    for top in [layout.stack_top, layout.irq_stack_top, layout.abort_stack_top] {
        for off in [PAGE_SIZE, 0x1000 * 2] {
            let va = (top - off) & !(PAGE_SIZE - 1);
            let desc = page_desc(&l1, &pool[..], base, va);
            assert!(priv_may_write(desc), "stack page {va:#010x} not writable");
        }
    }
}

/// The L1/L2 tables themselves must stay writable — `install_user_window`
/// writes a per-app L1 copy while isolation is already on.
#[test]
fn page_tables_are_writable() {
    let layout = &MEASURED_ISO_20260815;
    let (l1, pool, base) = fill_iso(layout);
    let desc = page_desc(&l1, &pool[..], base, layout.mmu_l1);
    assert!(priv_may_write(desc), "L1 table page not writable");
}

/// The shipped ELF that faulted was *not* page-aligned; that is the condition
/// `linker_iso.ld` now removes and `mmuiso status` reports live from the board.
#[test]
fn faulting_image_layout_was_unaligned() {
    assert!(!MEASURED_ISO_20260815.boundaries_page_aligned());
    let aligned = kairos::arch::armv7::mmu::KernelLayout {
        rodata_start: 0x002B_4000,
        rodata_end: 0x002E_6000,
        data_start: 0x002E_6000,
        ..MEASURED_ISO_20260815
    };
    assert!(aligned.boundaries_page_aligned());
}

/// The live PMM base is `__heap_end` = 0x01321000, which is 4 KiB but NOT
/// 16 KiB aligned, and the buddy allocator hands back `base + k * 16384`. So a
/// plain `alloc(4)` can never produce a legal TTBR0 base -- with `TTBCR.N == 0`
/// the low bits are consumed as attributes and the MMU walks a different 16 KiB
/// block. That is what hung `mmuiso matrix` on hardware.
#[test]
fn plain_alloc4_is_never_a_legal_l1_base() {
    let pmm_base: u32 = 0x0132_1000;
    assert_eq!(pmm_base % PAGE_SIZE, 0);
    assert_ne!(pmm_base % 0x4000, 0, "fixture must be the misaligned base");
    for k in 0..64u32 {
        let addr = pmm_base + k * 4 * PAGE_SIZE;
        assert!(!l1_base_ok(addr), "alloc(4) result {addr:#010x} looked legal");
    }
}

/// Over-allocating and rounding up always yields a legal, in-bounds L1 base.
#[test]
fn over_allocated_l1_is_always_alignable() {
    for base in [0x0132_1000u32, 0x0132_0000, 0x0132_2000, 0x0132_3000] {
        for k in 0..64u32 {
            let alloc = base + k * L1_ALLOC_PAGES * PAGE_SIZE;
            let l1 = align_up_l1(alloc);
            assert!(l1_base_ok(l1), "{l1:#010x} not a legal L1 base");
            assert!(l1 >= alloc, "aligned base below the allocation");
            let alloc_end = alloc + L1_ALLOC_PAGES * PAGE_SIZE;
            assert!(
                l1 + (L1_ENTRIES as u32) * 4 <= alloc_end,
                "16 KiB table at {l1:#010x} escapes its {}-page allocation",
                L1_ALLOC_PAGES
            );
        }
    }
}

#[test]
fn align_up_l1_is_idempotent_on_aligned_input() {
    assert_eq!(align_up_l1(0x0030_4000), 0x0030_4000);
    assert_eq!(align_up_l1(0x0030_4001), 0x0030_8000);
    assert_eq!(align_up_l1(0x0132_1000), 0x0132_4000);
    assert!(!l1_base_ok(0));
}

#[test]
fn cpu1_l1_omits_pmm() {
    let mut l1 = [0u32; L1_ENTRIES];
    let mut pool = [[0u32; L2_ENTRIES]; 32];
    fill_kernel_isolation_l1(
        &mut l1,
        &mut pool,
        0x0031_8000,
        &MEASURED_UTILITY_8AE130C,
        false,
    )
    .unwrap();
    assert_eq!(l1[0x0140_0000 / SECTION_SIZE as usize], 0);
}

#[test]
fn payloads_are_a32_not_elf() {
    let c = payloads::legal_read();
    assert_ne!(&c[..4], b"\x7fELF");
    assert_eq!(c[3], 0xE5);
}

fn word(code: &[u8; 64], i: usize) -> u32 {
    u32::from_le_bytes([code[i * 4], code[i * 4 + 1], code[i * 4 + 2], code[i * 4 + 3]])
}

/// Goal 5 probes. Both find their own code page with `sub r0, pc, #8`, so the
/// idiom is pinned here -- if it drifts, the payloads would probe the wrong
/// page and still "pass" for the wrong reason.
#[test]
fn code_payloads_are_a32_and_self_addressing() {
    for c in [payloads::code_write(), payloads::code_read()] {
        assert_ne!(&c[..4], b"\x7fELF");
        assert_eq!(word(&c, 0), 0xE24F_0008, "expected sub r0, pc, #8");
    }
    // code_write must store; code_read must load.
    assert_eq!(word(&payloads::code_write(), 2), 0xE580_1000);
    assert_eq!(word(&payloads::code_read(), 1), 0xE590_1000);
}

/// The code page is user-readable and user-non-writable, which is exactly what
/// the two goal-5 payloads assert on hardware. Ties payload expectations to the
/// mapping so they cannot drift apart.
#[test]
fn user_code_page_is_readable_not_writable() {
    let d = user_rx_page(0x1FFF_1000);
    assert!(user_may_read(d), "code-read payload would fault");
    assert!(!user_may_write(d), "code-write payload would not fault");
}

#[test]
fn matrix_covers_both_controls_and_all_probes() {
    let m = payloads::matrix();
    assert_eq!(m.len(), 10);
    let names: Vec<&str> = m.iter().map(|p| p.name).collect();
    for expected in [
        "legal-read",
        "code-read",
        "kernel-read",
        "kernel-write",
        "peer-read",
        "peer-write",
        "mmio-write",
        "code-write",
        "execute-data",
        "stack-overflow",
    ] {
        assert!(names.contains(&expected), "missing probe {expected}");
    }
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len(), "duplicate probe name");

    let positives: Vec<&str> = m
        .iter()
        .filter(|p| !p.expect_fault)
        .map(|p| p.name)
        .collect();
    assert_eq!(
        positives,
        vec!["legal-read", "code-read"],
        "positive controls must be exactly legal-read and code-read"
    );
    assert_eq!(m.iter().filter(|p| p.expect_fault).count(), 8);
}

#[test]
fn pointer_crossing_guard_rejected() {
    let space = AddressSpace {
        process_id: 2,
        package_id: dummy_hash(),
        payload_root: dummy_hash(),
        translation_table_root: 0,
        l2_phys: 0,
        code: Range {
            base: 0x14000000,
            len: PAGE_SIZE,
        },
        data: Range {
            base: 0x14001000,
            len: PAGE_SIZE,
        },
        stack: Range {
            base: 0x14003000,
            len: PAGE_SIZE,
        },
        guard: Range {
            base: 0x14002000,
            len: PAGE_SIZE,
        },
        asid: 3,
        generation: 9,
    };
    assert!(!space.range_valid(0x14001FFC, 8, Perm::Read));
}

#[test]
fn named_access_enum_exists() {
    let _ = Access::PrivRwUserNone;
    let _ = MemType::DeviceShareable;
}

#[test]
fn c10_sp_zero_is_watchdog() {
    let top = 0x0021_0000;
    assert!(!saved_kernel_sp_ok(0, top, SVC_STACK_BYTES));
    assert_eq!(
        abort_disposition(1, 0, top, SVC_STACK_BYTES, CPSR_MODE_USR),
        AbortDisposition::HaltIsolated
    );
}

#[test]
fn c10_sp_misaligned_is_watchdog() {
    let top = 0x0021_0000;
    assert!(!saved_kernel_sp_ok(1, top, SVC_STACK_BYTES));
    assert_eq!(
        abort_disposition(1, top - KERNEL_FRAME_BYTES + 1, top, SVC_STACK_BYTES, CPSR_MODE_USR),
        AbortDisposition::HaltIsolated
    );
}

#[test]
fn c10_sp_outside_stack_is_watchdog() {
    let top = 0x0021_0000;
    let below = top - SVC_STACK_BYTES - 4;
    assert!(!saved_kernel_sp_ok(below, top, SVC_STACK_BYTES));
    let too_high = top - 4;
    assert!(!saved_kernel_sp_ok(too_high, top, SVC_STACK_BYTES));
}

#[test]
fn c10_nested_abort_is_watchdog() {
    let top = 0x0021_0000;
    let sp = top - KERNEL_FRAME_BYTES;
    assert!(saved_kernel_sp_ok(sp, top, SVC_STACK_BYTES));
    assert_eq!(
        abort_disposition(2, sp, top, SVC_STACK_BYTES, CPSR_MODE_USR),
        AbortDisposition::WatchdogReset
    );
}

#[test]
fn c10_kernel_mode_abort_is_watchdog() {
    let top = 0x0021_0000;
    let sp = top - KERNEL_FRAME_BYTES;
    let spsr_svc = 0x13;
    assert_eq!(
        abort_disposition(1, sp, top, SVC_STACK_BYTES, spsr_svc),
        AbortDisposition::HaltIsolated
    );
}

#[test]
fn c10_user_abort_valid_frame_returns() {
    let top = 0x0021_0000;
    let sp = top - KERNEL_FRAME_BYTES;
    assert_eq!(
        abort_disposition(1, sp, top, SVC_STACK_BYTES, CPSR_MODE_USR),
        AbortDisposition::ReturnToKernel
    );
}
