// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Pack System 10A recovery (plaintext) and unified (later-partition auth + PCAP + handoff).
//! Does not flash or open COM12.

use std::env;
use std::fs;
use std::path::PathBuf;
use zynq_stage1_owned::bootimg::pack::{pack, pack_rsa_later_product, Part};
use zynq_stage1_owned::bootimg::parse::{
    parse_all, parse_all_product, ATTR_PL, ATTR_PS, HEADER_WINDOW_LEN,
};
use zynq_stage1_owned::handoff::{
    GOLD_UTIL_FILESZ, GOLD_UTIL_MEMSZ, UTIL_ALIGN, UTIL_FILESZ, UTIL_LOAD, UTIL_MEMSZ,
};
use zynq_stage1_owned::sha256::sha256;
use zynq_stage1_owned::{
    elf32_first_pt_load, elf32_le_entry, elf32_pt_load_span, validate_bootloader_artifact,
    UTILITY_ENTRY,
};

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

fn wp_le(s: &str) -> [u8; 256] {
    let t = if s.len() == 514 && s.starts_with("00") {
        &s[2..]
    } else {
        s
    };
    let mut be = [0u8; 256];
    for i in 0..256 {
        be[i] = u8::from_str_radix(&t[i * 2..i * 2 + 2], 16).unwrap();
    }
    let mut le = [0u8; 256];
    for i in 0..256 {
        le[i] = be[255 - i];
    }
    le
}

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let owned = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let elf = fs::read(owned.join("build/sys10a_plat.elf")).expect("sys10a_plat.elf");
    let (entry, paddr, filesz, stage1) = elf32_pt_load_files(&elf).expect("PT_LOAD");
    fs::write(owned.join("build/sys10a_plat.bin"), &stage1).unwrap();
    let bit = fs::read(root.join("fpga/zynq/release/PZ7020_G1P_PATH1_G64/system.bit")).unwrap();
    let util_path = env::var_os("STAGE1_UTIL_ELF")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("fpga/zynq/release/PZ7020_G1P_PATH1_G64/kairos_zynq.elf"));
    let util_elf = fs::read(&util_path).unwrap();
    let (util_entry, util_load, util_filesz, util_memsz, util_bin) =
        elf32_pt_load_span(&util_elf).expect("PT_LOAD span");
    let u0 = elf32_first_pt_load(&util_elf).unwrap();
    assert_eq!(util_entry, UTILITY_ENTRY);
    assert_eq!(util_load, UTIL_LOAD);
    if util_filesz == UTIL_FILESZ {
        assert_eq!(util_memsz, UTIL_MEMSZ);
    } else {
        assert_eq!(util_filesz, GOLD_UTIL_FILESZ);
        assert_eq!(util_memsz, GOLD_UTIL_MEMSZ);
        assert_eq!(u0.align, UTIL_ALIGN);
    }
    assert_eq!(util_bin.len(), util_filesz as usize);
    validate_bootloader_artifact("sys10a_plat.bin", &sha256(&stage1)).unwrap();

    let stage = Part {
        name: "sys10a_plat.bin",
        data: &stage1,
        load_addr: paddr,
        exec_addr: entry,
        attributes: ATTR_PS,
    };
    let bitstream = Part {
        name: "system.bit",
        data: &bit,
        load_addr: 0,
        exec_addr: 0,
        attributes: ATTR_PL,
    };
    let app = Part {
        name: "kairos_zynq.bin",
        data: &util_bin,
        load_addr: util_load,
        exec_addr: util_entry,
        attributes: ATTR_PS,
    };

    let recovery = pack(stage, bitstream, app).expect("pack recovery");
    fs::write(owned.join("build/BOOT_SYS10A_RECOVERY.BIN"), &recovery).unwrap();
    let parsed = parse_all(
        &recovery[..HEADER_WINDOW_LEN],
        HEADER_WINDOW_LEN,
        recovery.len(),
    )
    .expect("parse_all recovery");
    assert_eq!(parsed.source_offset, 0x1700);
    assert_eq!(parsed.partition_count, 3);
    assert_eq!(parsed.partitions[2].exec_addr, UTILITY_ENTRY);
    assert_eq!(parsed.partitions[2].byte_len(), util_filesz as usize);

    println!(
        "BOOT_SYS10A_RECOVERY.BIN sha256={} bytes={}",
        hex32(sha256(&recovery)),
        recovery.len()
    );

    if util_filesz != GOLD_UTIL_FILESZ {
        println!(
            "skip unified pack (main Utility filesz=0x{util_filesz:08X}) pt_load={} e_entry=0x{entry:08X} paddr=0x{paddr:08X} filesz={filesz}",
            stage1.len()
        );
        return;
    }

    let mut kat = [0u8; 256];
    for i in 0..256 {
        kat[i] = i as u8;
    }
    let n = wp_le("00b3510a2bcd4ce644c5b594ae5059e12b2f054b658d5da5959a2fdf1871b808bc3df3e628d2792e51aad5c124b43bda453dca5cde4bcf28e7bd4effba0cb4b742bbb6d5a013cb63d1aa3a89e02627ef5398b52c0cfd97d208abeb8d7c9bce0bbeb019a86ddb589beb29a5b74bf861075c677c81d430f030c265247af9d3c9140ccb65309d07e0adc1efd15cf17e7b055d7da3868e4648cc3a180f0ee7f8e1e7b18098a3391b4ce7161e98d57af8a947e201a463e2d6bbca8059e5706e9dfed8f4856465ffa712ed1aa18e888d12dc6aa09ce95ecfca83cc5b0b15db09c8647f5d524c0f2e7620a3416b9623cadc0f097af573261c98c8400aa12af38e43cad84d");
    let d = wp_le("1a502d0eea6c7b69e21d5839101f705456ed0ef852fb47fe21071f54c5f33c8ceb066c62d727e32d26c58137329f89d3195325b795264c195d85472f7507dbd0961d2951f935a26b34f0ac24d15490e1128a9b7138915bc7dbfa8fe396357131c543ae9c98507368d9ceb08c1c6198a3eda7aea185a0e976cd42c22d00f003d9f19d96ea4c9afcbfe1441ccc802cfb0689f59d804c6a4e4f404c15174745ed6cb8bc88ef0b33ba0d2a80e35e43bc90f350052e72016e75b00d357a381c9c0d467069ca660887c987766349fcc43460b4aa516bce079edd87ba164307b752c277ed9528ad3ba0bf1877349ed3b7966a6c240110409bf4d0fade0c68fdadd847fd");
    let unified = pack_rsa_later_product(
        stage,
        Part {
            name: "ddr_kat.bin",
            data: &kat,
            load_addr: 0x0300_0000,
            exec_addr: 0,
            attributes: ATTR_PS,
        },
        bitstream,
        app,
        &n,
        &d,
        &n,
        &d,
    )
    .expect("pack unified");
    fs::write(owned.join("build/BOOT_SYS10A_UNIFIED.BIN"), &unified).unwrap();
    let parsed = parse_all_product(
        &unified[..HEADER_WINDOW_LEN],
        HEADER_WINDOW_LEN,
        unified.len(),
    )
    .expect("parse_all_product");
    assert_eq!(parsed.source_offset, 0x1700);
    assert_eq!(parsed.partition_count, 4);
    assert_eq!(parsed.partitions[1].load_addr, 0x0300_0000);
    assert!(parsed.partitions[2].is_pl());
    assert_eq!(parsed.partitions[3].exec_addr, UTILITY_ENTRY);
    assert_eq!(parsed.partitions[3].byte_len(), GOLD_UTIL_FILESZ as usize);

    println!(
        "BOOT_SYS10A_RECOVERY.BIN sha256={} bytes={}",
        hex32(sha256(&recovery)),
        recovery.len()
    );
    println!(
        "BOOT_SYS10A_UNIFIED.BIN sha256={} bytes={} pt_load={} e_entry=0x{entry:08X} paddr=0x{paddr:08X} filesz={filesz}",
        hex32(sha256(&unified)),
        unified.len(),
        stage1.len()
    );
}
