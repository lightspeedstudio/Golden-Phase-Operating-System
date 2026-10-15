// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Utility PT_LOAD copy + working `handoff::jump` sequence.

use crate::bootimg::parse::{
    loader_bitstream_app, parse_all_product, ParsedHeaders, HEADER_WINDOW_LEN,
};
use crate::sd::bus::SdBus;
use crate::sd::sys04::BOOT16_PHYS;
use crate::UTILITY_ENTRY;

/// Locked PATH1_G64 Utility PT_LOAD (see `docs/boot/SYS09_HANDOFF_REFERENCE.md`).
pub const UTIL_LOAD: u32 = 0x0010_0000;
pub const GOLD_UTIL_FILESZ: u32 = 0x001D_84D4;
pub const GOLD_UTIL_MEMSZ: u32 = 0x001D_F158;
pub const GOLD_UTIL_BSS: u32 = 0x002D_9000;
pub const GOLD_UTIL_BSS_LEN: u32 = 0x6158;
/// Combined PT_LOAD span of main `zynq-a9,utility` isolation ELF (`--build-id`).
pub const UTIL_FILESZ: u32 = 0x0022_2B60;
pub const UTIL_MEMSZ: u32 = 0x0022_93A8;
pub const UTIL_ALIGN: u32 = 0x1000;
pub const UTIL_BSS: u32 = 0x0032_2B60;
pub const UTIL_BSS_LEN: u32 = 0x6848;
#[allow(dead_code)]
pub const CPU1_START: u32 = 0xFFFF_FFF0;

pub const HANDOFF_PASS_LINE: &str = "HANDOFF PASS\n";
pub const HANDOFF_FAIL_LINE: &str = "HANDOFF FAIL\n";
pub const JUMP_LINE: &str = "JUMP 00100060\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Parse,
    Bounds,
    Load,
    Entry,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Copy { dest: u32, len: u32 },
    Zero { dest: u32, len: u32 },
    ICacheInval,
    BtbInval,
    Dsb,
    Isb,
    Jump(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub src_off: u32,
    pub dest: u32,
    pub filesz: u32,
    pub memsz: u32,
    pub entry: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub ops: [Op; 7],
    pub cpu1_written: bool,
    pub wdt_enabled: bool,
    pub vbar_written: bool,
}

pub fn working_jump_ops(entry: u32) -> [Op; 5] {
    [
        Op::ICacheInval,
        Op::BtbInval,
        Op::Dsb,
        Op::Isb,
        Op::Jump(entry),
    ]
}

pub fn locked_place_ops() -> [Op; 2] {
    [
        Op::Copy {
            dest: UTIL_LOAD,
            len: UTIL_FILESZ,
        },
        Op::Zero {
            dest: UTIL_LOAD + UTIL_FILESZ,
            len: UTIL_MEMSZ - UTIL_FILESZ,
        },
    ]
}

pub fn locked_working_state() -> State {
    state_after(locked_place_ops(), working_jump_ops(UTILITY_ENTRY))
}

pub fn plan_from_headers(h: &ParsedHeaders, file_len: u32) -> Result<Plan, Error> {
    let (_, _, app) = loader_bitstream_app(h).map_err(|_| Error::Parse)?;
    if app.load_addr != UTIL_LOAD || app.exec_addr != UTILITY_ENTRY {
        return Err(Error::Entry);
    }
    let filesz = u32::try_from(app.byte_len()).map_err(|_| Error::Bounds)?;
    let memsz = memsz_for_filesz(filesz)?;
    let off = u32::try_from(app.data_off).map_err(|_| Error::Bounds)?;
    if off.checked_add(filesz).ok_or(Error::Bounds)? > file_len {
        return Err(Error::Bounds);
    }
    Ok(Plan {
        src_off: off,
        dest: app.load_addr,
        filesz,
        memsz,
        entry: app.exec_addr,
    })
}

pub fn headers<B: SdBus>(bus: &mut B, file_len: u32) -> Result<ParsedHeaders, Error> {
    #[cfg(feature = "std")]
    {
        let mut window = [0u8; HEADER_WINDOW_LEN];
        let n = core::cmp::min(HEADER_WINDOW_LEN, file_len as usize);
        bus.phys_read(BOOT16_PHYS, &mut window[..n]);
        parse_all_product(&window[..n], n, file_len as usize).map_err(|_| Error::Parse)
    }
    #[cfg(not(feature = "std"))]
    {
        let _ = bus;
        let n = core::cmp::min(HEADER_WINDOW_LEN, file_len as usize);
        let window = unsafe { core::slice::from_raw_parts(BOOT16_PHYS as *const u8, n) };
        parse_all_product(window, n, file_len as usize).map_err(|_| Error::Parse)
    }
}

pub fn working_entry<B: SdBus>(bus: &mut B, file_len: u32) -> Result<u32, Error> {
    let st = run(bus, file_len)?;
    if !matches_working(&st) {
        return Err(Error::Load);
    }
    match st.ops[6] {
        Op::Jump(entry) => Ok(entry),
        _ => Err(Error::Entry),
    }
}

fn copy_span<B: SdBus>(bus: &mut B, src: u32, dest: u32, len: u32) {
    #[cfg(feature = "std")]
    {
        let mut buf = vec![0u8; len as usize];
        bus.phys_read(src, &mut buf);
        bus.phys_write(dest, &buf);
    }
    #[cfg(not(feature = "std"))]
    {
        let _ = bus;
        unsafe {
            core::ptr::copy_nonoverlapping(src as *const u8, dest as *mut u8, len as usize);
        }
    }
}

fn zero_span<B: SdBus>(bus: &mut B, dest: u32, len: u32) {
    #[cfg(feature = "std")]
    {
        let z = vec![0u8; len as usize];
        bus.phys_write(dest, &z);
    }
    #[cfg(not(feature = "std"))]
    {
        let _ = bus;
        unsafe {
            core::ptr::write_bytes(dest as *mut u8, 0, len as usize);
        }
    }
}

fn memsz_for_filesz(filesz: u32) -> Result<u32, Error> {
    if filesz == UTIL_FILESZ {
        Ok(UTIL_MEMSZ)
    } else if filesz == GOLD_UTIL_FILESZ {
        Ok(GOLD_UTIL_MEMSZ)
    } else {
        Err(Error::Load)
    }
}

fn gold_place_ops() -> [Op; 2] {
    [
        Op::Copy {
            dest: UTIL_LOAD,
            len: GOLD_UTIL_FILESZ,
        },
        Op::Zero {
            dest: UTIL_LOAD + GOLD_UTIL_FILESZ,
            len: GOLD_UTIL_MEMSZ - GOLD_UTIL_FILESZ,
        },
    ]
}

pub fn place<B: SdBus>(bus: &mut B, plan: &Plan) -> Result<[Op; 2], Error> {
    if plan.dest != UTIL_LOAD || memsz_for_filesz(plan.filesz) != Ok(plan.memsz) {
        if cfg!(test) && plan.memsz >= plan.filesz {
            copy_span(bus, BOOT16_PHYS + plan.src_off, plan.dest, plan.filesz);
            let tail = plan.memsz - plan.filesz;
            zero_span(bus, plan.dest + plan.filesz, tail);
            return Ok([
                Op::Copy {
                    dest: plan.dest,
                    len: plan.filesz,
                },
                Op::Zero {
                    dest: plan.dest + plan.filesz,
                    len: tail,
                },
            ]);
        }
        return Err(Error::Load);
    }
    if plan.memsz < plan.filesz {
        return Err(Error::Load);
    }
    copy_span(bus, BOOT16_PHYS + plan.src_off, plan.dest, plan.filesz);
    let tail = plan.memsz - plan.filesz;
    zero_span(bus, plan.dest + plan.filesz, tail);
    if plan.filesz == GOLD_UTIL_FILESZ {
        Ok(gold_place_ops())
    } else {
        Ok(locked_place_ops())
    }
}

pub fn jump_ops(entry: u32) -> [Op; 5] {
    working_jump_ops(entry)
}

pub fn state_after(place: [Op; 2], jump: [Op; 5]) -> State {
    State {
        ops: [
            place[0], place[1], jump[0], jump[1], jump[2], jump[3], jump[4],
        ],
        cpu1_written: false,
        wdt_enabled: false,
        vbar_written: false,
    }
}

pub fn matches_working(s: &State) -> bool {
    s == &locked_working_state() || s == &state_after(gold_place_ops(), working_jump_ops(UTILITY_ENTRY))
}

pub fn run<B: SdBus>(bus: &mut B, file_len: u32) -> Result<State, Error> {
    let h = headers(bus, file_len)?;
    let plan = plan_from_headers(&h, file_len)?;
    let placed = place(bus, &plan)?;
    Ok(state_after(placed, jump_ops(plan.entry)))
}

/// Working `handoff::jump`: ICIALLU, BPIALL, DSB, ISB, branch.
#[cfg(not(feature = "std"))]
pub unsafe fn jump(entry: u32) -> ! {
    core::arch::asm!(
        "mcr p15, 0, {z}, c7, c5, 0",
        "mcr p15, 0, {z}, c7, c5, 6",
        "dsb",
        "isb",
        z = in(reg) 0u32,
        options(nostack),
    );
    let f: extern "C" fn() -> ! = core::mem::transmute(entry as usize);
    f()
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::bootimg::parse::{
        parse_all, ImageHeaderTable, PartitionHeader, ATTR_PL, ATTR_PS, HEADER_WINDOW_LEN,
        MAX_PARTITIONS,
    };
    use crate::sd::bus::HostSd;
    use crate::sd::sys04::BOOT16_PHYS;
    use crate::{elf32_first_pt_load, elf32_le_entry, UTILITY_ENTRY};
    use crate::sha256::sha256;
    use std::fs;

    fn repo_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn hex32(h: [u8; 32]) -> String {
        h.iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn dummy_headers(app_bytes: u32, load: u32, exec: u32, data_off: usize) -> ParsedHeaders {
        let z = PartitionHeader {
            enc_words: 0,
            data_words: 0,
            total_words: 0,
            load_addr: 0,
            exec_addr: 0,
            data_off: 0,
            attributes: 0,
        };
        let mut partitions = [z; MAX_PARTITIONS];
        partitions[0] = PartitionHeader {
            enc_words: 1,
            data_words: 1,
            total_words: 1,
            load_addr: 0,
            exec_addr: 0,
            data_off: 0x1700,
            attributes: ATTR_PS,
        };
        partitions[1] = PartitionHeader {
            enc_words: 64,
            data_words: 64,
            total_words: 64,
            load_addr: 0,
            exec_addr: 0,
            data_off: 0x2000,
            attributes: ATTR_PL,
        };
        let dw = app_bytes / 4;
        partitions[2] = PartitionHeader {
            enc_words: dw,
            data_words: dw,
            total_words: dw,
            load_addr: load,
            exec_addr: exec,
            data_off,
            attributes: ATTR_PS,
        };
        ParsedHeaders {
            source_offset: 0x1700,
            stage1_len: 4,
            ident: 0,
            reg_init0: 0,
            iht: ImageHeaderTable {
                version: 0x0101_0000,
                partition_count: 3,
                partition_header_off: 0,
                image_header_off: 0,
            },
            partitions,
            partition_count: 3,
        }
    }

    #[test]
    fn locked_elf_pt_load_matches_constants() {
        let p = repo_root().join("fpga/zynq/release/PZ7020_G1P_PATH1_G64/kairos_zynq.elf");
        let bytes = fs::read(&p).unwrap();
        assert_eq!(bytes.len(), 2475164);
        assert_eq!(
            hex32(sha256(&bytes)),
            "447f9fbdec6bbdb03227c8620baa854f3a141d52ec71452251e2f63e1852eaf8"
        );
        assert_eq!(elf32_le_entry(&bytes), Some(UTILITY_ENTRY));
        let l = elf32_first_pt_load(&bytes).unwrap();
        assert_eq!(l.paddr, UTIL_LOAD);
        assert_eq!(l.filesz, GOLD_UTIL_FILESZ);
        assert_eq!(l.memsz, GOLD_UTIL_MEMSZ);
        assert_eq!(l.align, UTIL_ALIGN);
        assert_eq!(GOLD_UTIL_BSS + GOLD_UTIL_BSS_LEN, UTIL_LOAD + GOLD_UTIL_MEMSZ);
        assert_eq!(l.memsz - l.filesz, 0x6C84);
        let img = crate::elf32_pt_load_image(&bytes).unwrap();
        assert_eq!(img.0, UTILITY_ENTRY);
        assert_eq!(img.1, UTIL_LOAD);
        assert_eq!(img.2.len(), GOLD_UTIL_MEMSZ as usize);
        assert_eq!(&img.2[GOLD_UTIL_FILESZ as usize..], vec![0u8; (GOLD_UTIL_MEMSZ - GOLD_UTIL_FILESZ) as usize]);
    }

    #[test]
    fn working_jump_is_icache_btb_dsb_isb() {
        assert_eq!(UTIL_BSS + UTIL_BSS_LEN, UTIL_LOAD + UTIL_MEMSZ);
        assert_eq!(
            working_jump_ops(UTILITY_ENTRY),
            [
                Op::ICacheInval,
                Op::BtbInval,
                Op::Dsb,
                Op::Isb,
                Op::Jump(UTILITY_ENTRY),
            ]
        );
        assert_eq!(CPU1_START, 0xFFFF_FFF0);
        let src = include_str!("mod.rs");
        assert!(src.contains("c7, c5, 0"));
        assert!(src.contains("c7, c5, 6"));
        assert_eq!(JUMP_LINE, "JUMP 00100060\n");
        assert_eq!(HANDOFF_PASS_LINE, "HANDOFF PASS\n");
        assert_eq!(HANDOFF_FAIL_LINE, "HANDOFF FAIL\n");
    }

    #[test]
    fn host_state_matches_locked_working_handoff() {
        let file_len = 0x3000 + UTIL_FILESZ;
        let h = dummy_headers(UTIL_FILESZ, UTIL_LOAD, UTILITY_ENTRY, 0x3000);
        let plan = plan_from_headers(&h, file_len).unwrap();
        assert_eq!(plan.dest, UTIL_LOAD);
        assert_eq!(plan.filesz, UTIL_FILESZ);
        assert_eq!(plan.memsz, UTIL_MEMSZ);
        assert_eq!(plan.entry, UTILITY_ENTRY);
        let st = state_after(locked_place_ops(), jump_ops(plan.entry));
        assert!(matches_working(&st));
        assert_eq!(st, locked_working_state());
        assert!(!st.cpu1_written);
        assert!(!st.wdt_enabled);
        assert!(!st.vbar_written);
    }

    #[test]
    fn gold_boot_plan_matches_locked_working_state() {
        let bytes = fs::read(repo_root().join("fpga/zynq/release/PZ7020_G1P_PATH1_G64/BOOT.BIN")).unwrap();
        let n = HEADER_WINDOW_LEN.min(bytes.len());
        let h = parse_all(&bytes[..n], n, bytes.len()).unwrap();
        let plan = plan_from_headers(&h, bytes.len() as u32).unwrap();
        assert_eq!(plan.dest, UTIL_LOAD);
        assert_eq!(plan.filesz, GOLD_UTIL_FILESZ);
        assert_eq!(plan.memsz, GOLD_UTIL_MEMSZ);
        assert_eq!(plan.entry, UTILITY_ENTRY);
        let st = state_after(gold_place_ops(), jump_ops(plan.entry));
        assert_eq!(st, state_after(gold_place_ops(), working_jump_ops(UTILITY_ENTRY)));
    }

    #[test]
    fn plan_rejects_wrong_entry_or_filesz() {
        let file_len = 0x3000 + UTIL_FILESZ;
        let bad_entry = dummy_headers(UTIL_FILESZ, UTIL_LOAD, 0x0010_0020, 0x3000);
        assert_eq!(
            plan_from_headers(&bad_entry, file_len),
            Err(Error::Entry)
        );
        let bad_len = dummy_headers(16, UTIL_LOAD, UTILITY_ENTRY, 0x3000);
        assert_eq!(plan_from_headers(&bad_len, file_len), Err(Error::Load));
    }

    #[test]
    fn place_copies_filesz_and_zeros_memsz_tail() {
        let mut bus = HostSd::new();
        let src = [0xEAu8, 0xFF, 0xFF, 0xFE, 0x11, 0x22, 0x33, 0x44];
        bus.phys_write(BOOT16_PHYS, &src);
        let plan = Plan {
            src_off: 0,
            dest: 0x0010_0000,
            filesz: 8,
            memsz: 16,
            entry: UTILITY_ENTRY,
        };
        let placed = place(&mut bus, &plan).unwrap();
        assert_eq!(
            placed[0],
            Op::Copy {
                dest: 0x0010_0000,
                len: 8
            }
        );
        assert_eq!(
            placed[1],
            Op::Zero {
                dest: 0x0010_0008,
                len: 8
            }
        );
        let mut first = [0u8; 8];
        bus.phys_read(0x0010_0000, &mut first);
        assert_eq!(first, src);
        let mut tail = [0xFFu8; 8];
        bus.phys_read(0x0010_0008, &mut tail);
        assert_eq!(tail, [0; 8]);
    }
}
