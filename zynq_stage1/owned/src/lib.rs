// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Stage 1 Systems 1–10A: provenance, boot-image, PS7/UART, SD ADMA, parse_all,
//! encrypted-partition framing, DevCfg secure PCAP, host encrypt packer,
//! software later-partition verify, plaintext FabricInit/PCAP,
//! Utility PT_LOAD handoff, host size/provenance measurement, unified
//! plaintext/encrypted/RSA dispatcher.
//!
//! This crate has no `[dependencies]`. Host-std packing can emit encrypted
//! partitions and authenticated later partitions. The device rlib does
//! not run AES. BootROM RSA is not enforced.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod sha256;
pub mod rsa;
pub mod bootimg;
pub mod platform;
pub mod sd;
pub mod devcfg;
pub mod handoff;

#[cfg(feature = "std")]
pub mod sys10;
#[cfg(feature = "std")]
pub mod sys11;

/// Quarantined vendor FSBL (`zynq_fsbl.elf`).
pub const QUARANTINED_FSBL_SHA256: [u8; 32] = [
    0x8b, 0x71, 0xaa, 0x70, 0x7f, 0xd9, 0xe2, 0x00, 0xa8, 0x9c, 0xd2, 0xb9, 0x66, 0xcf, 0x3d, 0x3e,
    0xc2, 0x6e, 0xd0, 0x35, 0x04, 0x95, 0x71, 0x8a, 0x51, 0xa9, 0xf8, 0xec, 0x25, 0x9f, 0x78, 0x74,
];

/// Gold Utility `e_entry` (JUMP target). Not `0x00100020`.
pub const UTILITY_ENTRY: u32 = 0x0010_0060;

/// Owned shortcut `zynq_stage1/src/main.rs` SHA-256 at lock time.
pub const MAIN_RS_SHA256_HEX: &str =
    "6637678d0e01b007caf3b5e150ed37e48ed5af6f55cc009a3b16e0d5c4ebd67d";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactError {
    FsblFilename,
    QuarantinedHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkClass {
    LightSpeedOwned,
    RustCore,
    CompilerBuiltins,
    UnexpectedFail,
}

/// Refuse the quarantined vendor bootloader by name or content hash.
pub fn validate_bootloader_artifact(filename: &str, sha256: &[u8; 32]) -> Result<(), ArtifactError> {
    if filename.eq_ignore_ascii_case("zynq_fsbl.elf") {
        return Err(ArtifactError::FsblFilename);
    }
    if sha256 == &QUARANTINED_FSBL_SHA256 {
        return Err(ArtifactError::QuarantinedHash);
    }
    Ok(())
}

/// Classify a rustc archive or object name from the Stage 1 owned rlib / rmeta set.
pub fn classify_link_member(name: &str) -> LinkClass {
    let base = name.rsplit(|c| c == '/' || c == '\\').next().unwrap_or(name);
    if base.contains("sys09_plat") || base.starts_with("sys09_plat") {
        return LinkClass::LightSpeedOwned;
    }
    if base.contains("sys10a_plat") || base.starts_with("sys10a_plat") {
        return LinkClass::LightSpeedOwned;
    }
    if base.starts_with("zynq_stage1_owned") || base.contains("zynq_stage1_owned") {
        return LinkClass::LightSpeedOwned;
    }
    if base.starts_with("core") || base.contains("libcore-") {
        return LinkClass::RustCore;
    }
    if base.contains("compiler_builtins") {
        return LinkClass::CompilerBuiltins;
    }
    if base.ends_with(".rmeta") || base == "lib.rmeta" {
        return LinkClass::LightSpeedOwned;
    }
    if base.ends_with(".o") && !base.contains('-') {
        return LinkClass::LightSpeedOwned;
    }
    LinkClass::UnexpectedFail
}

/// 32-bit little-endian ELF `e_entry`, or `None` if the header is not ELF32 LE.
pub fn elf32_le_entry(bytes: &[u8]) -> Option<u32> {
    if bytes.len() < 0x1C {
        return None;
    }
    if bytes[0] != 0x7F || bytes[1] != b'E' || bytes[2] != b'L' || bytes[3] != b'F' {
        return None;
    }
    if bytes[4] != 1 || bytes[5] != 1 {
        return None;
    }
    Some(u32::from_le_bytes([bytes[0x18], bytes[0x19], bytes[0x1A], bytes[0x1B]]))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Elf32PtLoad {
    pub paddr: u32,
    pub filesz: u32,
    pub memsz: u32,
    pub offset: u32,
    pub align: u32,
}

/// First ELF32 PT_LOAD (p_type = 1). `None` if the header is not ELF32 LE.
pub fn elf32_first_pt_load(bytes: &[u8]) -> Option<Elf32PtLoad> {
    if elf32_le_entry(bytes).is_none() || bytes.len() < 52 {
        return None;
    }
    let phoff = u32::from_le_bytes(bytes[0x1C..0x20].try_into().ok()?) as usize;
    let phentsize = u16::from_le_bytes(bytes[0x2A..0x2C].try_into().ok()?) as usize;
    let phnum = u16::from_le_bytes(bytes[0x2C..0x2E].try_into().ok()?) as usize;
    if phentsize < 32 {
        return None;
    }
    for i in 0..phnum {
        let o = phoff.checked_add(i.checked_mul(phentsize)?)?;
        if bytes.len() < o + 32 {
            return None;
        }
        let p_type = u32::from_le_bytes(bytes[o..o + 4].try_into().ok()?);
        if p_type != 1 {
            continue;
        }
        return Some(Elf32PtLoad {
            offset: u32::from_le_bytes(bytes[o + 4..o + 8].try_into().ok()?),
            paddr: u32::from_le_bytes(bytes[o + 12..o + 16].try_into().ok()?),
            filesz: u32::from_le_bytes(bytes[o + 16..o + 20].try_into().ok()?),
            memsz: u32::from_le_bytes(bytes[o + 20..o + 24].try_into().ok()?),
            align: u32::from_le_bytes(bytes[o + 28..o + 32].try_into().ok()?),
        });
    }
    None
}

/// All ELF32 PT_LOAD segments (`p_type = 1`). `None` if not ELF32 LE.
#[cfg(feature = "std")]
pub fn elf32_pt_loads(bytes: &[u8]) -> Option<(u32, Vec<Elf32PtLoad>)> {
    let entry = elf32_le_entry(bytes)?;
    if bytes.len() < 52 {
        return None;
    }
    let phoff = u32::from_le_bytes(bytes[0x1C..0x20].try_into().ok()?) as usize;
    let phentsize = u16::from_le_bytes(bytes[0x2A..0x2C].try_into().ok()?) as usize;
    let phnum = u16::from_le_bytes(bytes[0x2C..0x2E].try_into().ok()?) as usize;
    if phentsize < 32 {
        return None;
    }
    let mut loads = Vec::new();
    for i in 0..phnum {
        let o = phoff.checked_add(i.checked_mul(phentsize)?)?;
        if bytes.len() < o + 32 {
            return None;
        }
        let p_type = u32::from_le_bytes(bytes[o..o + 4].try_into().ok()?);
        if p_type != 1 {
            continue;
        }
        loads.push(Elf32PtLoad {
            offset: u32::from_le_bytes(bytes[o + 4..o + 8].try_into().ok()?),
            paddr: u32::from_le_bytes(bytes[o + 12..o + 16].try_into().ok()?),
            filesz: u32::from_le_bytes(bytes[o + 16..o + 20].try_into().ok()?),
            memsz: u32::from_le_bytes(bytes[o + 20..o + 24].try_into().ok()?),
            align: u32::from_le_bytes(bytes[o + 28..o + 32].try_into().ok()?),
        });
    }
    Some((entry, loads))
}

/// Linear PT_LOAD image: `p_filesz` file bytes then zeros to `p_memsz`.
#[cfg(feature = "std")]
pub fn elf32_pt_load_image(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let entry = elf32_le_entry(bytes)?;
    let p = elf32_first_pt_load(bytes)?;
    if p.memsz < p.filesz {
        return None;
    }
    let off = p.offset as usize;
    let n = p.filesz as usize;
    if bytes.len() < off.checked_add(n)? {
        return None;
    }
    let mut img = vec![0u8; p.memsz as usize];
    img[..n].copy_from_slice(&bytes[off..off + n]);
    Some((entry, p.paddr, img))
}

/// Contiguous DDR image covering every PT_LOAD: file bytes through the last `p_filesz`, zeros in gaps.
#[cfg(feature = "std")]
pub fn elf32_pt_load_span(bytes: &[u8]) -> Option<(u32, u32, u32, u32, Vec<u8>)> {
    let (entry, loads) = elf32_pt_loads(bytes)?;
    if loads.is_empty() {
        return None;
    }
    let dest = loads.iter().map(|l| l.paddr).min()?;
    let mut end_file = dest;
    let mut end_mem = dest;
    for l in &loads {
        if l.memsz < l.filesz {
            return None;
        }
        let fend = l.paddr.checked_add(l.filesz)?;
        let mend = l.paddr.checked_add(l.memsz)?;
        if fend > end_file {
            end_file = fend;
        }
        if mend > end_mem {
            end_mem = mend;
        }
    }
    let filesz = end_file.checked_sub(dest)?;
    let memsz = end_mem.checked_sub(dest)?;
    let mut img = vec![0u8; filesz as usize];
    for l in &loads {
        let rel = (l.paddr.checked_sub(dest)?) as usize;
        let off = l.offset as usize;
        let n = l.filesz as usize;
        if bytes.len() < off.checked_add(n)? {
            return None;
        }
        img[rel..rel + n].copy_from_slice(&bytes[off..off + n]);
    }
    Some((entry, dest, filesz, memsz, img))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_fsbl_filename() {
        let z = [0u8; 32];
        assert_eq!(
            validate_bootloader_artifact("zynq_fsbl.elf", &z),
            Err(ArtifactError::FsblFilename)
        );
        assert_eq!(
            validate_bootloader_artifact("ZYNQ_FSBL.ELF", &z),
            Err(ArtifactError::FsblFilename)
        );
    }

    #[test]
    fn refuses_quarantined_hash_even_if_renamed() {
        assert_eq!(
            validate_bootloader_artifact("not_fsbl.bin", &QUARANTINED_FSBL_SHA256),
            Err(ArtifactError::QuarantinedHash)
        );
    }

    #[test]
    fn allows_owned_name_and_non_quarantined_hash() {
        let mut h = QUARANTINED_FSBL_SHA256;
        h[0] ^= 1;
        assert!(validate_bootloader_artifact("zynq_stage1.elf", &h).is_ok());
    }

    #[test]
    fn classifies_known_link_members() {
        assert_eq!(
            classify_link_member("libzynq_stage1_owned-abc123.rlib"),
            LinkClass::LightSpeedOwned
        );
        assert_eq!(
            classify_link_member("libcore-0123.rlib"),
            LinkClass::RustCore
        );
        assert_eq!(
            classify_link_member("libcompiler_builtins-99.rlib"),
            LinkClass::CompilerBuiltins
        );
        assert_eq!(
            classify_link_member("vendor_bsp.a"),
            LinkClass::UnexpectedFail
        );
        assert_eq!(
            classify_link_member("sys09_plat-abc.rcgu.o"),
            LinkClass::LightSpeedOwned
        );
        assert_eq!(
            classify_link_member("sys10a_plat-abc.rcgu.o"),
            LinkClass::LightSpeedOwned
        );
    }

    #[test]
    fn crate_toml_has_no_forbidden_deps() {
        let t = include_str!("../Cargo.toml");
        assert!(!t.contains("kairos"));
        assert!(!t.contains("sha2"));
        assert!(!t.contains("hmac"));
        assert!(!t.contains("aes-gcm"));
        assert!(!t.contains("ed25519-dalek"));
        assert!(!t.contains("aes ="));
        assert!(!t.contains("rsa"));
        assert!(!t.to_ascii_lowercase().contains("xilinx"));
        assert!(!t.to_ascii_lowercase().contains("zynq_fsbl"));
        assert!(!t.contains("[dependencies]"));
        assert!(t.contains("AGPL-3.0-only OR LicenseRef-GPOS-Commercial"));
    }
}

#[cfg(all(test, feature = "std"))]
mod lock_tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn lock_text() -> String {
        fs::read_to_string(repo_root().join("docs/boot/SYS01_WORKING_REFERENCE_LOCK.md"))
            .expect("SYS01_WORKING_REFERENCE_LOCK.md")
    }

    struct LockRow {
        path: String,
        sha256_hex: String,
        len: u64,
    }

    fn parse_lock_v1(text: &str) -> Vec<LockRow> {
        let start = text.find("```lock-v1").expect("lock-v1 fence");
        let after = &text[start + "```lock-v1".len()..];
        let body = after
            .strip_prefix("\r\n")
            .or_else(|| after.strip_prefix('\n'))
            .unwrap_or(after);
        let end = body.find("```").expect("lock-v1 close");
        let mut rows = Vec::new();
        for line in body[..end].lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.split('\t');
            let path = parts.next().expect("path").to_string();
            let sha256_hex = parts.next().expect("sha256").to_ascii_lowercase();
            let len: u64 = parts.next().expect("len").parse().expect("len parse");
            rows.push(LockRow {
                path,
                sha256_hex,
                len,
            });
        }
        rows
    }

    fn hex32(h: [u8; 32]) -> String {
        let mut s = String::with_capacity(64);
        for b in h {
            s.push_str(&format!("{:02x}", b));
        }
        s
    }

    #[test]
    fn every_reference_file_matches_recorded_hash() {
        let rows = parse_lock_v1(&lock_text());
        assert_eq!(rows.len(), 10);
        let root = repo_root();
        for row in &rows {
            let p = root.join(&row.path);
            let bytes = fs::read(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
            assert_eq!(bytes.len() as u64, row.len, "{}", row.path);
            let digest = crate::sha256::sha256(&bytes);
            let got = hex32(digest);
            assert_eq!(got, row.sha256_hex, "{}", row.path);
            if Path::new(&row.path)
                .file_name()
                .and_then(|s| s.to_str())
                == Some("zynq_fsbl.elf")
            {
                assert_eq!(digest, QUARANTINED_FSBL_SHA256);
                assert_eq!(
                    validate_bootloader_artifact("zynq_fsbl.elf", &digest),
                    Err(ArtifactError::FsblFilename)
                );
                assert_eq!(
                    validate_bootloader_artifact("owned_stage1.elf", &digest),
                    Err(ArtifactError::QuarantinedHash)
                );
            }
        }
    }

    #[test]
    fn main_rs_unchanged() {
        let p = repo_root().join("zynq_stage1/src/main.rs");
        let bytes = fs::read(&p).expect("main.rs");
        assert_eq!(bytes.len(), 9580);
        let got = hex32(crate::sha256::sha256(&bytes));
        assert_eq!(got, MAIN_RS_SHA256_HEX);
        let text = core::str::from_utf8(&bytes).expect("utf8");
        assert!(text.contains("pub extern \"C\" fn rust_main()"));
        assert!(text.contains("handoff::jump(app_hdr.exec_addr)"));
    }

    #[test]
    fn utility_elf_entry_is_00100060() {
        let p = repo_root().join("fpga/zynq/release/PZ7020_G1P_PATH1_G64/kairos_zynq.elf");
        let bytes = fs::read(&p).expect("kairos_zynq.elf");
        assert_eq!(elf32_le_entry(&bytes), Some(UTILITY_ENTRY));
    }
}
