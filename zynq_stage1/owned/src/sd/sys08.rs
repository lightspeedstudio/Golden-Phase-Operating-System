// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! System 8: FabricInit + non-secure PCAP of the PL partition. No Utility jump.

use crate::bootimg::parse::{
    loader_bitstream_app, parse_all_product, strip_bit_header, ParsedHeaders, PartitionHeader,
    HEADER_WINDOW_LEN,
};
use crate::devcfg::{self, PlDiag, PlRequest};
use crate::platform::mmio::Io;
use crate::sd::bus::SdBus;
use crate::sd::sys04::BOOT16_PHYS;
use crate::sd::sys06::BOOT_CAP;
#[cfg(not(feature = "std"))]
use crate::sd::sys04::FAT_PASS_PREFIX;
#[cfg(not(feature = "std"))]
use crate::sd::sys06::load_boot_full_on;
#[cfg(not(feature = "std"))]
use crate::sd::SdCard;

pub const BIT_SCRATCH: u32 = 0x0100_0000;
pub const BIT_MAX: u32 = 0x0080_0000;
pub const HOLD_LINE: &str = "SYS08 HOLD\n";
pub const PL_PASS_LINE: &str = "PL PASS\n";
pub const PCAP_BEGIN_LINE: &str = "PCAP BEGIN\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Bounds,
    Parse,
    Adma,
    Pio,
    Bitstream,
    Devcfg(devcfg::Error),
}

fn loaded_headers<B: SdBus>(bus: &mut B, file_len: u32) -> Result<ParsedHeaders, Error> {
    #[cfg(feature = "std")]
    {
        let mut window = [0u8; HEADER_WINDOW_LEN];
        let n = core::cmp::min(HEADER_WINDOW_LEN, file_len as usize);
        bus.phys_read(BOOT16_PHYS, &mut window[..n]);
        parse_all_product(&window[..n], n, file_len as usize).map_err(|_| Error::Parse)
    }
    #[cfg(not(feature = "std"))]
    {
        let _ = bus;
        let n = core::cmp::min(HEADER_WINDOW_LEN, file_len as usize);
        let window = unsafe { core::slice::from_raw_parts(BOOT16_PHYS as *const u8, n) };
        parse_all_product(window, n, file_len as usize).map_err(|_| Error::Parse)
    }
}

#[cfg(not(feature = "std"))]
pub(crate) fn map_sys06(e: crate::sd::sys06::Error) -> Error {
    match e {
        crate::sd::sys06::Error::Bounds => Error::Bounds,
        crate::sd::sys06::Error::Pio => Error::Pio,
        _ => Error::Adma,
    }
}

fn locate_pl(h: &ParsedHeaders, file_len: u32) -> Result<PartitionHeader, Error> {
    let (_, bit, app) = loader_bitstream_app(h).map_err(|_| Error::Parse)?;
    if app.exec_addr != crate::UTILITY_ENTRY {
        return Err(Error::Parse);
    }
    if !bit.is_pl() || bit.enc_words != bit.data_words {
        return Err(Error::Parse);
    }
    if (bit.attributes & crate::bootimg::parse::ATTR_RSA) != 0 {
        return Err(Error::Parse);
    }
    let off = u32::try_from(bit.data_off).map_err(|_| Error::Bounds)?;
    let n = u32::try_from(bit.byte_len()).map_err(|_| Error::Bounds)?;
    if off.checked_add(n).ok_or(Error::Bounds)? > file_len {
        return Err(Error::Bounds);
    }
    Ok(bit)
}

/// Convert file-order configuration words into PCAP DMA byte order.
///
/// UG470 file bytes `AA 99 55 66` are BE word `0xAA995566`. ARM LE DMA of those
/// bytes presents `0x665599AA`. After this conversion DDR bytes are `66 55 99 AA`
/// so the 32-bit word on the PCAP bus is `0xAA995566` (gold Bootgen P1).
pub fn pcap_word_order(buf: &mut [u8]) {
    let mut i = 0usize;
    while i + 3 < buf.len() {
        let a = buf[i];
        let b = buf[i + 1];
        buf[i] = buf[i + 3];
        buf[i + 1] = buf[i + 2];
        buf[i + 2] = b;
        buf[i + 3] = a;
        i += 4;
    }
}

fn copy_stripped<B: SdBus>(bus: &mut B, p: PartitionHeader) -> Result<u32, Error> {
    let src = BOOT16_PHYS + p.data_off as u32;
    let n = p.byte_len() as u32;
    #[cfg(feature = "std")]
    {
        let mut buf = vec![0u8; n as usize];
        bus.phys_read(src, &mut buf);
        let raw = strip_bit_header(&buf).ok_or(Error::Bitstream)?;
        if raw.len() < 4 || raw.len() as u32 > BIT_MAX {
            return Err(Error::Bitstream);
        }
        let words = (raw.len() / 4) as u32;
        if words == 0 {
            return Err(Error::Bitstream);
        }
        let nbytes = (words as usize) * 4;
        let mut aligned = raw[..nbytes].to_vec();
        pcap_word_order(&mut aligned);
        bus.phys_write(BIT_SCRATCH, &aligned);
        Ok(words)
    }
    #[cfg(not(feature = "std"))]
    {
        let _ = bus;
        let bytes = unsafe { core::slice::from_raw_parts(src as *const u8, n as usize) };
        let raw = strip_bit_header(bytes).ok_or(Error::Bitstream)?;
        if raw.len() < 4 || raw.len() as u32 > BIT_MAX {
            return Err(Error::Bitstream);
        }
        let words = (raw.len() / 4) as u32;
        if words == 0 {
            return Err(Error::Bitstream);
        }
        let nbytes = (words as usize) * 4;
        let dst = BIT_SCRATCH as *mut u8;
        let s = raw.as_ptr();
        let mut i = 0usize;
        while i < nbytes {
            unsafe {
                core::ptr::write_volatile(dst.add(i), core::ptr::read_volatile(s.add(i)));
            }
            i += 1;
        }
        let scratch = unsafe { core::slice::from_raw_parts_mut(dst, nbytes) };
        pcap_word_order(scratch);
        Ok(words)
    }
}

#[cfg(any(test, not(feature = "std")))]
fn fail_token(e: Error) -> &'static str {
    match e {
        Error::Bounds => "BOUNDS",
        Error::Parse => "PARSE",
        Error::Adma => "ADMA",
        Error::Pio => "PIO",
        Error::Bitstream => "BITSTREAM",
        Error::Devcfg(d) => devcfg::class_name(d),
    }
}

#[cfg(not(feature = "std"))]
fn write_hex(io: &mut crate::platform::mmio::DeviceIo, n: u32) {
    use crate::platform::uart;
    use crate::sd::sys04::fmt_hex_u32;
    let mut b = [0u8; 8];
    let d = fmt_hex_u32(n, &mut b);
    if let Ok(s) = core::str::from_utf8(d) {
        uart::write_str(io, s);
    }
}

#[cfg(not(feature = "std"))]
fn write_dec(io: &mut crate::platform::mmio::DeviceIo, n: u32) {
    use crate::platform::uart;
    use crate::sd::sys04::fmt_dec_u32;
    let mut b = [0u8; 10];
    let d = fmt_dec_u32(n, &mut b);
    if let Ok(s) = core::str::from_utf8(d) {
        uart::write_str(io, s);
    }
}

#[cfg(not(feature = "std"))]
pub(crate) fn write_diag(io: &mut crate::platform::mmio::DeviceIo, d: &PlDiag, e: Error) {
    use crate::platform::uart;
    uart::write_str(io, "PCAP SRC addr=");
    write_hex(io, d.src_phys);
    uart::write_str(io, " words=");
    write_dec(io, d.words);
    uart::write_str(io, "\nPCAP SRC bytes=");
    write_hex(io, u32::from_be_bytes(d.src_bytes));
    uart::write_str(io, "\nPCAP SRC word=");
    write_hex(io, d.src_word_le);
    uart::write_str(io, "\nPCAP INIT=");
    write_dec(io, d.pre_init);
    uart::write_str(io, "\nPCAP CTRL=");
    write_hex(io, d.pre_ctrl);
    uart::write_str(io, " STATUS=");
    write_hex(io, d.pre_status);
    uart::write_str(io, " MCTRL=");
    write_hex(io, d.pre_mctrl);
    uart::write_str(io, "\nPCAP RST=");
    write_hex(io, d.pre_rst);
    uart::write_str(io, " LVL=");
    write_hex(io, d.pre_lvl);
    uart::write_str(io, "\nPCAP INT=");
    write_hex(io, d.post_int);
    uart::write_str(io, " STATUS=");
    write_hex(io, d.post_status);
    uart::write_str(io, "\nPCAP DMA src=");
    write_hex(io, d.dma_src);
    uart::write_str(io, " dst=");
    write_hex(io, d.dma_dst);
    uart::write_str(io, " slen=");
    write_hex(io, d.dma_slen);
    uart::write_str(io, " dlen=");
    write_hex(io, d.dma_dlen);
    uart::write_str(io, "\nPCAP INIT=");
    write_dec(io, d.post_init);
    uart::write_str(io, " DONE=");
    write_dec(io, d.post_done);
    uart::write_str(io, "\nPL FAIL class=");
    uart::write_str(io, fail_token(e));
    uart::write_str(io, "\n");
}

pub fn pcap_pl<B: SdBus, I: Io>(
    bus: &mut B,
    io: &mut I,
    file_len: u32,
) -> Result<(), (Error, PlDiag)> {
    let empty = PlRequest {
        src_phys: BIT_SCRATCH,
        words: 0,
        caches_off: true,
        poll_limit: if cfg!(test) { 32 } else { 0 },
    };
    if file_len > BOOT_CAP {
        return Err((Error::Bounds, PlDiag::new(&empty, [0; 4])));
    }
    let h = match loaded_headers(bus, file_len) {
        Ok(h) => h,
        Err(e) => return Err((e, PlDiag::new(&empty, [0; 4]))),
    };
    let p = match locate_pl(&h, file_len) {
        Ok(p) => p,
        Err(e) => return Err((e, PlDiag::new(&empty, [0; 4]))),
    };
    let words = match copy_stripped(bus, p) {
        Ok(w) => w,
        Err(e) => return Err((e, PlDiag::new(&empty, [0; 4]))),
    };
    let req = PlRequest {
        src_phys: BIT_SCRATCH,
        words,
        caches_off: true,
        poll_limit: if cfg!(test) { 32 } else { 0 },
    };
    let mut src4 = [0u8; 4];
    bus.phys_read(BIT_SCRATCH, &mut src4);
    let mut diag = PlDiag::new(&req, src4);
    match devcfg::program_pl_traced(io, &req, src4, &mut diag) {
        Ok(()) => Ok(()),
        Err(e) => Err((Error::Devcfg(e), diag)),
    }
}

#[cfg(all(
    not(feature = "std"),
    feature = "sys08",
    not(feature = "sys09"),
    not(feature = "sys10a")
))]
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    let mut io = crate::platform::mmio::DeviceIo;
    let _ = sys08_device(&mut io);
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}

#[cfg(not(feature = "std"))]
pub fn sys08_device(io: &mut crate::platform::mmio::DeviceIo) -> Result<(), u32> {
    use crate::platform::entry::sys03_platform_ready;
    use crate::platform::uart;
    use crate::sd::bus::DeviceSd;
    use crate::sd::sys04::fmt_dec_u32;
    sys03_platform_ready(io)?;
    let mut bus = DeviceSd;
    let mut card = SdCard::init(&mut bus).map_err(|_| 0xE2u32)?;
    uart::write_str(io, "SD INIT PASS\n");
    let size = load_boot_full_on(&mut card, &mut bus)
        .map_err(map_sys06)
        .map_err(|_| 0xE2u32)?;
    uart::write_str(io, FAT_PASS_PREFIX);
    let mut digits = [0u8; 10];
    let d = fmt_dec_u32(size, &mut digits);
    let s = core::str::from_utf8(d).map_err(|_| 0xE2u32)?;
    uart::write_str(io, s);
    uart::write_str(io, "\n");
    uart::write_str(io, "SD ADMA2 CMD18 PASS bytes=");
    uart::write_str(io, s);
    uart::write_str(io, "\n");
    let h = loaded_headers(&mut bus, size).map_err(|_| 0xE3u32)?;
    uart::write_str(io, "IMAGE HEADER PASS\n");
    uart::write_str(io, "PARTITIONS count=");
    let c = fmt_dec_u32(h.partition_count as u32, &mut digits);
    let cs = core::str::from_utf8(c).map_err(|_| 0xE3u32)?;
    uart::write_str(io, cs);
    uart::write_str(io, "\n");
    uart::write_str(io, PCAP_BEGIN_LINE);
    match pcap_pl(&mut bus, io, size) {
        Ok(()) => {
            uart::write_str(io, PL_PASS_LINE);
            uart::write_str(io, HOLD_LINE);
            Ok(())
        }
        Err((e, d)) => {
            write_diag(io, &d, e);
            Err(0xE8)
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::bootimg::pack::{pack, Part};
    use crate::bootimg::parse::{ATTR_PL, ATTR_PS};
    use crate::devcfg::regs::{
        CTRL, DMA_DST, DMA_Q_EMPTY, DMA_SRC, LOOPBACK, MCTRL, PCFG_INIT, PL_DEST, POR_B, RATE_EN,
        STATUS,
    };
    use crate::devcfg::regs::{PCAP_MODE, PROG_B};
    use crate::platform::mmio::Io;
    use crate::sd::bus::HostSd;
    use crate::UTILITY_ENTRY;
    use std::collections::BTreeMap;

    struct Fake {
        mem: BTreeMap<u32, u32>,
        log: Vec<(u32, u32)>,
    }

    impl Fake {
        fn new() -> Self {
            let mut mem = BTreeMap::new();
            mem.insert(CTRL, PCAP_MODE);
            mem.insert(MCTRL, POR_B | LOOPBACK);
            mem.insert(STATUS, PCFG_INIT | DMA_Q_EMPTY);
            mem.insert(crate::devcfg::regs::INT_STS, 0);
            Self {
                mem,
                log: Vec::new(),
            }
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
            self.log.push((addr, val));
            if addr == crate::devcfg::regs::INT_STS {
                let cur = self.g(crate::devcfg::regs::INT_STS);
                self.mem.insert(crate::devcfg::regs::INT_STS, cur & !val);
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
            if addr == crate::devcfg::regs::DMA_DST_LEN {
                self.mem.insert(addr, val);
                let isr = self.g(crate::devcfg::regs::INT_STS)
                    | crate::devcfg::regs::DMA_DONE
                    | crate::devcfg::regs::PCFG_DONE;
                self.mem.insert(crate::devcfg::regs::INT_STS, isr);
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

    fn tiny_boot() -> Vec<u8> {
        let mut bit = vec![0u8; 253];
        bit.extend_from_slice(&[0xAA, 0x99, 0x55, 0x66, 0x20, 0x00, 0x00, 0x00]);
        pack(
            Part {
                name: "s1.bin",
                data: &[0xEAu8, 0xFF, 0xFF, 0xFE],
                load_addr: 0,
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
            Part {
                name: "app.bin",
                data: &[0u8; 16],
                load_addr: 0x0010_0000,
                exec_addr: UTILITY_ENTRY,
                attributes: ATTR_PS,
            },
        )
        .unwrap()
    }

    #[test]
    fn pcap_word_order_makes_le_word_aa995566() {
        let mut b = [0xAAu8, 0x99, 0x55, 0x66, 0x20, 0x00, 0x00, 0x00, 0xDE];
        pcap_word_order(&mut b);
        assert_eq!(&b[..4], &[0x66, 0x55, 0x99, 0xAA]);
        assert_eq!(u32::from_le_bytes(b[..4].try_into().unwrap()), 0xAA99_5566);
        assert_eq!(&b[4..8], &[0x00, 0x00, 0x00, 0x20]);
        assert_eq!(b[8], 0xDE);
        pcap_word_order(&mut b[..8]);
        assert_eq!(&b[..8], &[0xAA, 0x99, 0x55, 0x66, 0x20, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn pcap_pl_host_dma_to_pl_dest() {
        let img = tiny_boot();
        let mut bus = HostSd::new();
        bus.phys_write(BOOT16_PHYS, &img);
        let mut io = Fake::new();
        pcap_pl(&mut bus, &mut io, img.len() as u32).unwrap();
        assert!(io.log.iter().any(|(a, v)| *a == DMA_DST && *v == PL_DEST));
        assert!(io.log.iter().any(|(a, v)| *a == DMA_SRC && *v == (BIT_SCRATCH | 1)));
        assert_eq!(io.g(CTRL) & RATE_EN, 0);
        let mut scratch = [0u8; 8];
        bus.phys_read(BIT_SCRATCH, &mut scratch);
        assert_eq!(&scratch[..4], &[0x66, 0x55, 0x99, 0xAA]);
        assert_eq!(u32::from_le_bytes(scratch[..4].try_into().unwrap()), 0xAA99_5566);
        let src = include_str!("sys08.rs");
        assert!(src.contains("IMAGE HEADER PASS"));
        assert!(src.contains("PCAP BEGIN"));
        assert!(src.contains("PL PASS"));
        assert!(src.contains("SYS08 HOLD"));
        assert!(src.contains("PL FAIL class="));
        assert!(src.contains("PCAP SRC addr="));
        assert!(src.contains("PCAP SRC bytes="));
        assert!(src.contains("PCAP SRC word="));
        let prod = src.split("#[cfg(test)]").next().unwrap();
        assert!(!prod.contains("JUMP"));
        assert!(!prod.contains("SYS07 HOLD"));
        assert!(!prod.contains("secure_transfer"));
    }

    #[test]
    fn por_b_clear_is_fabric_power_with_src_bytes() {
        let img = tiny_boot();
        let mut bus = HostSd::new();
        bus.phys_write(BOOT16_PHYS, &img);
        let mut io = Fake::new();
        io.mem.insert(MCTRL, LOOPBACK);
        let (e, d) = pcap_pl(&mut bus, &mut io, img.len() as u32).unwrap_err();
        assert!(matches!(e, Error::Devcfg(devcfg::Error::FabricPower)));
        assert_eq!(d.class, Some(devcfg::Error::FabricPower));
        assert_eq!(fail_token(e), "FABRIC_POWER");
        assert_eq!(d.src_bytes, [0x66, 0x55, 0x99, 0xAA]);
        assert_eq!(d.src_word_le, 0xAA99_5566);
        assert_eq!(d.src_phys, BIT_SCRATCH);
        assert!(d.words > 0);
    }
}
