// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Pack unsigned Stage 1 + RSA-authenticated P1 KAT. Keys stay gitignored.

use std::fs;
use std::path::PathBuf;
use zynq_stage1_owned::bootimg::pack::{pack_rsa_later, Part};
use zynq_stage1_owned::bootimg::parse::{parse, parse_all_rsa_later, ATTR_PS, HEADER_WINDOW_LEN};
use zynq_stage1_owned::rsa::pem::{parse_rsa_private_pem, parse_rsa_public_pem};
use zynq_stage1_owned::sha256::sha256;
use zynq_stage1_owned::{elf32_le_entry, validate_bootloader_artifact};

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
    let owned = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let keys = owned.join("build/oracle/sys07");
    let psk = fs::read_to_string(keys.join("psk.pem")).expect("psk.pem");
    let ssk = fs::read_to_string(keys.join("ssk.pem")).expect("ssk.pem");
    let ppk = fs::read_to_string(keys.join("ppk.pem")).expect("ppk.pem");
    let spk = fs::read_to_string(keys.join("spk.pem")).expect("spk.pem");
    let (ppk_n, _) = parse_rsa_public_pem(&ppk).expect("ppk");
    let (spk_n, _) = parse_rsa_public_pem(&spk).expect("spk");
    let (psk_n, _, psk_d) = parse_rsa_private_pem(&psk).expect("psk");
    let (ssk_n, _, ssk_d) = parse_rsa_private_pem(&ssk).expect("ssk");
    assert_eq!(psk_n, ppk_n);
    assert_eq!(ssk_n, spk_n);

    let elf = fs::read(owned.join("build/sys07_plat.elf")).expect("sys07_plat.elf");
    let (entry, paddr, filesz, stage1) = elf32_pt_load_files(&elf).expect("PT_LOAD");
    fs::write(owned.join("build/sys07_plat.bin"), &stage1).unwrap();
    validate_bootloader_artifact("sys07_plat.elf", &sha256(&stage1)).unwrap();
    let kat = fs::read(owned.join("build/oracle/ddr_kat.bin")).expect("ddr_kat.bin");
    assert_eq!(kat.len(), 256);

    let img = pack_rsa_later(
        Part {
            name: "sys07_plat.elf",
            data: &stage1,
            load_addr: paddr,
            exec_addr: entry,
            attributes: ATTR_PS,
        },
        Part {
            name: "ddr_kat.bin",
            data: &kat,
            load_addr: 0x0300_0000,
            exec_addr: 0,
            attributes: ATTR_PS,
        },
        &ppk_n,
        &psk_d,
        &spk_n,
        &ssk_d,
    )
    .expect("pack_rsa_later");

    assert_eq!(
        parse(&img),
        Err(zynq_stage1_owned::bootimg::parse::ParseError::VendorRsa)
    );
    let win = core::cmp::min(HEADER_WINDOW_LEN, img.len());
    let parsed = parse_all_rsa_later(&img[..win], win, img.len()).expect("parse_all_rsa_later");
    assert_eq!(parsed.partition_count, 2);
    assert_eq!(parsed.partitions[1].load_addr, 0x0300_0000);

    let out = owned.join("build/oracle/BOOT_SYS07_P1.BIN");
    fs::write(&out, &img).unwrap();
    println!("path={}", out.display());
    println!("sha256={}", hex32(sha256(&img)));
    println!("bytes={}", img.len());
    println!("sys07_plat_pt_load={filesz}");
    println!("elf_e_entry=0x{entry:08X}");
    println!("pt_load_paddr=0x{paddr:08X}");
    println!("p1_file_off=0x{:X}", parsed.partitions[1].data_off);
    println!("efuse=no");
    println!("bootrom_rsa=NOT_ENFORCED");
}
