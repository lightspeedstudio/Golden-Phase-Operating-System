// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Pack System 9 plaintext PCAP + Utility handoff BOOT.BIN. Does not flash or open COM12.

use std::fs;
use std::path::PathBuf;
use zynq_stage1_owned::bootimg::pack::{pack, Part};
use zynq_stage1_owned::bootimg::parse::{parse_all, ATTR_PL, ATTR_PS, HEADER_WINDOW_LEN};
use zynq_stage1_owned::handoff::{GOLD_UTIL_FILESZ, GOLD_UTIL_MEMSZ, UTIL_ALIGN, UTIL_LOAD};
use zynq_stage1_owned::sha256::sha256;
use zynq_stage1_owned::{elf32_first_pt_load, elf32_le_entry, validate_bootloader_artifact, UTILITY_ENTRY};

fn hex32(h: [u8; 32]) -> String {
    h.iter().map(|b| format!("{:02x}", b)).collect()
}

fn elf32_pt_load_files(elf: &[u8]) -> Result<(u32, u32, u32, Vec<u8>), String> {
    let entry = elf32_le_entry(elf).ok_or("not ELF32 LE")?;
    let p = elf32_first_pt_load(elf).ok_or("no PT_LOAD")?;
    let off = p.offset as usize;
    let n = p.filesz as usize;
    if elf.len() < off.checked_add(n).ok_or("PT_LOAD overflow")? {
        return Err("PT_LOAD truncated".into());
    }
    Ok((entry, p.paddr, p.filesz, elf[off..off + n].to_vec()))
}

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let owned = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let elf = fs::read(owned.join("build/sys09_plat.elf")).expect("sys09_plat.elf");
    let (entry, paddr, filesz, stage1) = elf32_pt_load_files(&elf).expect("PT_LOAD");
    fs::write(owned.join("build/sys09_plat.bin"), &stage1).unwrap();
    let bit = fs::read(root.join("fpga/zynq/release/PZ7020_G1P_PATH1_G64/system.bit")).unwrap();
    let util_elf = fs::read(root.join("fpga/zynq/release/PZ7020_G1P_PATH1_G64/kairos_zynq.elf")).unwrap();
    let (util_entry, util_load, util_filesz, util_bin) = elf32_pt_load_files(&util_elf).unwrap();
    let u = elf32_first_pt_load(&util_elf).unwrap();
    assert_eq!(util_entry, UTILITY_ENTRY);
    assert_eq!(util_load, UTIL_LOAD);
    assert_eq!(util_filesz, GOLD_UTIL_FILESZ);
    assert_eq!(u.memsz, GOLD_UTIL_MEMSZ);
    assert_eq!(u.align, UTIL_ALIGN);
    assert_eq!(util_bin.len(), GOLD_UTIL_FILESZ as usize);
    validate_bootloader_artifact("sys09_plat.bin", &sha256(&stage1)).unwrap();
    let img = pack(
        Part {
            name: "sys09_plat.bin",
            data: &stage1,
            load_addr: paddr,
            exec_addr: entry,
            attributes: ATTR_PS,
        },
        Part {
            name: "system.bit",
            data: &bit,
            load_addr: 0,
            exec_addr: 0,
            attributes: ATTR_PL,
        },
        Part {
            name: "kairos_zynq.bin",
            data: &util_bin,
            load_addr: util_load,
            exec_addr: util_entry,
            attributes: ATTR_PS,
        },
    )
    .expect("pack");
    fs::write(owned.join("build/BOOT_SYS09_HANDOFF.BIN"), &img).unwrap();
    let parsed = parse_all(&img[..HEADER_WINDOW_LEN], HEADER_WINDOW_LEN, img.len()).expect("parse_all");
    assert_eq!(parsed.source_offset, 0x1700);
    assert_eq!(parsed.partition_count, 3);
    assert!(parsed.partitions[1].is_pl());
    assert_eq!(parsed.partitions[2].exec_addr, UTILITY_ENTRY);
    assert_eq!(parsed.partitions[2].load_addr, UTIL_LOAD);
    assert_eq!(parsed.partitions[2].byte_len(), GOLD_UTIL_FILESZ as usize);
    println!(
        "BOOT_SYS09_HANDOFF.BIN sha256={} bytes={} pt_load={} e_entry=0x{entry:08X} paddr=0x{paddr:08X} filesz={filesz}",
        hex32(sha256(&img)),
        img.len(),
        stage1.len()
    );
}
