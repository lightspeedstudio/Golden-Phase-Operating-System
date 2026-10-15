// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! System 10 / 10A host measurement constants.

pub const STATUS: &str = "SYS10_OVERSIZE";
pub const GATE: u32 = 8192;
pub const BOARD_BOOT_SHA256: &str =
    "eb2f34c3052f9c1a17b0ab34d31385decb0a3cc1c9c75fabacce97e9e63d7b27";
pub const BOARD_BOOT_BYTES: u64 = 6_005_972;
pub const BOARD_UART_SHA256: &str =
    "391218176cef09b7d642b71f5cca9990dec2d91b95cc73c9879185d9dfa0219d";
pub const BOARD_UART_BYTES: u64 = 6994;
pub const BOARD_ELF_SHA256: &str =
    "08502fd5fd9f9a5c16f333571a1e9d6f9970bb1d4618ae87a3683e0d4f77c082";
pub const BOARD_ELF_BYTES: u64 = 34_072;
pub const BOARD_PT_LOAD_SHA256: &str =
    "8b120b51296aa755ff93ced945441c116557f4af0873636dac3569f8273ae44b";
pub const PT_LOAD_FILESZ: u32 = 0x4B58;
pub const PT_LOAD_MEMSZ: u32 = 0x4B58;
pub const OCM_SPAN: u32 = 0x6B60;
pub const UNIFIED_ELF_SHA256: &str =
    "cbc4e37ad3a8444541db886a040a42dc0c94c9a09102bffb3b51f5280ee26854";
pub const UNIFIED_ELF_BYTES: u64 = 50_292;
pub const UNIFIED_PT_LOAD_SHA256: &str =
    "8e4254ead8617071ec8c768272b2ae347b2caaf629aeb67298dbd49f6097ef42";
pub const UNIFIED_PT_LOAD_FILESZ: u32 = 0x748C;
pub const UNIFIED_PT_LOAD_MEMSZ: u32 = 0x7490;
pub const UNIFIED_OCM_SPAN: u32 = 0x9490;
pub const UNIFIED_BOOT_SHA256: &str =
    "8b396c6f519935cb8099747db466c2bc7647df0666ece36ccf6d0a261e577874";
pub const UNIFIED_BOOT_BYTES: u64 = 6_018_260;
pub const UTILITY_ENTRY: u32 = 0x0010_0060;
pub const SOURCE_OFFSET: u32 = 0x1700;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sha256::sha256;
    use crate::{
        classify_link_member, elf32_le_entry, elf32_pt_loads, validate_bootloader_artifact,
        LinkClass, MAIN_RS_SHA256_HEX, QUARANTINED_FSBL_SHA256,
    };
    use std::fs;
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn hex32(h: [u8; 32]) -> String {
        h.iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn hashes_txt() -> String {
        fs::read_to_string(root().join("docs/boot/SYS10_ARTIFACT_HASHES.txt")).unwrap()
    }

    fn hashes10a() -> String {
        fs::read_to_string(root().join("docs/boot/SYS10A_ARTIFACT_HASHES.txt")).unwrap()
    }

    #[test]
    fn status_is_oversize_complete_link_set() {
        let size = fs::read_to_string(root().join("docs/boot/STAGE1_SIZE_REPORT.md")).unwrap();
        assert!(size.contains("Status: **SYS10_OVERSIZE**"));
        assert!(!size.contains("Status: **SYS10_SIZE_PASS**"));
        assert!(size.contains("SYS10_INCOMPLETE_LINK_SET"));
        assert_eq!(STATUS, "SYS10_OVERSIZE");
        let a = hashes10a();
        assert!(a.contains("status\tSYS10_OVERSIZE"));
        assert!(a.contains(&format!("unified_boot\t{UNIFIED_BOOT_SHA256}")));
        let h = hashes_txt();
        assert!(h.contains("status\tSYS10_INCOMPLETE_LINK_SET"));
        assert!(h.contains(&format!("board_passed_boot\t{BOARD_BOOT_SHA256}")));
    }

    #[test]
    fn main_rs_unchanged() {
        let p = root().join("zynq_stage1/src/main.rs");
        let b = fs::read(&p).unwrap();
        assert_eq!(b.len(), 9580);
        assert_eq!(hex32(sha256(&b)), MAIN_RS_SHA256_HEX);
    }

    #[test]
    fn board_passed_uart_and_elf() {
        let uart = fs::read(
            root().join("fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS09_20260820/com12_sys09.uart"),
        )
        .unwrap();
        assert_eq!(uart.len() as u64, BOARD_UART_BYTES);
        assert_eq!(hex32(sha256(&uart)), BOARD_UART_SHA256);
        let elf = fs::read(root().join("docs/boot/sys10/sys09_plat.elf.board_passed")).unwrap();
        assert_eq!(elf.len() as u64, BOARD_ELF_BYTES);
        assert_eq!(hex32(sha256(&elf)), BOARD_ELF_SHA256);
        let bin = fs::read(root().join("docs/boot/sys10/sys09_plat.bin.board_passed")).unwrap();
        assert_eq!(hex32(sha256(&bin)), BOARD_PT_LOAD_SHA256);
        assert_eq!(bin.len() as u32, PT_LOAD_FILESZ);
    }

    #[test]
    fn all_pt_load_counted_filesz_not_confused_with_memsz() {
        let elf = fs::read(root().join("docs/boot/sys10/sys09_plat.elf.board_passed")).unwrap();
        let (entry, loads) = elf32_pt_loads(&elf).unwrap();
        assert_eq!(entry, 0);
        assert_eq!(loads.len(), 1, "ignoring a PT_LOAD is a System 10 failure");
        assert_eq!(loads[0].filesz, PT_LOAD_FILESZ);
        assert_eq!(loads[0].memsz, PT_LOAD_MEMSZ);
        assert_eq!(loads[0].paddr, 0);
        assert_eq!(loads[0].offset, 0x1000);
        assert_eq!(loads[0].align, 0x1000);
        let filesz_sum: u32 = loads.iter().map(|l| l.filesz).sum();
        let memsz_sum: u32 = loads.iter().map(|l| l.memsz).sum();
        assert_eq!(filesz_sum, PT_LOAD_FILESZ);
        assert_eq!(memsz_sum, PT_LOAD_MEMSZ);
        let h = hashes_txt();
        assert!(h.contains(&format!("pt_load_filesz_sum\t{filesz_sum}")));
        assert!(h.contains(&format!("pt_load_memsz_sum\t{memsz_sum}")));
        assert!(h.contains(&format!("ocm_span\t{OCM_SPAN}")));
        assert!(filesz_sum > GATE);
        assert!(OCM_SPAN > GATE);
        assert_eq!(elf32_le_entry(&elf), Some(0));
    }

    #[test]
    fn payload_matches_pt_load_filesz_not_memsz_tail() {
        let elf = fs::read(root().join("docs/boot/sys10/sys09_plat.elf.board_passed")).unwrap();
        let bin = fs::read(root().join("docs/boot/sys10/sys09_plat.bin.board_passed")).unwrap();
        let (_, loads) = elf32_pt_loads(&elf).unwrap();
        let off = loads[0].offset as usize;
        let n = loads[0].filesz as usize;
        assert_eq!(&elf[off..off + n], bin.as_slice());
        assert_ne!(n, GATE as usize);
    }

    #[test]
    fn quarantined_fsbl_refused_and_not_this_elf() {
        let elf = fs::read(root().join("docs/boot/sys10/sys09_plat.elf.board_passed")).unwrap();
        let h = sha256(&elf);
        assert_ne!(h, QUARANTINED_FSBL_SHA256);
        assert_eq!(
            validate_bootloader_artifact("zynq_fsbl.elf", &h),
            Err(crate::ArtifactError::FsblFilename)
        );
        let s = String::from_utf8_lossy(&elf);
        assert!(!s.contains("zynq_fsbl"));
        assert!(!s.contains("Xilinx"));
        assert!(!s.contains("Bootgen"));
        assert!(!s.contains("kairos"));
    }

    #[test]
    fn utility_entry_constant() {
        assert_eq!(UTILITY_ENTRY, crate::UTILITY_ENTRY);
        assert_eq!(SOURCE_OFFSET, 0x1700);
    }

    #[test]
    fn sys09_incomplete_and_sys10a_complete_linkage() {
        let nm9 = fs::read_to_string(root().join("docs/boot/SYS10_SYMBOL_SIZES.txt")).unwrap();
        for need in [
            "rust_main",
            "parse_all",
            "pcap_pl",
            "handoff_app",
            "read_blocks_adma",
            "FatVol",
            "run_ops",
        ] {
            assert!(nm9.contains(need), "missing {need}");
        }
        assert!(!nm9.contains("secure_transfer"));
        assert!(!nm9.contains("::rsa::"));
        assert!(!nm9.contains("verify_pkcs1"));
        assert!(!nm9.contains("sys07"));
        let nm = fs::read_to_string(root().join("docs/boot/SYS10A_SYMBOL_SIZES.txt")).unwrap();
        for need in [
            "rust_main",
            "pcap_pl",
            "p1_decrypt_kat",
            "p1_rsa_kat",
            "verify_pkcs1",
            "parse_all_encrypted",
            "parse_all_product",
            "working_entry",
            "read_blocks_adma",
        ] {
            assert!(nm.contains(need), "missing {need}");
        }
        let size = fs::read_to_string(root().join("docs/boot/STAGE1_SIZE_REPORT.md")).unwrap();
        assert!(size.contains("p1_decrypt_kat"));
        assert!(size.contains("p1_rsa_kat"));
        assert!(size.contains("Kind::Plaintext"));
    }

    #[test]
    fn map_has_no_unknown_and_plats_are_owned() {
        let sum = fs::read_to_string(root().join("docs/boot/SYS10_LINK_MAP_SUMMARY.txt")).unwrap();
        assert!(sum.contains("unknown_count\t0"));
        let sum_a = fs::read_to_string(root().join("docs/boot/SYS10A_LINK_MAP_SUMMARY.txt")).unwrap();
        assert!(sum_a.contains("unknown_count\t0"));
        assert_eq!(
            classify_link_member("sys09_plat-ddcc1079d3b24560.sys09_plat.be8b9d82045bc284-cgu.0.rcgu.o"),
            LinkClass::LightSpeedOwned
        );
        assert_eq!(
            classify_link_member("sys10a_plat-eb61048b047cb2bc.sys10a_plat.10d603b2636123c4-cgu.0.rcgu.o"),
            LinkClass::LightSpeedOwned
        );
        assert_eq!(
            classify_link_member("vendor_bsp.a"),
            LinkClass::UnexpectedFail
        );
    }

    #[test]
    fn reports_have_no_secrets() {
        for rel in [
            "docs/boot/STAGE1_SIZE_REPORT.md",
            "docs/boot/SYS10_ARTIFACT_HASHES.txt",
            "docs/boot/SYS10_SYMBOL_SIZES.txt",
            "docs/boot/SYS10_LINK_MAP_SUMMARY.txt",
            "docs/boot/SYS10A_ARTIFACT_HASHES.txt",
            "docs/boot/SYS10A_SYMBOL_SIZES.txt",
            "docs/boot/SYS10A_LINK_MAP_SUMMARY.txt",
        ] {
            let t = fs::read_to_string(root().join(rel)).unwrap();
            let low = t.to_ascii_lowercase();
            assert!(!low.contains("begin rsa private"));
            assert!(!low.contains("begin encrypted"));
            assert!(!t.contains(".nky"));
            assert!(!t.contains("-----BEGIN"));
        }
    }

    #[test]
    fn path1_g64_and_gold_boot_unchanged() {
        let bit = fs::read(root().join("fpga/zynq/release/PZ7020_G1P_PATH1_G64/system.bit")).unwrap();
        assert_eq!(
            hex32(sha256(&bit)),
            "d19f08d20cac85ea0e33c41a8a552166f5a9176e16977f09aa64d17e68135829"
        );
        let util = fs::read(root().join("fpga/zynq/release/PZ7020_G1P_PATH1_G64/kairos_zynq.elf")).unwrap();
        assert_eq!(elf32_le_entry(&util), Some(crate::UTILITY_ENTRY));
        let gold = fs::read(root().join("fpga/zynq/release/PZ7020_G1P_PATH1_G64/BOOT.BIN")).unwrap();
        assert_eq!(
            hex32(sha256(&gold)),
            "7da748547d5016191643ddc4df555a8cf02c1d8adee3235f1dde766166ea0b85"
        );
        assert_ne!(hex32(sha256(&gold)), BOARD_BOOT_SHA256);
    }

    #[test]
    fn optional_local_boot_must_match_board_hash() {
        let p = root().join("zynq_stage1/owned/build/BOOT_SYS09_HANDOFF.BIN");
        if p.exists() {
            let b = fs::read(&p).unwrap();
            assert_eq!(b.len() as u64, BOARD_BOOT_BYTES);
            assert_eq!(hex32(sha256(&b)), BOARD_BOOT_SHA256);
        }
        let u = root().join("zynq_stage1/owned/build/BOOT_SYS10A_UNIFIED.BIN");
        if u.exists() {
            let b = fs::read(&u).unwrap();
            assert_eq!(b.len() as u64, UNIFIED_BOOT_BYTES);
            assert_eq!(hex32(sha256(&b)), UNIFIED_BOOT_SHA256);
        }
    }

    #[test]
    fn unified_elf_pt_load_and_ocm() {
        let elf = fs::read(root().join("docs/boot/sys10a/sys10a_plat.elf")).unwrap();
        assert_eq!(elf.len() as u64, UNIFIED_ELF_BYTES);
        assert_eq!(hex32(sha256(&elf)), UNIFIED_ELF_SHA256);
        let bin = fs::read(root().join("docs/boot/sys10a/sys10a_plat.bin")).unwrap();
        assert_eq!(hex32(sha256(&bin)), UNIFIED_PT_LOAD_SHA256);
        assert_eq!(bin.len() as u32, UNIFIED_PT_LOAD_FILESZ);
        let (_, loads) = elf32_pt_loads(&elf).unwrap();
        assert_eq!(loads.len(), 1);
        assert_eq!(loads[0].filesz, UNIFIED_PT_LOAD_FILESZ);
        assert_eq!(loads[0].memsz, UNIFIED_PT_LOAD_MEMSZ);
        assert_ne!(loads[0].filesz, loads[0].memsz);
        let filesz_sum: u32 = loads.iter().map(|l| l.filesz).sum();
        assert!(filesz_sum > GATE);
        assert!(UNIFIED_OCM_SPAN > GATE);
        assert_eq!(filesz_sum, UNIFIED_PT_LOAD_FILESZ);
        let a = hashes10a();
        assert!(a.contains(&format!("pt_load_filesz_sum\t{filesz_sum}")));
        assert!(a.contains(&format!("ocm_span\t{UNIFIED_OCM_SPAN}")));
        let off = loads[0].offset as usize;
        let n = loads[0].filesz as usize;
        assert_eq!(&elf[off..off + n], bin.as_slice());
    }
}
