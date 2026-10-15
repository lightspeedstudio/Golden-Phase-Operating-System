// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! System 10A: plaintext / encrypted / RSA product dispatcher in one Stage 1.

use crate::bootimg::frame::ENC_STATUS_BBRAM;
use crate::bootimg::parse::{
    parse_all_product, parse_all_rsa_later, ATTR_RSA, HEADER_WINDOW_LEN,
};
use crate::sd::bus::SdBus;
use crate::sd::sys04::BOOT16_PHYS;
use crate::sd::sys06::BOOT_CAP;
#[cfg(not(feature = "std"))]
use crate::sd::sys04::FAT_PASS_PREFIX;
#[cfg(not(feature = "std"))]
use crate::sd::sys06::load_boot_full_on;
#[cfg(not(feature = "std"))]
use crate::sd::sys08::{write_diag, PCAP_BEGIN_LINE, PL_PASS_LINE};
#[cfg(not(feature = "std"))]
use crate::sd::SdCard;
#[cfg(not(feature = "std"))]
use crate::sd::sys06::p1_decrypt_kat;
#[cfg(not(feature = "std"))]
use crate::sd::sys07::{p1_rsa_kat, BOOTROM_RSA_LINE, PARTITION_RSA_PASS};
#[cfg(not(feature = "std"))]
use crate::sd::sys08::pcap_pl;

pub const DISPATCH_PLAIN: &str = "DISPATCH PLAIN\n";
pub const DISPATCH_ENC: &str = "DISPATCH ENC\n";
pub const DISPATCH_RSA: &str = "DISPATCH RSA\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Plaintext,
    Encrypted,
    RsaProduct,
    RsaKat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    pub kind: Kind,
    pub partition_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Bounds,
    Parse,
}

fn enc_status(window: &[u8]) -> Result<u32, Error> {
    let s = window.get(0x28..0x2C).ok_or(Error::Parse)?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn later_rsa(h: &crate::bootimg::parse::ParsedHeaders) -> bool {
    let mut i = 1usize;
    while i < h.partition_count {
        if (h.partitions[i].attributes & ATTR_RSA) != 0 {
            return true;
        }
        i += 1;
    }
    false
}

fn classify_bytes(window: &[u8], file_len: u32) -> Result<Decision, Error> {
    if file_len < 0x2C || file_len > BOOT_CAP {
        return Err(Error::Bounds);
    }
    let n = window.len();
    if n == 0 || n > HEADER_WINDOW_LEN {
        return Err(Error::Bounds);
    }
    if enc_status(window)? == ENC_STATUS_BBRAM {
        let h = crate::bootimg::frame::parse_all_encrypted(window, n, file_len as usize)
            .map_err(|_| Error::Parse)?;
        return Ok(Decision {
            kind: Kind::Encrypted,
            partition_count: h.partition_count,
        });
    }
    match parse_all_product(window, n, file_len as usize) {
        Ok(h) => {
            let kind = if later_rsa(&h) {
                Kind::RsaProduct
            } else {
                Kind::Plaintext
            };
            Ok(Decision {
                kind,
                partition_count: h.partition_count,
            })
        }
        Err(_) => {
            let h = parse_all_rsa_later(window, n, file_len as usize).map_err(|_| Error::Parse)?;
            if !later_rsa(&h) {
                return Err(Error::Parse);
            }
            Ok(Decision {
                kind: Kind::RsaKat,
                partition_count: h.partition_count,
            })
        }
    }
}

pub fn classify<B: SdBus>(bus: &mut B, file_len: u32) -> Result<Decision, Error> {
    let n = core::cmp::min(HEADER_WINDOW_LEN, file_len as usize);
    #[cfg(feature = "std")]
    {
        let mut window = [0u8; HEADER_WINDOW_LEN];
        bus.phys_read(BOOT16_PHYS, &mut window[..n]);
        classify_bytes(&window[..n], file_len)
    }
    #[cfg(not(feature = "std"))]
    {
        let _ = bus;
        let window = unsafe { core::slice::from_raw_parts(BOOT16_PHYS as *const u8, n) };
        classify_bytes(window, file_len)
    }
}

#[cfg(all(not(feature = "std"), feature = "sys10a"))]
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    let mut io = crate::platform::mmio::DeviceIo;
    let _ = sys10a_device(&mut io);
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}

#[cfg(not(feature = "std"))]
fn emit(io: &mut crate::platform::mmio::DeviceIo, s: &str) {
    crate::platform::uart::write_str(io, s);
    crate::platform::uart::wait_tx_empty(io);
}

#[cfg(not(feature = "std"))]
fn jump_app(io: &mut crate::platform::mmio::DeviceIo, entry: u32) -> ! {
    use crate::handoff::{self, HANDOFF_PASS_LINE};
    use crate::platform::uart;
    use crate::sd::sys04::fmt_hex_u32;
    emit(io, HANDOFF_PASS_LINE);
    uart::write_str(io, "JUMP ");
    let mut hex = [0u8; 8];
    let h = fmt_hex_u32(entry, &mut hex);
    if let Ok(s) = core::str::from_utf8(h) {
        uart::write_str(io, s);
    }
    uart::write_str(io, "\n");
    uart::wait_tx_empty(io);
    unsafe { handoff::jump(entry) }
}

#[cfg(not(feature = "std"))]
pub fn sys10a_device(io: &mut crate::platform::mmio::DeviceIo) -> Result<(), u32> {
    use crate::handoff::{self, HANDOFF_FAIL_LINE};
    use crate::platform::entry::sys03_platform_ready;
    use crate::sd::bus::DeviceSd;
    use crate::sd::sys04::fmt_dec_u32;
    sys03_platform_ready(io)?;
    let mut bus = DeviceSd;
    let mut card = SdCard::init(&mut bus).map_err(|_| 0xE2u32)?;
    emit(io, "SD INIT PASS\n");
    let size = load_boot_full_on(&mut card, &mut bus).map_err(|_| 0xE2u32)?;
    emit(io, FAT_PASS_PREFIX);
    let mut digits = [0u8; 10];
    let d = fmt_dec_u32(size, &mut digits);
    let s = core::str::from_utf8(d).map_err(|_| 0xE2u32)?;
    emit(io, s);
    emit(io, "\n");
    emit(io, "SD ADMA2 CMD18 PASS bytes=");
    emit(io, s);
    emit(io, "\n");
    let decision = classify(&mut bus, size).map_err(|_| 0xE3u32)?;
    emit(io, "IMAGE HEADER PASS\n");
    emit(io, "PARTITIONS count=");
    let c = fmt_dec_u32(decision.partition_count as u32, &mut digits);
    let cs = core::str::from_utf8(c).map_err(|_| 0xE3u32)?;
    emit(io, cs);
    emit(io, "\n");
    match decision.kind {
        Kind::Encrypted => {
            emit(io, DISPATCH_ENC);
            match p1_decrypt_kat(&mut bus, io, size) {
                Ok(()) => emit(io, "P1 KAT PASS\n"),
                Err(_) => {
                    emit(io, "P1 FAIL\n");
                    return Err(0xE5);
                }
            }
            Ok(())
        }
        Kind::RsaKat => {
            emit(io, DISPATCH_RSA);
            emit(io, BOOTROM_RSA_LINE);
            match p1_rsa_kat(&mut bus, size) {
                Ok(()) => emit(io, PARTITION_RSA_PASS),
                Err(_) => {
                    emit(io, "P1 FAIL\n");
                    return Err(0xE7);
                }
            }
            Ok(())
        }
        Kind::RsaProduct => {
            emit(io, DISPATCH_RSA);
            emit(io, BOOTROM_RSA_LINE);
            match p1_rsa_kat(&mut bus, size) {
                Ok(()) => emit(io, PARTITION_RSA_PASS),
                Err(_) => {
                    emit(io, "P1 FAIL\n");
                    return Err(0xE7);
                }
            }
            emit(io, PCAP_BEGIN_LINE);
            match pcap_pl(&mut bus, io, size) {
                Ok(()) => emit(io, PL_PASS_LINE),
                Err((e, d)) => {
                    write_diag(io, &d, e);
                    crate::platform::uart::wait_tx_empty(io);
                    return Err(0xE8);
                }
            }
            match handoff::working_entry(&mut bus, size) {
                Ok(entry) => jump_app(io, entry),
                Err(_) => {
                    emit(io, HANDOFF_FAIL_LINE);
                    Err(0xE9)
                }
            }
        }
        Kind::Plaintext => {
            emit(io, DISPATCH_PLAIN);
            emit(io, PCAP_BEGIN_LINE);
            match pcap_pl(&mut bus, io, size) {
                Ok(()) => emit(io, PL_PASS_LINE),
                Err((e, d)) => {
                    write_diag(io, &d, e);
                    crate::platform::uart::wait_tx_empty(io);
                    return Err(0xE8);
                }
            }
            match handoff::working_entry(&mut bus, size) {
                Ok(entry) => jump_app(io, entry),
                Err(_) => {
                    emit(io, HANDOFF_FAIL_LINE);
                    Err(0xE9)
                }
            }
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::bootimg::pack::{pack, pack_rsa_later, pack_rsa_later_product, pack_secure_image, Part};
    use crate::bootimg::parse::{parse_all, ATTR_PL, ATTR_PS, ParseError};
    use crate::bootimg::stream::StreamKeys;
    use crate::devcfg::regs::{
        CTRL, DMA_DONE, DMA_DST, DMA_DST_LEN, DMA_Q_EMPTY, INT_STS, LOOPBACK, MCTRL, PCFG_DONE,
        PCFG_INIT, PL_DEST, POR_B, PROG_B, RATE_EN, STATUS,
    };
    use crate::handoff::UTIL_LOAD;
    use crate::platform::mmio::Io;
    use crate::sd::sys06::{p1_decrypt_kat, P1_DEST};
    use crate::sd::sys07::{p1_rsa_kat, KAT_LEN};
    use crate::sd::sys08::pcap_pl;
    use crate::UTILITY_ENTRY;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;

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

    fn wp() -> ([u8; 256], [u8; 256]) {
        (
            wp_le("00b3510a2bcd4ce644c5b594ae5059e12b2f054b658d5da5959a2fdf1871b808bc3df3e628d2792e51aad5c124b43bda453dca5cde4bcf28e7bd4effba0cb4b742bbb6d5a013cb63d1aa3a89e02627ef5398b52c0cfd97d208abeb8d7c9bce0bbeb019a86ddb589beb29a5b74bf861075c677c81d430f030c265247af9d3c9140ccb65309d07e0adc1efd15cf17e7b055d7da3868e4648cc3a180f0ee7f8e1e7b18098a3391b4ce7161e98d57af8a947e201a463e2d6bbca8059e5706e9dfed8f4856465ffa712ed1aa18e888d12dc6aa09ce95ecfca83cc5b0b15db09c8647f5d524c0f2e7620a3416b9623cadc0f097af573261c98c8400aa12af38e43cad84d"),
            wp_le("1a502d0eea6c7b69e21d5839101f705456ed0ef852fb47fe21071f54c5f33c8ceb066c62d727e32d26c58137329f89d3195325b795264c195d85472f7507dbd0961d2951f935a26b34f0ac24d15490e1128a9b7138915bc7dbfa8fe396357131c543ae9c98507368d9ceb08c1c6198a3eda7aea185a0e976cd42c22d00f003d9f19d96ea4c9afcbfe1441ccc802cfb0689f59d804c6a4e4f404c15174745ed6cb8bc88ef0b33ba0d2a80e35e43bc90f350052e72016e75b00d357a381c9c0d467069ca660887c987766349fcc43460b4aa516bce079edd87ba164307b752c277ed9528ad3ba0bf1877349ed3b7966a6c240110409bf4d0fade0c68fdadd847fd"),
        )
    }

    fn kat() -> [u8; 256] {
        let mut k = [0u8; 256];
        for i in 0..256 {
            k[i] = i as u8;
        }
        k
    }

    fn s1() -> Part<'static> {
        Part {
            name: "s1.bin",
            data: &[0x11, 0x22, 0x33, 0x44],
            load_addr: 0,
            exec_addr: 0,
            attributes: ATTR_PS,
        }
    }

    fn bit() -> Vec<u8> {
        let mut b = vec![0u8; 253];
        b.extend_from_slice(&[0xAA, 0x99, 0x55, 0x66, 0x20, 0x00, 0x00, 0x00]);
        b
    }

    fn app() -> Part<'static> {
        Part {
            name: "app.bin",
            data: &[0xE0, 0xE1, 0xE2, 0xE3],
            load_addr: UTIL_LOAD,
            exec_addr: UTILITY_ENTRY,
            attributes: ATTR_PS,
        }
    }

    fn stream_keys() -> StreamKeys {
        let mut aes = [0u8; 32];
        let mut iv = [0u8; 16];
        let mut mac = [0u8; 32];
        for i in 0..32 {
            aes[i] = i as u8;
            mac[i] = 0xA0u8.wrapping_add(i as u8);
        }
        for i in 0..16 {
            iv[i] = 0x50u8.wrapping_add(i as u8);
        }
        StreamKeys { aes, iv, mac }
    }

    struct Fake {
        mem: BTreeMap<u32, u32>,
        ram: Rc<RefCell<BTreeMap<u32, u8>>>,
    }

    impl Fake {
        fn new(ram: Rc<RefCell<BTreeMap<u32, u8>>>) -> Self {
            let mut mem = BTreeMap::new();
            mem.insert(CTRL, crate::devcfg::regs::PCAP_MODE);
            mem.insert(MCTRL, POR_B | LOOPBACK);
            mem.insert(STATUS, PCFG_INIT | DMA_Q_EMPTY);
            mem.insert(INT_STS, 0);
            Self { mem, ram }
        }
        fn g(&self, a: u32) -> u32 {
            *self.mem.get(&a).unwrap_or(&0)
        }
    }

    impl Io for Fake {
        fn read(&mut self, addr: u32) -> u32 {
            self.g(addr)
        }
        fn write(&mut self, addr: u32, val: u32) {
            if addr == INT_STS {
                let cur = self.g(INT_STS);
                self.mem.insert(INT_STS, cur & !val);
                return;
            }
            if addr == CTRL {
                let old = self.g(CTRL);
                self.mem.insert(CTRL, val);
                let st = self.g(STATUS);
                if (old & PROG_B) != 0 && (val & PROG_B) == 0 {
                    self.mem.insert(STATUS, st & !PCFG_INIT);
                }
                if (val & PROG_B) != 0 {
                    self.mem.insert(STATUS, self.g(STATUS) | PCFG_INIT);
                }
                return;
            }
            if addr == DMA_DST_LEN {
                self.mem.insert(addr, val);
                let dst = self.g(DMA_DST) & !1;
                if dst == P1_DEST {
                    let mut ram = self.ram.borrow_mut();
                    for i in 0..KAT_LEN {
                        ram.insert(P1_DEST + i as u32, i as u8);
                    }
                    let isr = self.g(INT_STS) | DMA_DONE;
                    self.mem.insert(INT_STS, isr);
                } else {
                    let isr = self.g(INT_STS) | DMA_DONE | PCFG_DONE;
                    self.mem.insert(INT_STS, isr);
                }
                return;
            }
            self.mem.insert(addr, val);
        }
        fn poll(&mut self, addr: u32, mask: u32) -> Result<(), u32> {
            if (self.read(addr) & mask) == mask {
                Ok(())
            } else {
                Err(addr)
            }
        }
        fn delay_ms(&mut self, _ms: u32) {}
    }

    struct RamBus {
        ram: Rc<RefCell<BTreeMap<u32, u8>>>,
    }

    impl SdBus for RamBus {
        fn r8(&mut self, _off: u32) -> u8 {
            0
        }
        fn w8(&mut self, _off: u32, _v: u8) {}
        fn r16(&mut self, _off: u32) -> u16 {
            0
        }
        fn w16(&mut self, _off: u32, _v: u16) {}
        fn r32(&mut self, _off: u32) -> u32 {
            0
        }
        fn w32(&mut self, _off: u32, _v: u32) {}
        fn delay(&mut self, _loops: u32) {}
        fn phys_write(&mut self, addr: u32, bytes: &[u8]) {
            let mut ram = self.ram.borrow_mut();
            for (i, b) in bytes.iter().enumerate() {
                ram.insert(addr + i as u32, *b);
            }
        }
        fn phys_read(&mut self, addr: u32, out: &mut [u8]) {
            let ram = self.ram.borrow();
            for (i, b) in out.iter_mut().enumerate() {
                *b = *ram.get(&(addr + i as u32)).unwrap_or(&0);
            }
        }
    }

    fn load_img(img: &[u8]) -> (RamBus, Rc<RefCell<BTreeMap<u32, u8>>>) {
        let ram = Rc::new(RefCell::new(BTreeMap::new()));
        {
            let mut m = ram.borrow_mut();
            for (i, b) in img.iter().enumerate() {
                m.insert(BOOT16_PHYS + i as u32, *b);
            }
        }
        (RamBus { ram: ram.clone() }, ram)
    }

    #[test]
    fn classify_plaintext_encrypted_and_rsa_product() {
        let bit = bit();
        let plain = pack(
            s1(),
            Part {
                name: "system.bit",
                data: &bit,
                load_addr: 0,
                exec_addr: 0,
                attributes: ATTR_PL,
            },
            app(),
        )
        .unwrap();
        let (mut bus, _) = load_img(&plain);
        let d = classify(&mut bus, plain.len() as u32).unwrap();
        assert_eq!(d.kind, Kind::Plaintext);
        assert_eq!(d.partition_count, 3);
        assert_eq!(
            parse_all(&plain[..HEADER_WINDOW_LEN.min(plain.len())], HEADER_WINDOW_LEN.min(plain.len()), plain.len())
                .unwrap()
                .partition_count,
            3
        );

        let k = kat();
        let (n, e) = wp();
        let rsa = pack_rsa_later_product(
            s1(),
            Part {
                name: "ddr_kat.bin",
                data: &k,
                load_addr: P1_DEST,
                exec_addr: 0,
                attributes: ATTR_PS,
            },
            Part {
                name: "system.bit",
                data: &bit,
                load_addr: 0,
                exec_addr: 0,
                attributes: ATTR_PL,
            },
            app(),
            &n,
            &e,
            &n,
            &e,
        )
        .unwrap();
        let win = HEADER_WINDOW_LEN.min(rsa.len());
        assert_eq!(parse_all(&rsa[..win], win, rsa.len()), Err(ParseError::VendorRsa));
        let (mut bus, _) = load_img(&rsa);
        let d = classify(&mut bus, rsa.len() as u32).unwrap();
        assert_eq!(d.kind, Kind::RsaProduct);
        assert_eq!(d.partition_count, 4);

        let kat_only = pack_rsa_later(
            s1(),
            Part {
                name: "ddr_kat.bin",
                data: &k,
                load_addr: P1_DEST,
                exec_addr: 0,
                attributes: ATTR_PS,
            },
            &n,
            &e,
            &n,
            &e,
        )
        .unwrap();
        let (mut bus, _) = load_img(&kat_only);
        let d = classify(&mut bus, kat_only.len() as u32).unwrap();
        assert_eq!(d.kind, Kind::RsaKat);

        let enc = pack_secure_image(
            &[
                s1(),
                Part {
                    name: "ddr_kat.bin",
                    data: &k,
                    load_addr: P1_DEST,
                    exec_addr: 0,
                    attributes: ATTR_PS,
                },
                Part {
                    name: "system.bit",
                    data: &bit,
                    load_addr: 0,
                    exec_addr: 0,
                    attributes: ATTR_PL,
                },
            ],
            app(),
            &stream_keys(),
        )
        .unwrap();
        let (mut bus, _) = load_img(&enc);
        let d = classify(&mut bus, enc.len() as u32).unwrap();
        assert_eq!(d.kind, Kind::Encrypted);
        assert_eq!(d.partition_count, 4);
    }

    #[test]
    fn rsa_and_aes_arms_execute() {
        let bit = bit();
        let k = kat();
        let (n, e) = wp();
        let rsa = pack_rsa_later_product(
            s1(),
            Part {
                name: "ddr_kat.bin",
                data: &k,
                load_addr: P1_DEST,
                exec_addr: 0,
                attributes: ATTR_PS,
            },
            Part {
                name: "system.bit",
                data: &bit,
                load_addr: 0,
                exec_addr: 0,
                attributes: ATTR_PL,
            },
            app(),
            &n,
            &e,
            &n,
            &e,
        )
        .unwrap();
        let (mut bus, ram) = load_img(&rsa);
        assert_eq!(classify(&mut bus, rsa.len() as u32).unwrap().kind, Kind::RsaProduct);
        p1_rsa_kat(&mut bus, rsa.len() as u32).unwrap();
        let mut got = [0u8; KAT_LEN];
        bus.phys_read(P1_DEST, &mut got);
        assert_eq!(got, k);
        let mut io = Fake::new(ram);
        pcap_pl(&mut bus, &mut io, rsa.len() as u32).unwrap();
        assert_eq!(io.g(CTRL) & RATE_EN, 0);
        assert_eq!(io.g(DMA_DST), PL_DEST);

        let enc = pack_secure_image(
            &[
                s1(),
                Part {
                    name: "ddr_kat.bin",
                    data: &k,
                    load_addr: P1_DEST,
                    exec_addr: 0,
                    attributes: ATTR_PS,
                },
                Part {
                    name: "system.bit",
                    data: &bit,
                    load_addr: 0,
                    exec_addr: 0,
                    attributes: ATTR_PL,
                },
            ],
            app(),
            &stream_keys(),
        )
        .unwrap();
        let mut enc_pad = enc;
        while enc_pad.len() < HEADER_WINDOW_LEN {
            enc_pad.push(0);
        }
        let (mut bus, ram) = load_img(&enc_pad);
        assert_eq!(
            classify(&mut bus, enc_pad.len() as u32).unwrap().kind,
            Kind::Encrypted
        );
        let mut io = Fake::new(ram);
        p1_decrypt_kat(&mut bus, &mut io, enc_pad.len() as u32).unwrap();
        assert_eq!(io.g(CTRL) & RATE_EN, RATE_EN);
        let mut got = [0u8; KAT_LEN];
        bus.phys_read(P1_DEST, &mut got);
        assert_eq!(got, k);

        let src = include_str!("sys10a.rs");
        let prod = src.split("#[cfg(test)]").next().unwrap();
        assert!(prod.contains("p1_decrypt_kat"));
        assert!(prod.contains("p1_rsa_kat"));
        assert!(prod.contains("pcap_pl"));
        assert!(prod.contains("handoff::jump"));
        assert!(prod.contains("wait_tx_empty"));
        assert!(prod.contains("jump_app"));
        let ja = prod.split("fn jump_app").nth(1).unwrap();
        let jump_at = ja.find("handoff::jump").unwrap();
        assert!(ja[..jump_at].contains("wait_tx_empty"));
        assert!(prod.contains("working_entry"));
        assert!(prod.contains(DISPATCH_PLAIN.trim_end()));
        assert!(prod.contains(DISPATCH_ENC.trim_end()));
        assert!(prod.contains(DISPATCH_RSA.trim_end()));
        assert!(prod.contains("PARTITION_RSA_PASS"));
        assert!(prod.contains("PCAP_BEGIN_LINE"));
        assert!(prod.contains("PL_PASS_LINE"));
        assert!(prod.contains("Kind::Plaintext"));
        assert!(prod.contains("Kind::Encrypted"));
        assert!(prod.contains("Kind::RsaProduct"));
        let sys07 = include_str!("sys07.rs");
        let vp = sys07.split("fn verify_p1").nth(1).unwrap();
        let vp_body = vp.split("fn verify_kat").next().unwrap();
        assert!(vp_body.contains("from_raw_parts"));
        assert!(vp_body.contains("not(feature = \"std\")"));
    }
}
