// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Pack System 3 recovery BOOT.BIN from the linked ELF. Host only. No COM12.

use std::fs;
use std::path::PathBuf;
use zynq_stage1_owned::bootimg::pack::{pack, Part};
use zynq_stage1_owned::bootimg::parse::{parse, ATTR_PL, ATTR_PS};
use zynq_stage1_owned::sha256::sha256;
use zynq_stage1_owned::{elf32_le_entry, validate_bootloader_artifact, UTILITY_ENTRY};

fn hex32(h: [u8; 32]) -> String {
    h.iter().map(|b| format!("{:02x}", b)).collect()
}

/// ELF32 LE PT_LOAD file image: bytes at `p_offset` for `p_filesz`, placed at `p_paddr`.
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
        let o = phoff.checked_add(i.checked_mul(phentsize).ok_or("phdr overflow")?)
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
    for &(paddr, offset, filesz) in &loads {
        let seg_end = paddr.checked_add(filesz).ok_or("segment overflow")?;
        if seg_end > end {
            end = seg_end;
        }
        let off = offset as usize;
        let n = filesz as usize;
        if elf.len() < off + n {
            return Err("PT_LOAD past EOF".into());
        }
        let _ = off;
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
    let elf_path = owned.join("build/sys03_plat.elf");
    let elf = fs::read(&elf_path).expect("sys03_plat.elf — link first");
    let (entry, paddr, filesz, stage1) = elf32_pt_load_files(&elf).expect("PT_LOAD");
    fs::write(owned.join("build/sys03_plat.bin"), &stage1).unwrap();

    let bit = fs::read(root.join("fpga/zynq/release/PZ7020_G1P_PATH1_G64/system.bit")).unwrap();
    let util_elf = fs::read(root.join("fpga/zynq/release/PZ7020_G1P_PATH1_G64/kairos_zynq.elf")).unwrap();
    let (util_entry, util_load, _util_fsz, util_bin) = elf32_pt_load_files(&util_elf).unwrap();
    assert_eq!(util_entry, UTILITY_ENTRY);
    assert_eq!(util_load, 0x0010_0000);

    validate_bootloader_artifact("sys03_plat.bin", &sha256(&stage1)).unwrap();

    let img = pack(
        Part {
            name: "sys03_plat.bin",
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
    let boot_path = owned.join("build/BOOT_RECOVERY_PLAINTEXT.BIN");
    fs::write(&boot_path, &img).unwrap();
    let parsed = parse(&img).expect("parse packed");
    assert_eq!(parsed.source_offset, 0x1700);
    assert_eq!(parsed.partitions[0].byte_len(), stage1.len());
    assert_eq!(parsed.partitions[0].exec_addr, entry);
    assert_eq!(parsed.partitions[2].exec_addr, UTILITY_ENTRY);

    let mut report = String::from("# System 3 linked artifacts\n\n");
    report.push_str("source_commit: 8ea707a409b59fddcb2dda7b4ac2097c333baca2\n");
    report.push_str("linker: fpga/zynq/tools/arm-gnu-toolchain-13.3.rel1-mingw-w64-i686-arm-none-eabi/bin/arm-none-eabi-gcc.exe\n");
    report.push_str("linker_id: Arm GNU Toolchain 13.3.Rel1 (Build arm-13.24) 13.3.1 20240614\n");
    report.push_str("vitis_d_gcc: unavailable (BitLocker)\n");
    report.push_str(&format!("elf_e_entry: 0x{entry:08X}\n"));
    report.push_str(&format!("pt_load_paddr: 0x{paddr:08X}\n"));
    report.push_str(&format!("pt_load_filesz: {filesz} (0x{filesz:X})\n"));
    report.push_str("elf_machine: ARM ELF32 LE EXEC\n");
    report.push_str("com12: not run\n");
    report.push_str("\nColumns: path, SHA-256, bytes.\n\n");
    for rel in [
        "zynq_stage1/owned/build/sys03_plat.elf",
        "zynq_stage1/owned/build/sys03_plat.bin",
        "zynq_stage1/owned/build/sys03_plat.map",
        "zynq_stage1/owned/build/sys03_plat.readelf.txt",
        "zynq_stage1/owned/build/BOOT_RECOVERY_PLAINTEXT.BIN",
        "zynq_stage1/src/main.rs",
        "zynq_stage1/owned/src/bin/sys03_plat.rs",
        "zynq_stage1/owned/src/platform/entry.rs",
        "zynq_stage1/owned/src/start.S",
        "fpga/zynq/release/PZ7020_G1P_PATH1_G64/system.bit",
        "fpga/zynq/release/PZ7020_G1P_PATH1_G64/kairos_zynq.elf",
    ] {
        let p = root.join(rel);
        if !p.exists() {
            continue;
        }
        let b = fs::read(&p).unwrap();
        report.push_str(&format!("| `{rel}` | `{}` | {} |\n", hex32(sha256(&b)), b.len()));
    }
    let hashes = root.join("docs/boot/SYS03_ARTIFACT_HASHES.txt");
    fs::write(&hashes, &report).unwrap();
    print!("{report}");
}
