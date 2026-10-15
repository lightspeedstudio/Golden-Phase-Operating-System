// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! System 11: file-only UART/evidence classifier. No COM port. No card write.

use crate::sha256::sha256;

pub const EVIDENCE: &str = "fpga/zynq/evidence/kairos_zynq_boot";

pub const PRODUCT_UART: &str =
    "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS10A_20260820/com12_sys10a.uart";
pub const PRODUCT_UART_SHA256: &str =
    "ceb02585476e6b316f94c01c720e4f385ba1f6d86aaf2b1c9a3a67dd3294c64d";
pub const PRODUCT_UART_BYTES: u64 = 7107;

pub const RECOVERY_UART: &str =
    "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS09_20260820/com12_sys09.uart";
pub const RECOVERY_UART_SHA256: &str =
    "391218176cef09b7d642b71f5cca9990dec2d91b95cc73c9879185d9dfa0219d";
pub const RECOVERY_UART_BYTES: u64 = 6994;

pub const AES_UART: &str =
    "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS06_20260820/com12_sys06.uart";
pub const AES_UART_SHA256: &str =
    "ad6b0106577972effe75e0b6597a980b55242cedbf2f650c8b4cd1b63814c95c";
pub const AES_UART_BYTES: u64 = 284;

pub const PRODUCT_ORDER: &[&str] = &[
    "GP-S1 BEGIN",
    "UART EARLY PASS",
    "PS7 CLOCK PASS",
    "DDR BEGIN",
    "DDR PASS",
    "WDT STOP PASS",
    "BOOTMODE SD PASS",
    "SD INIT PASS",
    "IMAGE HEADER PASS",
    "PARTITIONS count=4",
    "DISPATCH RSA",
    "BOOTROM RSA2048 STAGE1 status=NOT_ENFORCED",
    "STAGE1 RSA2048 PARTITION PASS implementation=SOFTWARE",
    "PCAP BEGIN",
    "PL PASS",
    "HANDOFF PASS",
    "JUMP 00100060",
    "gpos>",
];

pub const RECOVERY_ORDER: &[&str] = &[
    "IMAGE HEADER PASS",
    "PARTITIONS count=3",
    "PCAP BEGIN",
    "PL PASS",
    "HANDOFF PASS",
    "JUMP 00100060",
    "gpos>",
];

pub const AES_ORDER: &[&str] = &[
    "IMAGE HEADER PASS",
    "PARTITIONS count=4",
    "ENCRYPTED",
    "P1 KAT PASS",
    "SYS06 HOLD",
];

const HOST_INJECT: &[&[u8]] = &[
    b"SAW_GPOS",
    b"SAW_FAIL",
    b"SAW: ",
    b"=== COM12",
    b"PRESS PZ7020 RESET",
    b"SYS10A_COM12_EMPTY",
    b"SYS10_SIZE_PASS",
];

const FAIL_MARKERS: &[&str] = &[
    "PL FAIL",
    "HANDOFF FAIL",
    "P1 FAIL",
    "DDR FAIL",
    "IMAGE HEADER FAIL",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Product,
    Recovery,
    Aes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Missing,
    Reordered,
    Duplicate,
    HostInject,
    WrongEntry,
    FailMarker,
}

fn hex32(h: [u8; 32]) -> String {
    h.iter().map(|b| format!("{:02x}", b)).collect()
}

fn lines(uart: &[u8]) -> Vec<&str> {
    let mut out = Vec::new();
    for raw in uart.split(|b| *b == b'\n') {
        if let Ok(s) = core::str::from_utf8(raw) {
            out.push(s.trim_end_matches('\r').trim_end());
        }
    }
    out
}

fn ordered_unique(got: &[&str], want: &[&str]) -> Result<(), Error> {
    let mut pos: Vec<usize> = Vec::new();
    for marker in want {
        let hits: Vec<usize> = got
            .iter()
            .enumerate()
            .filter(|(_, l)| **l == *marker)
            .map(|(i, _)| i)
            .collect();
        if hits.is_empty() {
            return Err(Error::Missing);
        }
        if hits.len() > 1 {
            return Err(Error::Duplicate);
        }
        pos.push(hits[0]);
    }
    let mut i = 1usize;
    while i < pos.len() {
        if pos[i] <= pos[i - 1] {
            return Err(Error::Reordered);
        }
        i += 1;
    }
    Ok(())
}

fn gate(uart: &[u8]) -> Result<Vec<&str>, Error> {
    for h in HOST_INJECT {
        if uart.windows(h.len()).any(|w| w == *h) {
            return Err(Error::HostInject);
        }
    }
    if uart.windows(13).any(|w| w == b"JUMP 00100020") {
        return Err(Error::WrongEntry);
    }
    let got = lines(uart);
    for f in FAIL_MARKERS {
        if got.iter().any(|l| *l == *f || l.starts_with(&format!("{f} "))) {
            return Err(Error::FailMarker);
        }
    }
    Ok(got)
}

pub fn classify(uart: &[u8]) -> Result<Kind, Error> {
    let got = gate(uart)?;
    if ordered_unique(&got, PRODUCT_ORDER).is_ok() {
        if got.iter().any(|l| *l == "ENCRYPTED" || *l == "SYS06 HOLD") {
            return Err(Error::HostInject);
        }
        return Ok(Kind::Product);
    }
    if ordered_unique(&got, RECOVERY_ORDER).is_ok() {
        if got.iter().any(|l| *l == "DISPATCH RSA" || *l == "ENCRYPTED") {
            return Err(Error::HostInject);
        }
        return Ok(Kind::Recovery);
    }
    if ordered_unique(&got, AES_ORDER).is_ok() {
        if got.iter().any(|l| l.starts_with("JUMP ") || *l == "PL PASS") {
            return Err(Error::HostInject);
        }
        return Ok(Kind::Aes);
    }
    if let Err(e) = ordered_unique(&got, PRODUCT_ORDER) {
        return Err(e);
    }
    Err(Error::Missing)
}

pub fn product_hash_ok(uart: &[u8]) -> bool {
    uart.len() as u64 == PRODUCT_UART_BYTES && hex32(sha256(uart)) == PRODUCT_UART_SHA256
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn read(rel: &str) -> Vec<u8> {
        fs::read(root().join(rel)).unwrap()
    }

    fn hashed(rel: &str, sha: &str, n: u64) {
        let b = read(rel);
        assert_eq!(b.len() as u64, n, "{rel}");
        assert_eq!(hex32(sha256(&b)), sha, "{rel}");
    }

    #[test]
    fn fixture_hashes_match_reference() {
        hashed(PRODUCT_UART, PRODUCT_UART_SHA256, PRODUCT_UART_BYTES);
        hashed(RECOVERY_UART, RECOVERY_UART_SHA256, RECOVERY_UART_BYTES);
        hashed(AES_UART, AES_UART_SHA256, AES_UART_BYTES);
        hashed(
            "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS08_20260820/com12_sys08.uart",
            "97462f04ce6096ed17c95619c171225dba7d4b3654f011a477ee7c83d388dbcf",
            245,
        );
        hashed(
            "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS08_20260820/com12_sys08_diag.uart",
            "75c20baa18a495fac3e1280b0c30faa51f8f667e41dc351c78d84b6f6060017d",
            557,
        );
        hashed(
            "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS10A_20260820/com12_sys10a_stack_hang.uart",
            "01956933f2715d2e65a608421ce29229c7a021b482800811309d2d8cfc8efe5e",
            185,
        );
        hashed(
            "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS10A_20260820/com12_sys10a_binary_burst.uart",
            "30b77ee0528cd6dfb6ca329733bd91fd8942eb56347b7608fd9db18f36fa4867",
            6988,
        );
        hashed(
            "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS08_20260820/com12_sys08_crcfix.uart",
            "75ee7277e6d554c47ff2f559d6af0752f0e8b816339752e137b5504053d768a7",
            257,
        );
        hashed(
            "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS05_20260819/com12_sys05.uart",
            "b8f7145da7d3cf24c572a06b3e7a61f1b7b6a9a8bc281e366a1502da0665d3d8",
            258,
        );
        hashed(
            "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS04_20260819/com12_sys04.uart",
            "d298dc4bf6cc00031372577df32f9c6069a2b17f73c0f06b4e2a3fcef9f9e78b",
            195,
        );
        hashed(
            "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS03_20260819/com12_sys03.uart",
            "8fcdc3257f3449a9114594474d0e34d8e71540348081da82336f819f132f0dee",
            112,
        );
        hashed(
            "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS07_20260820/com12_sys07.uart",
            "6fd8675e4d477f4e5507256633be8e315c7eeb5fad1f6c925754df370dfe4bdc",
            329,
        );
        hashed(
            "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS07_20260820/com12_sys07_instr.uart",
            "4377e3018a48789f9caf6ac359db87e932314a6c9e6a8471bf3df75669f5063f",
            528,
        );
        let doc = fs::read_to_string(root().join("docs/boot/SYS11_UART_REFERENCE.md")).unwrap();
        assert!(doc.contains(PRODUCT_UART_SHA256));
        assert!(doc.contains("no COM12 UART exists for DDR FAIL"));
        assert!(doc.contains("no COM12 UART exists for ADMA wait FAIL"));
    }

    #[test]
    fn product_pass_is_sys10a_uart() {
        let u = read(PRODUCT_UART);
        assert!(product_hash_ok(&u));
        assert_eq!(classify(&u), Ok(Kind::Product));
        assert_ne!(classify(&u), Ok(Kind::Recovery));
        assert_ne!(classify(&u), Ok(Kind::Aes));
    }

    #[test]
    fn recovery_and_aes_stay_separate() {
        assert_eq!(classify(&read(RECOVERY_UART)), Ok(Kind::Recovery));
        assert_eq!(classify(&read(AES_UART)), Ok(Kind::Aes));
        assert_ne!(classify(&read(RECOVERY_UART)), Ok(Kind::Product));
        assert_ne!(classify(&read(AES_UART)), Ok(Kind::Product));
        assert_ne!(classify(&read(AES_UART)), Ok(Kind::Recovery));
    }

    #[test]
    fn real_failure_transcripts_are_rejected() {
        let fails = [
            "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS08_20260820/com12_sys08.uart",
            "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS08_20260820/com12_sys08_diag.uart",
            "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS10A_20260820/com12_sys10a_stack_hang.uart",
            "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS10A_20260820/com12_sys10a_binary_burst.uart",
            "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS08_20260820/com12_sys08_crcfix.uart",
            "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS05_20260819/com12_sys05.uart",
            "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS04_20260819/com12_sys04.uart",
            "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS03_20260819/com12_sys03.uart",
            "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS07_20260820/com12_sys07.uart",
            "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS07_20260820/com12_sys07_instr.uart",
        ];
        for rel in fails {
            let u = read(rel);
            assert_ne!(classify(&u), Ok(Kind::Product), "{rel}");
            assert!(classify(&u).is_err(), "{rel} {:?}", classify(&u));
        }
        assert_eq!(
            classify(&read(
                "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS08_20260820/com12_sys08.uart"
            )),
            Err(Error::FailMarker)
        );
        assert_eq!(
            classify(&read(
                "fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS08_20260820/com12_sys08_diag.uart"
            )),
            Err(Error::FailMarker)
        );
    }

    #[test]
    fn require_jump_00100060_reject_00100020() {
        let mut u = read(PRODUCT_UART);
        let i = u
            .windows(13)
            .position(|w| w == b"JUMP 00100060")
            .expect("product UART JUMP 00100060");
        u[i..i + 13].copy_from_slice(b"JUMP 00100020");
        assert_eq!(classify(&u), Err(Error::WrongEntry));
        assert!(!product_hash_ok(&u));
    }

    fn replace_once(src: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
        match src.windows(from.len()).position(|w| w == from) {
            None => panic!("missing {:?}", from),
            Some(i) => {
                let mut out = Vec::with_capacity(src.len() - from.len() + to.len());
                out.extend_from_slice(&src[..i]);
                out.extend_from_slice(to);
                out.extend_from_slice(&src[i + from.len()..]);
                out
            }
        }
    }

    #[test]
    fn reorder_duplicate_and_host_inject_fail() {
        let src = read(PRODUCT_UART);
        let swapped = replace_once(&src, b"PCAP BEGIN\r\nPL PASS", b"PL PASS\r\nPCAP BEGIN");
        assert_eq!(classify(&swapped), Err(Error::Reordered));
        let dup = replace_once(&src, b"PL PASS\r\n", b"PL PASS\r\nPL PASS\r\n");
        assert_eq!(classify(&dup), Err(Error::Duplicate));
        let mut injected = src.clone();
        injected.extend_from_slice(b"\r\nSAW_GPOS\r\n");
        assert_eq!(classify(&injected), Err(Error::HostInject));
        let stale = replace_once(
            &src,
            b"JUMP 00100060",
            b"JUMP 00100060\r\nJUMP 00100060",
        );
        assert_eq!(classify(&stale), Err(Error::Duplicate));
    }

    #[test]
    fn harness_source_never_opens_a_port() {
        let src = include_str!("sys11.rs");
        let needle = |a: &str, b: &str| format!("{a}{b}");
        assert!(!src.contains(&needle("Serial", "Port")));
        assert!(!src.contains(&needle("new_with_", "dummy")));
        assert!(!src.contains(&needle("IO.", "Ports")));
        assert!(root().join(EVIDENCE).is_dir());
    }
}
