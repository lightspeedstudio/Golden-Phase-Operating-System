// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! System 7: software RSA-2048 later-partition verify. BootROM RSA not enforced.

use crate::bootimg::parse::{parse_all_rsa_later, ParsedHeaders, PartitionHeader, ATTR_RSA, HEADER_WINDOW_LEN};
use crate::rsa::cert::verify_later_partition;
use crate::rsa::AC_LEN;
#[cfg(not(feature = "std"))]
use crate::rsa::cert::validate_cert;
#[cfg(not(feature = "std"))]
use crate::rsa::{verify_pkcs1_sha256, verify_pkcs1_sha256_digest, E_ZYNQ, PART_SIG_OFF, SPK_OFF, SPK_SIG_OFF};
#[cfg(not(feature = "std"))]
use crate::sha256::sha256_concat;
use crate::sd::bus::SdBus;
use crate::sd::sys04::BOOT16_PHYS;
#[cfg(not(feature = "std"))]
use crate::sd::sys04::FAT_PASS_PREFIX;
use crate::sd::sys06::{BOOT_CAP, P1_DEST};
#[cfg(not(feature = "std"))]
use crate::sd::sys06::load_boot_full_on;
#[cfg(not(feature = "std"))]
use crate::sd::SdCard;

pub const KAT_LEN: usize = 256;
pub const BOOTROM_RSA_LINE: &str = "BOOTROM RSA2048 STAGE1 status=NOT_ENFORCED\n";
pub const PARTITION_RSA_PASS: &str = "STAGE1 RSA2048 PARTITION PASS implementation=SOFTWARE\n";
pub const HOLD_LINE: &str = "SYS07 HOLD\n";
#[cfg(not(feature = "std"))]
const CANARY: u32 = 0xC0DE_5A7E;

#[cfg(not(feature = "std"))]
static mut SYS07_SCRATCH_CANARY: u32 = CANARY;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Bounds,
    Parse,
    Adma,
    Pio,
    Auth,
    Kat,
    Canary,
}

#[cfg(not(feature = "std"))]
fn map_sys06(e: crate::sd::sys06::Error) -> Error {
    match e {
        crate::sd::sys06::Error::Bounds => Error::Bounds,
        crate::sd::sys06::Error::Pio => Error::Pio,
        _ => Error::Adma,
    }
}

fn loaded_headers<B: SdBus>(bus: &mut B, file_len: u32) -> Result<ParsedHeaders, Error> {
    #[cfg(feature = "std")]
    {
        let mut window = [0u8; HEADER_WINDOW_LEN];
        let n = core::cmp::min(HEADER_WINDOW_LEN, file_len as usize);
        bus.phys_read(BOOT16_PHYS, &mut window[..n]);
        parse_all_rsa_later(&window[..n], n, file_len as usize).map_err(|_| Error::Parse)
    }
    #[cfg(not(feature = "std"))]
    {
        let _ = bus;
        let n = core::cmp::min(HEADER_WINDOW_LEN, file_len as usize);
        let window = unsafe { core::slice::from_raw_parts(BOOT16_PHYS as *const u8, n) };
        parse_all_rsa_later(window, n, file_len as usize).map_err(|_| Error::Parse)
    }
}

fn locate_p1(h: &ParsedHeaders, file_len: u32) -> Result<PartitionHeader, Error> {
    if h.partition_count < 2 {
        return Err(Error::Parse);
    }
    if (h.partitions[0].attributes & ATTR_RSA) != 0 {
        return Err(Error::Parse);
    }
    let mut found = None;
    for i in 1..h.partition_count {
        let p = h.partitions[i];
        if p.is_ps() && !p.is_pl() && p.load_addr == P1_DEST && (p.attributes & ATTR_RSA) != 0 {
            found = Some(p);
            break;
        }
    }
    let p = found.ok_or(Error::Parse)?;
    if p.enc_words != p.data_words || p.exec_addr != 0 {
        return Err(Error::Parse);
    }
    if p.byte_len() != KAT_LEN {
        return Err(Error::Parse);
    }
    if p.stored_len() != KAT_LEN + AC_LEN {
        return Err(Error::Parse);
    }
    let off = u32::try_from(p.data_off).map_err(|_| Error::Bounds)?;
    let n = u32::try_from(p.stored_len()).map_err(|_| Error::Bounds)?;
    if off.checked_add(n).ok_or(Error::Bounds)? > file_len {
        return Err(Error::Bounds);
    }
    Ok(p)
}

fn copy_p1<B: SdBus>(bus: &mut B, p: PartitionHeader) -> Result<(), Error> {
    let mut buf = [0u8; KAT_LEN];
    bus.phys_read(BOOT16_PHYS + p.data_off as u32, &mut buf);
    bus.phys_write(P1_DEST, &buf);
    Ok(())
}

fn verify_p1<B: SdBus>(bus: &mut B, p: PartitionHeader) -> Result<(), Error> {
    #[cfg(feature = "std")]
    {
        let mut payload = [0u8; KAT_LEN];
        let mut ac = [0u8; AC_LEN];
        bus.phys_read(BOOT16_PHYS + p.data_off as u32, &mut payload);
        bus.phys_read(BOOT16_PHYS + p.data_off as u32 + KAT_LEN as u32, &mut ac);
        verify_later_partition(&payload, &ac).map_err(|_| Error::Auth)
    }
    #[cfg(not(feature = "std"))]
    {
        let _ = bus;
        let src = BOOT16_PHYS + p.data_off as u32;
        let payload = unsafe { core::slice::from_raw_parts(src as *const u8, KAT_LEN) };
        let ac = unsafe { core::slice::from_raw_parts((src + KAT_LEN as u32) as *const u8, AC_LEN) };
        verify_later_partition(payload, ac).map_err(|_| Error::Auth)
    }
}

fn verify_kat<B: SdBus>(bus: &mut B) -> Result<(), Error> {
    let mut got = [0u8; KAT_LEN];
    bus.phys_read(P1_DEST, &mut got);
    let mut i = 0usize;
    while i < KAT_LEN {
        if got[i] != i as u8 {
            return Err(Error::Kat);
        }
        i += 1;
    }
    Ok(())
}

pub fn p1_rsa_kat<B: SdBus>(bus: &mut B, file_len: u32) -> Result<(), Error> {
    if file_len < HEADER_WINDOW_LEN as u32 && file_len < 0x1700 {
        return Err(Error::Bounds);
    }
    if file_len > BOOT_CAP {
        return Err(Error::Bounds);
    }
    let h = loaded_headers(bus, file_len)?;
    let p = locate_p1(&h, file_len)?;
    verify_p1(bus, p)?;
    copy_p1(bus, p)?;
    verify_kat(bus)
}

#[cfg(not(feature = "std"))]
fn canaries_ok(c_lo: &u32, c_hi: &u32) -> bool {
    let scratch = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(SYS07_SCRATCH_CANARY)) };
    let lo = unsafe { core::ptr::read_volatile(c_lo) };
    let hi = unsafe { core::ptr::read_volatile(c_hi) };
    lo == CANARY && hi == CANARY && scratch == CANARY
}

#[cfg(not(feature = "std"))]
fn take256(b: &[u8], off: usize) -> Result<[u8; 256], Error> {
    let s = b.get(off..off + 256).ok_or(Error::Auth)?;
    let mut o = [0u8; 256];
    o.copy_from_slice(s);
    Ok(o)
}

#[cfg(not(feature = "std"))]
fn write_ticks(io: &mut crate::platform::mmio::DeviceIo, dt: u32) {
    use crate::platform::uart;
    use crate::sd::sys04::fmt_dec_u32;
    uart::write_str(io, "RSA MODEXP PASS ticks=");
    let mut digits = [0u8; 10];
    let d = fmt_dec_u32(dt, &mut digits);
    if let Ok(s) = core::str::from_utf8(d) {
        uart::write_str(io, s);
    }
    uart::write_str(io, "\n");
}

/// Device verify: AC/payload stay in the DDR image window. No 1728-byte stack copy.
#[cfg(not(feature = "std"))]
fn sys07_rsa_traced(
    io: &mut crate::platform::mmio::DeviceIo,
    bus: &mut crate::sd::bus::DeviceSd,
    file_len: u32,
) -> Result<(), Error> {
    use crate::platform::mmio::{Io, GTIMER_COUNT_L};
    use crate::platform::uart;
    let mut c_lo = 0u32;
    let mut c_hi = 0u32;
    unsafe {
        core::ptr::write_volatile(&mut c_lo, CANARY);
        core::ptr::write_volatile(&mut c_hi, CANARY);
        core::ptr::write_volatile(core::ptr::addr_of_mut!(SYS07_SCRATCH_CANARY), CANARY);
    }
    uart::write_str(io, "RSA PARSE BEGIN\n");
    if !canaries_ok(&c_lo, &c_hi) {
        uart::write_str(io, "CANARY FAIL\n");
        return Err(Error::Canary);
    }
    if file_len < HEADER_WINDOW_LEN as u32 && file_len < 0x1700 {
        return Err(Error::Bounds);
    }
    if file_len > BOOT_CAP {
        return Err(Error::Bounds);
    }
    let h = loaded_headers(bus, file_len)?;
    let p = locate_p1(&h, file_len)?;
    let src = BOOT16_PHYS + p.data_off as u32;
    let payload = unsafe { core::slice::from_raw_parts(src as *const u8, KAT_LEN) };
    let ac = unsafe { core::slice::from_raw_parts((src + KAT_LEN as u32) as *const u8, AC_LEN) };
    validate_cert(ac).map_err(|_| Error::Auth)?;
    if !canaries_ok(&c_lo, &c_hi) {
        uart::write_str(io, "CANARY FAIL\n");
        return Err(Error::Canary);
    }
    uart::write_str(io, "RSA PARSE PASS\n");

    uart::write_str(io, "RSA HASH BEGIN\n");
    let hash = sha256_concat(payload, &ac[..PART_SIG_OFF]);
    if !canaries_ok(&c_lo, &c_hi) {
        uart::write_str(io, "CANARY FAIL\n");
        return Err(Error::Canary);
    }
    uart::write_str(io, "RSA HASH PASS\n");

    uart::write_str(io, "RSA MODEXP BEGIN\n");
    let t0 = io.read(GTIMER_COUNT_L);
    let n = take256(ac, 0x40)?;
    let sig = take256(ac, SPK_SIG_OFF)?;
    verify_pkcs1_sha256(&n, E_ZYNQ, &sig, &ac[SPK_OFF..SPK_SIG_OFF]).map_err(|_| Error::Auth)?;
    let t1 = io.read(GTIMER_COUNT_L);
    write_ticks(io, t1.wrapping_sub(t0));
    if !canaries_ok(&c_lo, &c_hi) {
        uart::write_str(io, "CANARY FAIL\n");
        return Err(Error::Canary);
    }

    uart::write_str(io, "RSA MODEXP BEGIN\n");
    let t2 = io.read(GTIMER_COUNT_L);
    let n2 = take256(ac, SPK_OFF)?;
    let sig2 = take256(ac, PART_SIG_OFF)?;
    verify_pkcs1_sha256_digest(&n2, E_ZYNQ, &sig2, &hash).map_err(|_| Error::Auth)?;
    let t3 = io.read(GTIMER_COUNT_L);
    write_ticks(io, t3.wrapping_sub(t2));
    if !canaries_ok(&c_lo, &c_hi) {
        uart::write_str(io, "CANARY FAIL\n");
        return Err(Error::Canary);
    }
    uart::write_str(io, "RSA VERIFY PASS\n");

    copy_p1(bus, p)?;
    if !canaries_ok(&c_lo, &c_hi) {
        uart::write_str(io, "CANARY FAIL\n");
        return Err(Error::Canary);
    }
    uart::write_str(io, "P1 COPY PASS\n");
    verify_kat(bus)?;
    let _ = (c_lo, c_hi);
    Ok(())
}

#[cfg(all(not(feature = "std"), feature = "sys07", not(feature = "sys08"), not(feature = "sys09"), not(feature = "sys10a")))]
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    let mut io = crate::platform::mmio::DeviceIo;
    let _ = sys07_device(&mut io);
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}

#[cfg(not(feature = "std"))]
pub fn sys07_device(io: &mut crate::platform::mmio::DeviceIo) -> Result<(), u32> {
    use crate::platform::entry::sys03_platform_ready;
    use crate::platform::uart;
    use crate::sd::bus::DeviceSd;
    use crate::sd::sys04::fmt_dec_u32;
    sys03_platform_ready(io)?;
    let mut bus = DeviceSd;
    let mut card = SdCard::init(&mut bus).map_err(|_| 0xE2u32)?;
    uart::write_str(io, "SD INIT PASS\n");
    let size = load_boot_full_on(&mut card, &mut bus).map_err(map_sys06).map_err(|_| 0xE2u32)?;
    uart::write_str(io, FAT_PASS_PREFIX);
    let mut digits = [0u8; 10];
    let d = fmt_dec_u32(size, &mut digits);
    let s = core::str::from_utf8(d).map_err(|_| 0xE2u32)?;
    uart::write_str(io, s);
    uart::write_str(io, "\n");
    uart::write_str(io, "SD ADMA2 CMD18 PASS bytes=");
    uart::write_str(io, s);
    uart::write_str(io, "\n");
    let count = loaded_headers(&mut bus, size).map_err(|_| 0xE3u32)?.partition_count;
    uart::write_str(io, "IMAGE HEADER PASS\n");
    uart::write_str(io, "PARTITIONS count=");
    let c = fmt_dec_u32(count as u32, &mut digits);
    let cs = core::str::from_utf8(c).map_err(|_| 0xE3u32)?;
    uart::write_str(io, cs);
    uart::write_str(io, "\n");
    uart::write_str(io, BOOTROM_RSA_LINE);
    match sys07_rsa_traced(io, &mut bus, size) {
        Ok(()) => uart::write_str(io, PARTITION_RSA_PASS),
        Err(Error::Canary) => {
            uart::write_str(io, "CANARY FAIL\n");
            return Err(0xE7);
        }
        Err(_) => {
            uart::write_str(io, "P1 FAIL\n");
            return Err(0xE7);
        }
    }
    uart::write_str(io, HOLD_LINE);
    Ok(())
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::bootimg::pack::{pack_rsa_later, Part};
    use crate::bootimg::parse::{parse, ATTR_PS, ParseError};
    use crate::sd::bus::HostSd;

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

    fn packed() -> Vec<u8> {
        let mut kat = [0u8; 256];
        for i in 0..256 {
            kat[i] = i as u8;
        }
        let n = wp_le("00b3510a2bcd4ce644c5b594ae5059e12b2f054b658d5da5959a2fdf1871b808bc3df3e628d2792e51aad5c124b43bda453dca5cde4bcf28e7bd4effba0cb4b742bbb6d5a013cb63d1aa3a89e02627ef5398b52c0cfd97d208abeb8d7c9bce0bbeb019a86ddb589beb29a5b74bf861075c677c81d430f030c265247af9d3c9140ccb65309d07e0adc1efd15cf17e7b055d7da3868e4648cc3a180f0ee7f8e1e7b18098a3391b4ce7161e98d57af8a947e201a463e2d6bbca8059e5706e9dfed8f4856465ffa712ed1aa18e888d12dc6aa09ce95ecfca83cc5b0b15db09c8647f5d524c0f2e7620a3416b9623cadc0f097af573261c98c8400aa12af38e43cad84d");
        let d = wp_le("1a502d0eea6c7b69e21d5839101f705456ed0ef852fb47fe21071f54c5f33c8ceb066c62d727e32d26c58137329f89d3195325b795264c195d85472f7507dbd0961d2951f935a26b34f0ac24d15490e1128a9b7138915bc7dbfa8fe396357131c543ae9c98507368d9ceb08c1c6198a3eda7aea185a0e976cd42c22d00f003d9f19d96ea4c9afcbfe1441ccc802cfb0689f59d804c6a4e4f404c15174745ed6cb8bc88ef0b33ba0d2a80e35e43bc90f350052e72016e75b00d357a381c9c0d467069ca660887c987766349fcc43460b4aa516bce079edd87ba164307b752c277ed9528ad3ba0bf1877349ed3b7966a6c240110409bf4d0fade0c68fdadd847fd");
        pack_rsa_later(
            Part {
                name: "sys07_plat.bin",
                data: &[0x11, 0x22, 0x33, 0x44],
                load_addr: 0,
                exec_addr: 0,
                attributes: ATTR_PS,
            },
            Part {
                name: "ddr_kat.bin",
                data: &kat,
                load_addr: P1_DEST,
                exec_addr: 0,
                attributes: ATTR_PS,
            },
            &n,
            &d,
            &n,
            &d,
        )
        .unwrap()
    }

    #[test]
    fn host_p1_rsa_kat_and_malformed_fail() {
        let img = packed();
        assert_eq!(parse(&img), Err(ParseError::VendorRsa));
        let mut h = HostSd::new();
        h.phys_write(BOOT16_PHYS, &img);
        p1_rsa_kat(&mut h, img.len() as u32).unwrap();
        let mut kat = [0u8; KAT_LEN];
        h.phys_read(P1_DEST, &mut kat);
        for i in 0..KAT_LEN {
            assert_eq!(kat[i], i as u8);
        }
        let parsed = loaded_headers(&mut h, img.len() as u32).unwrap();
        let p = locate_p1(&parsed, img.len() as u32).unwrap();
        let mut bad = img.clone();
        bad[p.data_off + KAT_LEN + crate::rsa::PART_SIG_OFF] ^= 1;
        let mut h_bad = HostSd::new();
        let poison = [0xAAu8; KAT_LEN];
        h_bad.phys_write(P1_DEST, &poison);
        h_bad.phys_write(BOOT16_PHYS, &bad);
        assert_eq!(p1_rsa_kat(&mut h_bad, bad.len() as u32), Err(Error::Auth));
        let mut dest = [0u8; KAT_LEN];
        h_bad.phys_read(P1_DEST, &mut dest);
        assert_eq!(dest, poison);
        let src = include_str!("sys07.rs");
        let prod = src.split("#[cfg(test)]").next().unwrap();
        let boot = prod.find(BOOTROM_RSA_LINE.trim_end()).expect("bootrom");
        let pass = prod.find(PARTITION_RSA_PASS.trim_end()).expect("pass");
        let hold = prod.find(HOLD_LINE.trim_end()).expect("hold");
        assert!(boot < pass && pass < hold);
        assert!(prod.contains("RSA PARSE BEGIN"));
        assert!(prod.contains("RSA HASH BEGIN"));
        assert!(prod.contains("RSA MODEXP BEGIN"));
        assert!(prod.contains("RSA VERIFY PASS"));
        assert!(prod.contains("P1 COPY PASS"));
        assert!(prod.contains("CANARY FAIL"));
        let gi = include_str!("../../.gitignore");
        assert!(gi.contains("/build/oracle/"));
        assert!(gi.contains("*.pem"));
        let spec = include_str!("../../../../docs/boot/SYS07_RSA_REFERENCE.md");
        assert!(!spec.contains("-----"));
        assert!(spec.contains(BOOTROM_RSA_LINE.trim_end()));
        assert!(spec.contains(PARTITION_RSA_PASS.trim_end()));
        assert!(spec.contains(HOLD_LINE.trim_end()));
    }
}
