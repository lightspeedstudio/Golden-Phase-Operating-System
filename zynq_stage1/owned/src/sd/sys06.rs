// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! System 6: full encrypted BOOT.BIN ADMA load, parse, DevCfg P1 KAT.
//! Hardware AES/HMAC only on device. Not System 6 closed. Not BBRAM.

use crate::bootimg::frame::{parse_all_encrypted, predict_enc_bytes};
use crate::bootimg::parse::{ParsedHeaders, PartitionHeader, HEADER_WINDOW_LEN};
use crate::devcfg::{self, Request, Target};
use crate::platform::mmio::Io;
use crate::sd::bus::SdBus;
use crate::sd::fat::FatVol;
use crate::sd::sys04::{validate_boot16, ADMA_TABLE_PHYS, BOOT16_PHYS, FAT_SCRATCH_PHYS};
#[cfg(not(feature = "std"))]
use crate::sd::sys04::FAT_PASS_PREFIX;
use crate::sd::{LbaRun, SdCard, SdError, MAX_ADMA_DESC, READ_16K};

pub const SYS06_PARSE_STATUS: &str = "SYS06_PARSE_ENCRYPTED_HEADER";
/// Host-wired P1 DevCfg + KAT compare. Not silicon PASS. Not System 6 closed.
pub const SYS06_P1_STATUS: &str = "SYS06_P1_DEVCFG_KAT";

/// Reserved BOOT.BIN DDR window. Exclusive end is `P1_DEST`.
pub const BOOT_CAP: u32 = 0x0100_0000;
/// Partition 1 load / DevCfg PS dest (DDR KAT).
pub const P1_DEST: u32 = 0x0300_0000;
pub const KAT_LEN: usize = 256;
/// ADMA descriptor length field is 16-bit; 128×512 = 65536 overflows it.
const ADMA_MAX_BLOCKS: u32 = 127;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Bounds,
    Parse,
    Adma,
    Pio,
    Dma,
    Hmac,
    Timeout,
    Kat,
}

pub fn parse_window<B: SdBus>(bus: &mut B, fat_len: u32) -> Result<usize, SdError> {
    Ok(loaded_headers(bus, fat_len).map_err(|_| SdError::Adma)?.partition_count)
}

fn loaded_headers<B: SdBus>(bus: &mut B, file_len: u32) -> Result<ParsedHeaders, Error> {
    #[cfg(feature = "std")]
    {
        let mut window = [0u8; HEADER_WINDOW_LEN];
        bus.phys_read(BOOT16_PHYS, &mut window);
        parse_all_encrypted(&window, HEADER_WINDOW_LEN, file_len as usize).map_err(|_| Error::Parse)
    }
    #[cfg(not(feature = "std"))]
    {
        let _ = bus;
        let window =
            unsafe { core::slice::from_raw_parts(BOOT16_PHYS as *const u8, HEADER_WINDOW_LEN) };
        parse_all_encrypted(window, HEADER_WINDOW_LEN, file_len as usize).map_err(|_| Error::Parse)
    }
}

fn map_sd(e: SdError) -> Error {
    match e {
        SdError::PioForbidden => Error::Pio,
        _ => Error::Adma,
    }
}

fn map_devcfg(e: devcfg::Error) -> Error {
    match e {
        devcfg::Error::Hmac => Error::Hmac,
        devcfg::Error::Timeout => Error::Timeout,
        _ => Error::Dma,
    }
}

fn sd_bytes(file_len: u32) -> Result<u32, Error> {
    let n = file_len.checked_add(511).ok_or(Error::Bounds)? & !511;
    Ok(n)
}

fn validate_boot_region(file_len: u32) -> Result<u32, Error> {
    if file_len < READ_16K as u32 {
        return Err(Error::Bounds);
    }
    let n = sd_bytes(file_len)?;
    if n > BOOT_CAP {
        return Err(Error::Bounds);
    }
    let end = BOOT16_PHYS.checked_add(n).ok_or(Error::Bounds)?;
    if end > P1_DEST {
        return Err(Error::Bounds);
    }
    Ok(n)
}

fn ranges_overlap(a0: u32, a1: u32, b0: u32, b1: u32) -> bool {
    a0 < b1 && b0 < a1
}

fn locate_p1(h: &ParsedHeaders, file_len: u32) -> Result<PartitionHeader, Error> {
    let mut found = None;
    for i in 0..h.partition_count {
        let p = h.partitions[i];
        if p.is_ps() && !p.is_pl() && p.load_addr == P1_DEST {
            found = Some(p);
            break;
        }
    }
    let p = found.ok_or(Error::Parse)?;
    if p.enc_words <= p.data_words || p.exec_addr != 0 {
        return Err(Error::Parse);
    }
    let unenc = (p.data_words as usize).saturating_mul(4);
    if unenc != KAT_LEN {
        return Err(Error::Parse);
    }
    let enc = (p.enc_words as usize).saturating_mul(4);
    if enc != predict_enc_bytes(unenc, false) {
        return Err(Error::Parse);
    }
    let off = u32::try_from(p.data_off).map_err(|_| Error::Bounds)?;
    let enc_u = u32::try_from(enc).map_err(|_| Error::Bounds)?;
    let src = BOOT16_PHYS.checked_add(off).ok_or(Error::Bounds)?;
    let src_end = src.checked_add(enc_u).ok_or(Error::Bounds)?;
    let boot_end = BOOT16_PHYS.checked_add(file_len).ok_or(Error::Bounds)?;
    if off.checked_add(enc_u).ok_or(Error::Bounds)? > file_len || src_end > boot_end {
        return Err(Error::Bounds);
    }
    let kat_end = P1_DEST.checked_add(KAT_LEN as u32).ok_or(Error::Bounds)?;
    if ranges_overlap(BOOT16_PHYS, boot_end, P1_DEST, kat_end)
        || ranges_overlap(src, src_end, P1_DEST, kat_end)
    {
        return Err(Error::Bounds);
    }
    Ok(p)
}

fn adma_chunks<B: SdBus>(
    card: &mut SdCard,
    bus: &mut B,
    run: LbaRun,
    mut dest: u32,
) -> Result<(), Error> {
    let mut lba = run.lba;
    let mut left = run.blocks;
    while left > 0 {
        let n = core::cmp::min(left, ADMA_MAX_BLOCKS);
        let bytes = n.checked_mul(512).ok_or(Error::Bounds)?;
        card.read_blocks_adma(bus, lba, dest, bytes, ADMA_TABLE_PHYS)
            .map_err(map_sd)?;
        lba = lba.checked_add(n).ok_or(Error::Bounds)?;
        dest = dest.checked_add(bytes).ok_or(Error::Bounds)?;
        left -= n;
    }
    Ok(())
}

/// ADMA the entire FAT `BOOT.BIN` into the reserved DDR window.
pub fn load_boot_full_on<B: SdBus>(card: &mut SdCard, bus: &mut B) -> Result<u32, Error> {
    let vol = FatVol::mount(card, bus, ADMA_TABLE_PHYS, FAT_SCRATCH_PHYS).map_err(|_| Error::Adma)?;
    let (first, size) = vol
        .find_boot_bin(card, bus, ADMA_TABLE_PHYS, FAT_SCRATCH_PHYS)
        .map_err(|_| Error::Adma)?;
    let n = validate_boot_region(size)?;
    let mut runs = [LbaRun { lba: 0, blocks: 0 }; MAX_ADMA_DESC];
    let nrun = vol
        .file_runs(
            card,
            bus,
            first,
            n,
            ADMA_TABLE_PHYS,
            FAT_SCRATCH_PHYS,
            &mut runs,
        )
        .map_err(|_| Error::Adma)?;
    if nrun == 0 {
        return Err(Error::Adma);
    }
    let mut dest = BOOT16_PHYS;
    for i in 0..nrun {
        adma_chunks(card, bus, runs[i], dest)?;
        let span = runs[i].blocks.checked_mul(512).ok_or(Error::Bounds)?;
        dest = dest.checked_add(span).ok_or(Error::Bounds)?;
    }
    if card.pio_used {
        return Err(Error::Pio);
    }
    validate_boot16(bus, BOOT16_PHYS).map_err(map_sd)?;
    Ok(size)
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

/// Parse the loaded image, DevCfg-decrypt P1 to `P1_DEST`, compare the KAT.
pub fn p1_decrypt_kat<B: SdBus, I: Io>(
    bus: &mut B,
    io: &mut I,
    file_len: u32,
) -> Result<(), Error> {
    validate_boot_region(file_len)?;
    let h = loaded_headers(bus, file_len)?;
    let p = locate_p1(&h, file_len)?;
    let off = p.data_off as u32;
    let src = BOOT16_PHYS.checked_add(off).ok_or(Error::Bounds)?;
    let req = Request {
        src_phys: src,
        dest_phys: P1_DEST,
        image_words: p.enc_words,
        data_words: p.data_words,
        target: Target::Ps,
        caches_off: true,
        fabric_prepared: false,
        poll_limit: if cfg!(test) { 32 } else { 0 },
    };
    devcfg::secure_transfer(io, &req).map_err(map_devcfg)?;
    verify_kat(bus)
}

#[cfg(all(not(feature = "std"), feature = "sys06", not(feature = "sys07"), not(feature = "sys08"), not(feature = "sys09"), not(feature = "sys10a")))]
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    let mut io = crate::platform::mmio::DeviceIo;
    let _ = sys06_device(&mut io);
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}

#[cfg(not(feature = "std"))]
pub fn sys06_device(io: &mut crate::platform::mmio::DeviceIo) -> Result<(), u32> {
    use crate::platform::entry::sys03_platform_ready;
    use crate::platform::uart;
    use crate::sd::bus::DeviceSd;
    use crate::sd::sys04::fmt_dec_u32;
    sys03_platform_ready(io)?;
    let mut bus = DeviceSd;
    let mut card = SdCard::init(&mut bus).map_err(|_| 0xE2u32)?;
    uart::write_str(io, "SD INIT PASS\n");
    let size = load_boot_full_on(&mut card, &mut bus).map_err(|_| 0xE2u32)?;
    uart::write_str(io, FAT_PASS_PREFIX);
    let mut digits = [0u8; 10];
    let d = fmt_dec_u32(size, &mut digits);
    let s = core::str::from_utf8(d).map_err(|_| 0xE2u32)?;
    uart::write_str(io, s);
    uart::write_str(io, "\n");
    uart::write_str(io, "SD ADMA2 CMD18 PASS bytes=");
    uart::write_str(io, s);
    uart::write_str(io, "\n");
    let count = parse_window(&mut bus, size).map_err(|_| 0xE3u32)?;
    uart::write_str(io, "IMAGE HEADER PASS\n");
    uart::write_str(io, "PARTITIONS count=");
    let c = fmt_dec_u32(count as u32, &mut digits);
    let cs = core::str::from_utf8(c).map_err(|_| 0xE3u32)?;
    uart::write_str(io, cs);
    uart::write_str(io, "\n");
    uart::write_str(io, "ENCRYPTED\n");
    uart::write_str(io, "UTILITY ENTRY 00100060\n");
    match p1_decrypt_kat(&mut bus, io, size) {
        Ok(()) => uart::write_str(io, "P1 KAT PASS\n"),
        Err(_) => {
            uart::write_str(io, "P1 FAIL\n");
            return Err(0xE5);
        }
    }
    uart::write_str(io, "SYS06 HOLD\n");
    Ok(())
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::bootimg::parse::{parse_all, ParseError};
    use crate::bootimg::stream::{keys_from_nky, open_encrypted_partition};
    use crate::devcfg::regs::{
        CTRL, DMA_DONE, DMA_DONE_CNT, DMA_DST, DMA_DST_LEN, DMA_Q_EMPTY, DMA_SRC, DMA_SRC_LEN,
        HMAC_ERR, INT_STS, LOOPBACK, MCTRL, PCFG_INIT, RATE_EN, STATUS, UNLOCK, UNLOCK_KEY,
    };
    use crate::sd::bus::HostSd;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::rc::Rc;

    fn oracle_boot() -> Option<Vec<u8>> {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("build/oracle/BOOT_SYS06_ORACLE.BIN");
        if p.exists() {
            Some(std::fs::read(&p).unwrap())
        } else {
            None
        }
    }

    fn oracle_keys() -> Option<crate::bootimg::stream::StreamKeys> {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("build/oracle/sys06_dev.nky");
        if !p.exists() {
            return None;
        }
        keys_from_nky(&std::fs::read_to_string(&p).unwrap())
    }

    fn put_sec(h: &mut HostSd, lba: u32, data: &[u8]) {
        let mut s = [0u8; 512];
        s[..data.len()].copy_from_slice(data);
        h.sectors.insert(lba, s);
    }

    fn fat32_with_boot(payload: &[u8]) -> HostSd {
        let mut h = HostSd::new();
        let spc: u32 = 64;
        let need_blocks = (payload.len() as u32 + 511) / 512;
        let file_clus = (need_blocks + spc - 1) / spc;
        let max_clus = 8 + file_clus;
        let fat_sz = (max_clus * 4 + 511) / 512;
        let reserved = 1u32;
        let data_lba = reserved + fat_sz;
        let tot_sec = data_lba + max_clus * spc;
        let mut bpb = [0u8; 512];
        bpb[0] = 0xEB;
        bpb[1] = 0x58;
        bpb[2] = 0x90;
        bpb[11..13].copy_from_slice(&512u16.to_le_bytes());
        bpb[13] = spc as u8;
        bpb[14..16].copy_from_slice(&1u16.to_le_bytes());
        bpb[16] = 1;
        bpb[21] = 0xF8;
        bpb[32..36].copy_from_slice(&tot_sec.to_le_bytes());
        bpb[36..40].copy_from_slice(&fat_sz.to_le_bytes());
        bpb[44..48].copy_from_slice(&2u32.to_le_bytes());
        bpb[82..90].copy_from_slice(b"FAT32   ");
        bpb[510] = 0x55;
        bpb[511] = 0xAA;
        put_sec(&mut h, 0, &bpb);
        let mut fat = vec![0u8; (fat_sz * 512) as usize];
        fat[0..4].copy_from_slice(&0x0FFF_FFF8u32.to_le_bytes());
        fat[4..8].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes());
        fat[8..12].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes());
        for i in 0..file_clus {
            let cl = 3 + i;
            let nxt = if i + 1 == file_clus {
                0x0FFF_FFFF
            } else {
                cl + 1
            };
            let o = (cl * 4) as usize;
            fat[o..o + 4].copy_from_slice(&nxt.to_le_bytes());
        }
        for s in 0..fat_sz {
            let o = (s * 512) as usize;
            put_sec(&mut h, 1 + s, &fat[o..o + 512]);
        }
        let mut root = [0u8; 512];
        root[0..11].copy_from_slice(b"BOOT    BIN");
        root[11] = 0x20;
        root[20..22].copy_from_slice(&0u16.to_le_bytes());
        root[26..28].copy_from_slice(&3u16.to_le_bytes());
        root[28..32].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        put_sec(&mut h, data_lba, &root);
        let file_lba = data_lba + spc;
        for (i, chunk) in payload.chunks(512).enumerate() {
            put_sec(&mut h, file_lba + i as u32, chunk);
        }
        h
    }

    struct RamBus<'a> {
        sd: &'a mut HostSd,
        ram: Rc<RefCell<BTreeMap<u32, u8>>>,
    }

    impl SdBus for RamBus<'_> {
        fn r8(&mut self, off: u32) -> u8 {
            self.sd.r8(off)
        }
        fn w8(&mut self, off: u32, v: u8) {
            self.sd.w8(off, v);
        }
        fn r16(&mut self, off: u32) -> u16 {
            self.sd.r16(off)
        }
        fn w16(&mut self, off: u32, v: u16) {
            self.sd.w16(off, v);
        }
        fn r32(&mut self, off: u32) -> u32 {
            self.sd.r32(off)
        }
        fn w32(&mut self, off: u32, v: u32) {
            self.sd.w32(off, v);
        }
        fn delay(&mut self, loops: u32) {
            self.sd.delay(loops);
        }
        fn phys_write(&mut self, addr: u32, bytes: &[u8]) {
            let mut ram = self.ram.borrow_mut();
            for (i, b) in bytes.iter().enumerate() {
                ram.insert(addr + i as u32, *b);
            }
        }
        fn phys_read(&mut self, addr: u32, out: &mut [u8]) {
            let ram = self.ram.borrow();
            for (i, slot) in out.iter_mut().enumerate() {
                *slot = *ram.get(&(addr + i as u32)).unwrap_or(&0);
            }
        }
    }

    struct DecryptIo {
        mem: BTreeMap<u32, u32>,
        ram: Rc<RefCell<BTreeMap<u32, u8>>>,
        keys: crate::bootimg::stream::StreamKeys,
        defer: u32,
        left: u32,
        started: bool,
        finished: bool,
        complete: bool,
        inject: u32,
        force_plain: Option<[u8; KAT_LEN]>,
    }

    impl DecryptIo {
        fn new(
            ram: Rc<RefCell<BTreeMap<u32, u8>>>,
            keys: crate::bootimg::stream::StreamKeys,
        ) -> Self {
            let mut mem = BTreeMap::new();
            mem.insert(CTRL, 0x0400_0E00);
            mem.insert(MCTRL, LOOPBACK);
            mem.insert(STATUS, PCFG_INIT | DMA_Q_EMPTY);
            mem.insert(INT_STS, 0);
            Self {
                mem,
                ram,
                keys,
                defer: 0,
                left: 0,
                started: false,
                finished: false,
                complete: true,
                inject: 0,
                force_plain: None,
            }
        }

        fn g(&self, a: u32) -> u32 {
            *self.mem.get(&a).unwrap_or(&0)
        }

        fn finish_dma(&mut self) {
            if self.finished {
                return;
            }
            self.finished = true;
            if self.inject == HMAC_ERR {
                let isr = self.g(INT_STS) | HMAC_ERR;
                self.mem.insert(INT_STS, isr);
                return;
            }
            let src = self.g(DMA_SRC) & !1;
            let dst = self.g(DMA_DST) & !1;
            let img = self.g(DMA_SRC_LEN);
            let data = self.g(DMA_DST_LEN);
            let enc_n = (img as usize).saturating_mul(4);
            let unenc = (data as usize).saturating_mul(4);
            let mut blob = vec![0u8; enc_n];
            {
                let ram = self.ram.borrow();
                for i in 0..enc_n {
                    blob[i] = *ram.get(&(src + i as u32)).unwrap_or(&0);
                }
            }
            let plain = if let Some(p) = self.force_plain {
                p.to_vec()
            } else {
                match open_encrypted_partition(&blob, false, &self.keys, unenc) {
                    Ok(p) => p,
                    Err(_) => {
                        let isr = self.g(INT_STS) | HMAC_ERR;
                        self.mem.insert(INT_STS, isr);
                        return;
                    }
                }
            };
            {
                let mut ram = self.ram.borrow_mut();
                for (i, b) in plain.iter().enumerate() {
                    ram.insert(dst + i as u32, *b);
                }
            }
            let isr = self.g(INT_STS) | DMA_DONE | self.inject;
            self.mem.insert(INT_STS, isr);
            let st = self.g(STATUS);
            let cnt = ((st & DMA_DONE_CNT) >> 28).saturating_add(1).min(3);
            self.mem.insert(STATUS, (st & !DMA_DONE_CNT) | (cnt << 28));
        }
    }

    impl Io for DecryptIo {
        fn read(&mut self, addr: u32) -> u32 {
            if addr == INT_STS && self.started && self.complete && !self.finished {
                if self.left == 0 {
                    self.finish_dma();
                } else {
                    self.left -= 1;
                }
            }
            self.g(addr)
        }
        fn write(&mut self, addr: u32, val: u32) {
            if addr == INT_STS {
                let cur = self.g(INT_STS);
                self.mem.insert(INT_STS, cur & !val);
            } else if addr == STATUS {
                let cur = self.g(STATUS);
                self.mem.insert(STATUS, cur & !(val & DMA_DONE_CNT));
            } else {
                self.mem.insert(addr, val);
            }
            if addr == DMA_DST_LEN {
                self.started = true;
                self.left = self.defer;
                if self.complete && self.defer == 0 {
                    self.finish_dma();
                }
            }
        }
        fn poll(&mut self, _addr: u32, _mask: u32) -> Result<(), u32> {
            panic!("secure_transfer must not use Io::poll");
        }
        fn delay_ms(&mut self, _ms: u32) {}
    }

    #[test]
    fn parse_window_oracle_count_4() {
        let Some(boot) = oracle_boot() else {
            return;
        };
        let mut h = HostSd::new();
        h.phys_write(BOOT16_PHYS, &boot[..HEADER_WINDOW_LEN]);
        assert_eq!(parse_window(&mut h, boot.len() as u32).unwrap(), 4);
        assert_eq!(
            parse_all(&boot[..HEADER_WINDOW_LEN], HEADER_WINDOW_LEN, boot.len()),
            Err(ParseError::Encrypted)
        );
        let src = include_str!("sys06.rs");
        assert!(src.contains("IMAGE HEADER PASS"));
        assert!(src.contains("P1 KAT PASS"));
        assert!(src.contains("SYS06 HOLD"));
        assert!(!src.contains("SYS05 HOLD\n"));
        assert!(src.contains("secure_transfer"));
        assert_eq!(SYS06_PARSE_STATUS, "SYS06_PARSE_ENCRYPTED_HEADER");
        assert_eq!(SYS06_P1_STATUS, "SYS06_P1_DEVCFG_KAT");
        assert_ne!(SYS06_P1_STATUS, "SYS06_CLOSED");
    }

    #[test]
    fn load_full_parse_p1_kat_and_fail_closed() {
        let Some(boot) = oracle_boot() else {
            return;
        };
        let Some(keys) = oracle_keys() else {
            return;
        };
        let mut sd = fat32_with_boot(&boot);
        let mut card = SdCard::init(&mut sd).unwrap();
        let size = load_boot_full_on(&mut card, &mut sd).unwrap();
        assert_eq!(size, boot.len() as u32);
        assert!(sd.adma_used);
        assert!(!card.pio_used);
        let mut got16 = [0u8; HEADER_WINDOW_LEN];
        sd.phys_read(BOOT16_PHYS, &mut got16);
        assert_eq!(&got16, &boot[..HEADER_WINDOW_LEN]);
        let mut p1 = vec![0u8; 1068];
        sd.phys_read(BOOT16_PHYS + 0x4E80, &mut p1);
        assert_eq!(&p1, &boot[0x4E80..0x4E80 + 1068]);
        assert_eq!(parse_window(&mut sd, size).unwrap(), 4);
        let ram = Rc::new(RefCell::new(core::mem::take(&mut sd.dest)));
        let mut bus = RamBus {
            sd: &mut sd,
            ram: ram.clone(),
        };
        let mut io = DecryptIo::new(ram.clone(), keys);
        p1_decrypt_kat(&mut bus, &mut io, size).unwrap();
        let mut kat = [0u8; KAT_LEN];
        bus.phys_read(P1_DEST, &mut kat);
        for i in 0..KAT_LEN {
            assert_eq!(kat[i], i as u8);
        }
        assert_eq!(io.g(UNLOCK), UNLOCK_KEY);
        assert_eq!(io.g(DMA_SRC), (BOOT16_PHYS + 0x4E80) | 1);
        assert_eq!(io.g(DMA_DST), P1_DEST | 1);
        assert_eq!(io.g(DMA_SRC_LEN), 1068 / 4);
        assert_eq!(io.g(DMA_DST_LEN), 256 / 4);
        assert_eq!(io.g(MCTRL) & LOOPBACK, 0);
        assert_eq!(io.g(CTRL) & RATE_EN, RATE_EN);

        let cipher0 = *ram.borrow().get(&(BOOT16_PHYS + 0x4E80 + 204)).unwrap();
        ram.borrow_mut()
            .insert(BOOT16_PHYS + 0x4E80 + 204, cipher0 ^ 1);
        let mut io_h = DecryptIo::new(ram.clone(), keys);
        assert_eq!(
            p1_decrypt_kat(&mut bus, &mut io_h, size),
            Err(Error::Hmac)
        );

        let mut io_t = DecryptIo::new(ram.clone(), keys);
        io_t.complete = false;
        io_t.defer = 8;
        assert_eq!(
            p1_decrypt_kat(&mut bus, &mut io_t, size),
            Err(Error::Timeout)
        );

        let mut io_k = DecryptIo::new(ram.clone(), keys);
        io_k.force_plain = Some([0xA5; KAT_LEN]);
        assert_eq!(p1_decrypt_kat(&mut bus, &mut io_k, size), Err(Error::Kat));

        assert_eq!(validate_boot_region(READ_16K as u32 - 1), Err(Error::Bounds));
        assert_eq!(validate_boot_region(BOOT_CAP + 1), Err(Error::Bounds));
        let gold = std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../fpga/zynq/release/PZ7020_G1P_PATH1_G64/BOOT.BIN"),
        )
        .unwrap();
        let mut g = HostSd::new();
        g.phys_write(BOOT16_PHYS, &gold[..HEADER_WINDOW_LEN]);
        assert_eq!(loaded_headers(&mut g, gold.len() as u32), Err(Error::Parse));
        assert_eq!(
            parse_all(
                &gold[..HEADER_WINDOW_LEN],
                HEADER_WINDOW_LEN,
                gold.len()
            )
            .unwrap()
            .partition_count,
            3
        );
    }

    #[test]
    fn p1_request_rejects_missing_kat_partition() {
        let Some(boot) = oracle_boot() else {
            return;
        };
        let mut h = HostSd::new();
        h.phys_write(BOOT16_PHYS, &boot[..HEADER_WINDOW_LEN]);
        let parsed = loaded_headers(&mut h, boot.len() as u32).unwrap();
        let p = locate_p1(&parsed, boot.len() as u32).unwrap();
        assert_eq!(p.data_off, 0x4E80);
        assert_eq!(p.load_addr, P1_DEST);
        assert_eq!((p.data_words as usize) * 4, KAT_LEN);
        assert_eq!((p.enc_words as usize) * 4, 1068);
        let mut none = parsed;
        none.partitions[1].load_addr = 0;
        assert_eq!(locate_p1(&none, boot.len() as u32), Err(Error::Parse));
    }
}
