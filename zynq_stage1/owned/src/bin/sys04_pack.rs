// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Pack System 4 ADMA BOOT.BIN. Does not overwrite System 3 recovery image.

use std::fs;
use std::path::PathBuf;
use zynq_stage1_owned::bootimg::pack::{pack, Part};
use zynq_stage1_owned::bootimg::parse::{parse, ATTR_PL, ATTR_PS};
use zynq_stage1_owned::sha256::sha256;
use zynq_stage1_owned::{elf32_le_entry, validate_bootloader_artifact, UTILITY_ENTRY};

fn hex32(h: [u8; 32]) -> String {
    h.iter().map(|b| format!("{:02x}", b)).collect()
}

fn elf32_pt_load_files(elf: &[u8]) -> Result<(u32, u32, u32, Vec<u8>), String> {
    let entry = elf32_le_entry(elf).ok_or("not ELF32 LE")?;
    if elf.len() < 52 {
        return Err("truncated ELF header".into());
    }
    let phoff = u32::from_le_bytes(elf[0x1C..0x20].try_into().unwrap()) as usize;
    let phentsize = u16::from_le_bytes(elf[0x2A..0x2C].try_into().unwrap()) as usize;
    let phnum = u16::from_le_bytes(elf[0x2C..0x2E].try_into().unwrap()) as usize;
    if phentsize < 32 {
        return Err("phdr too small".into());
    }
    let mut loads = Vec::new();
    for i in 0..phnum {
        let o = phoff
            .checked_add(i.checked_mul(phentsize).ok_or("phdr overflow")?)
            .ok_or("phdr overflow")?;
        if elf.len() < o + 24 {
            return Err("phdr truncated".into());
        }
        let p_type = u32::from_le_bytes(elf[o..o + 4].try_into().unwrap());
        if p_type != 1 {
            continue;
        }
        let p_offset = u32::from_le_bytes(elf[o + 4..o + 8].try_into().unwrap());
        let p_paddr = u32::from_le_bytes(elf[o + 12..o + 16].try_into().unwrap());
        let p_filesz = u32::from_le_bytes(elf[o + 16..o + 20].try_into().unwrap());
        loads.push((p_paddr, p_offset, p_filesz));
    }
    if loads.is_empty() {
        return Err("no PT_LOAD".into());
    }
    loads.sort_by_key(|l| l.0);
    let base = loads[0].0;
    let mut end = base;
    for &(paddr, _, filesz) in &loads {
        let seg_end = paddr.checked_add(filesz).ok_or("segment overflow")?;
        if seg_end > end {
            end = seg_end;
        }
    }
    let mut img = vec![0u8; (end - base) as usize];
    for &(paddr, offset, filesz) in &loads {
        let dest = (paddr - base) as usize;
        let off = offset as usize;
        let n = filesz as usize;
        img[dest..dest + n].copy_from_slice(&elf[off..off + n]);
    }
    Ok((entry, base, end - base, img))
}

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let owned = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let elf = fs::read(owned.join("build/sys04_plat.elf")).expect("sys04_plat.elf");
    let (entry, paddr, filesz, stage1) = elf32_pt_load_files(&elf).expect("PT_LOAD");
    fs::write(owned.join("build/sys04_plat.bin"), &stage1).unwrap();
    let bit = fs::read(root.join("fpga/zynq/release/PZ7020_G1P_PATH1_G64/system.bit")).unwrap();
    let util_elf = fs::read(root.join("fpga/zynq/release/PZ7020_G1P_PATH1_G64/kairos_zynq.elf")).unwrap();
    let (util_entry, util_load, _, util_bin) = elf32_pt_load_files(&util_elf).unwrap();
    assert_eq!(util_entry, UTILITY_ENTRY);
    validate_bootloader_artifact("sys04_plat.bin", &sha256(&stage1)).unwrap();
    let img = pack(
        Part {
            name: "sys04_plat.bin",
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
    fs::write(owned.join("build/BOOT_SYS04_ADMA.BIN"), &img).unwrap();
    let parsed = parse(&img).expect("parse");
    assert_eq!(parsed.source_offset, 0x1700);
    assert_eq!(parsed.partitions[0].exec_addr, entry);

    let mut report = String::from("# System 4 linked artifacts\n\n");
    report.push_str("from_sys03_evidence: 659a7744a4c689d72ce4cc3c7f9f54280a675a43\n");
    report.push_str("linker: fpga/zynq/tools/arm-gnu-toolchain-13.3.rel1-mingw-w64-i686-arm-none-eabi/bin/arm-none-eabi-gcc.exe\n");
    report.push_str(&format!("elf_e_entry: 0x{entry:08X}\n"));
    report.push_str(&format!("pt_load_paddr: 0x{paddr:08X}\n"));
    report.push_str(&format!("pt_load_filesz: {filesz} (0x{filesz:X})\n"));
    report.push_str("com12: not run\n\n");
    for rel in [
        "zynq_stage1/owned/build/sys04_plat.elf",
        "zynq_stage1/owned/build/sys04_plat.bin",
        "zynq_stage1/owned/build/sys04_plat.map",
        "zynq_stage1/owned/build/BOOT_SYS04_ADMA.BIN",
        "docs/boot/SYS04_SD_REFERENCE.md",
        "docs/boot/SYS04_BOOT_FIRST_16K.bin",
        "docs/boot/SYS04_BOOT_DIRENT.bin",
        "zynq_stage1/src/main.rs",
        "src/board/zynq7000/sd.rs",
        "fpga/zynq/release/PZ7020_G1P_PATH1_G64/system.bit",
    ] {
        let p = root.join(rel);
        if !p.exists() {
            continue;
        }
        let b = fs::read(&p).unwrap();
        report.push_str(&format!("| `{rel}` | `{}` | {} |\n", hex32(sha256(&b)), b.len()));
    }
    fs::write(root.join("docs/boot/SYS04_ARTIFACT_HASHES.txt"), &report).unwrap();
    print!("{report}");
}
