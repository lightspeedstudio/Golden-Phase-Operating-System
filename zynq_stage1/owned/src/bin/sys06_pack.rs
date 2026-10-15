// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Pack encrypted System 6 P1 `BOOT.BIN` from `sys06_plat.elf`.
//! Keys stay in the gitignored `.nky`. This bin never prints key bytes.

use std::fs;
use std::path::PathBuf;
use zynq_stage1_owned::bootimg::bit::strip_encrypted_pl;
use zynq_stage1_owned::bootimg::frame::{check_encrypted_image, parse_all_encrypted, ENC_STATUS_BBRAM};
use zynq_stage1_owned::bootimg::pack::{pack_secure_image, Part};
use zynq_stage1_owned::bootimg::parse::{parse_all, ATTR_PL, ATTR_PS, HEADER_WINDOW_LEN};
use zynq_stage1_owned::bootimg::stream::keys_from_nky;
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
    let owned = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = owned.join("../..");
    let nky_path = owned.join("build/oracle/sys06_dev.nky");
    let nky_text = fs::read_to_string(&nky_path).expect("sys06_dev.nky");
    let keys = keys_from_nky(&nky_text).expect("nky Key 0 / StartCBC / HMAC lengths");
    let nky_bytes = fs::read(&nky_path).expect("nky bytes");
    let nky_hash = hex32(sha256(&nky_bytes));

    let elf = fs::read(owned.join("build/sys06_plat.elf")).expect("sys06_plat.elf");
    let (entry, paddr, filesz, stage1) = elf32_pt_load_files(&elf).expect("PT_LOAD");
    fs::write(owned.join("build/sys06_plat.bin"), &stage1).unwrap();
    validate_bootloader_artifact("sys06_plat.elf", &sha256(&stage1)).unwrap();

    let kat = fs::read(owned.join("build/oracle/ddr_kat.bin")).expect("ddr_kat.bin");
    assert_eq!(kat.len(), 256);
    let bit = fs::read(root.join("fpga/zynq/release/PZ7020_G1P_PATH1_G64/system.bit")).unwrap();
    let pl = strip_encrypted_pl(&bit).expect("strip PL");
    let util_elf = fs::read(root.join("fpga/zynq/release/PZ7020_G1P_PATH1_G64/kairos_zynq.elf")).unwrap();
    let (util_entry, util_load, _, utility) = elf32_pt_load_files(&util_elf).unwrap();
    assert_eq!(util_entry, UTILITY_ENTRY);

    let img = pack_secure_image(
        &[
            Part {
                name: "sys06_plat.elf",
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
            Part {
                name: "system.bit",
                data: &pl,
                load_addr: 0,
                exec_addr: 0,
                attributes: ATTR_PL,
            },
        ],
        Part {
            name: "kairos_zynq.elf",
            data: &utility,
            load_addr: util_load,
            exec_addr: util_entry,
            attributes: ATTR_PS,
        },
        &keys,
    )
    .expect("pack_secure_image");

    let enc = u32::from_le_bytes(img[0x28..0x2C].try_into().unwrap());
    assert_eq!(enc, ENC_STATUS_BBRAM);
    assert_eq!(
        parse_all(&img[..HEADER_WINDOW_LEN], HEADER_WINDOW_LEN, img.len()),
        Err(zynq_stage1_owned::bootimg::parse::ParseError::Encrypted)
    );
    let parsed = parse_all_encrypted(&img[..HEADER_WINDOW_LEN], HEADER_WINDOW_LEN, img.len())
        .expect("parse_all_encrypted");
    assert_eq!(parsed.partition_count, 4);
    assert_eq!(parsed.partitions[1].load_addr, 0x0300_0000);
    assert_eq!(parsed.partitions[3].exec_addr, UTILITY_ENTRY);
    check_encrypted_image(&img).expect("check_encrypted_image");

    let out = owned.join("build/oracle/BOOT_SYS06_P1.BIN");
    fs::write(&out, &img).unwrap();

    println!("path={}", out.display());
    println!("sha256={}", hex32(sha256(&img)));
    println!("bytes={}", img.len());
    println!("enc_status=0x{enc:08X}");
    println!("key_file={}", nky_path.display());
    println!("key_file_sha256={nky_hash}");
    println!("key_file_bytes={}", nky_bytes.len());
    println!("sys06_plat_pt_load={filesz}");
    println!("elf_e_entry=0x{entry:08X}");
    println!("pt_load_paddr=0x{paddr:08X}");
    println!("p1_file_off=0x{:X}", parsed.partitions[1].data_off);
    println!("efuse=no");
}
