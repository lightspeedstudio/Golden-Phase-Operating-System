// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Golden Phase Stage 1 — owned First Stage Boot Loader (OCM).
//!
//! ```text
//! BootROM → this ELF → PS7 → SD BOOT.BIN → PhaseHash + Ed25519
//!         → PCAP bitstream → kernel handoff
//! ```
//!
//! Not Xilinx FSBL. Not a vendor-blob fallback.

#![no_std]
#![no_main]

use kairos::board::zynq7000::fat::{Fat32, FatError};
use kairos::board::zynq7000::sd::{SdCard, SdError};
use kairos::board::zynq7000::stage1::{auth, boot_mode, handoff, measure, pcap, ps7, wdt};
use kairos::board::zynq7000::uart;
use kairos::board::zynq_boot_auth::split_signed;
use kairos::board::zynq_bootimg::{self, BootImgError};

extern "C" {
    fn _start();
}

#[used]
static _FORCE_START: unsafe extern "C" fn() = _start;

const BOOT_SCRATCH: usize = 0x0200_0000;
const BOOT_MAX: usize = 0x0100_0000;
const BIT_SCRATCH: usize = 0x0100_0000;
const BIT_MAX: usize = 0x0080_0000;
const BOOT_NAME83: &[u8; 11] = b"BOOT    BIN";

fn write_hex_u32(v: u32) {
    for i in (0..8).rev() {
        let nib = ((v >> (i * 4)) & 0xF) as u8;
        let c = if nib < 10 { b'0' + nib } else { b'a' + nib - 10 };
        uart::write_byte(c);
    }
}

fn hang() -> ! {
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}

fn fail(msg: &str) -> ! {
    uart::write_str("STAGE1 FAIL: ");
    uart::write_str(msg);
    uart::write_str("\n");
    hang();
}

fn fat_code(e: FatError) -> u32 {
    match e {
        FatError::Io(SdError::NoCard) => 0x10,
        FatError::Io(SdError::Timeout) => 0x11,
        FatError::Io(SdError::CmdError) => 0x12,
        FatError::Io(SdError::NotSdV2) => 0x13,
        FatError::Io(SdError::NotReady) => 0x14,
        FatError::Io(SdError::Detail(d)) => {
            0x1500_0000 | ((d.class as u32) << 16) | (d.eistr as u32)
        }
        FatError::NoFat32 => 0x20,
        FatError::NotFound => 0x21,
        FatError::TooBig => 0x22,
        FatError::BadChain => 0x23,
    }
}

fn fail_img(e: BootImgError) -> ! {
    fail(match e {
        BootImgError::Truncated => "truncated",
        BootImgError::BadIdent => "ident",
        BootImgError::BadIht => "iht",
        BootImgError::BadChecksum => "hdr csum",
        BootImgError::NoBitstream => "no bitstream",
        BootImgError::NoApplication => "no app",
        BootImgError::Encrypted => "encrypted",
        BootImgError::VendorRsa => "vendor rsa",
        BootImgError::BadRange => "range",
        BootImgError::BadSignature => "sig",
    })
}

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    // #region agent log
    uart::write_str_bounded("\nGP STAGE1\n");
    // #endregion
    unsafe {
        wdt::stop();
        if ps7::bringup().is_err() {
            hang();
        }
    }

    uart::write_str("\nGP STAGE1: PS7 INIT PASS\n");
    uart::write_str("STAGE1: silicon version raw ");
    write_hex_u32(pcap::silicon_version());
    uart::write_str("\n");

    if !unsafe { ps7::ddr_smoke(0x0010_0000) && ps7::ddr_smoke(0x0050_0000) } {
        fail("ddr readback");
    }
    uart::write_str("STAGE1: DDR PASS\n");

    if !boot_mode::is_sd() {
        fail("boot mode");
    }

    let mut card = match SdCard::init() {
        Ok(c) => c,
        Err(_) => fail("sd init"),
    };
    uart::write_str("STAGE1: SD INIT PASS\n");

    let vol = match Fat32::mount(&mut card) {
        Ok(v) => v,
        Err(e) => {
            // #region agent log
            uart::write_str("STAGE1 DBG fat e=");
            write_hex_u32(fat_code(e));
            uart::write_str("\n");
            // #endregion
            fail("fat32 mount")
        }
    };
    uart::write_str("STAGE1: FAT PASS\n");
    uart::write_str("STAGE1: LOAD BOOT.BIN\n");

    let n = match unsafe {
        vol.load_root_file(&mut card, BOOT_NAME83, BOOT_SCRATCH as *mut u8, BOOT_MAX)
    } {
        Ok(s) => s,
        Err(e) => {
            // #region agent log
            uart::write_str("STAGE1 DBG load e=");
            write_hex_u32(fat_code(e));
            uart::write_str("\n");
            // #endregion
            fail("load BOOT.BIN")
        }
    };
    // #region agent log
    uart::write_str("STAGE1 DBG load n=");
    write_hex_u32(n as u32);
    uart::write_str("\n");
    // #endregion
    uart::write_str("STAGE1: HASH\n");

    let boot = unsafe { core::slice::from_raw_parts(BOOT_SCRATCH as *const u8, n) };
    let (img, sig) = match split_signed(boot) {
        Some(x) => x,
        None => fail("sig trailer"),
    };

    let (loader, bit_hdr, app_hdr) = match zynq_bootimg::loader_bitstream_app(img) {
        Ok(t) => t,
        Err(e) => fail_img(e),
    };
    if let Err(e) = auth::check_app(&app_hdr) {
        fail_img(e);
    }
    let mut ddr = [app_hdr; zynq_bootimg::MAX_PS_DDR];
    let nd = match zynq_bootimg::ps_ddr_partitions(img, &mut ddr) {
        Ok(n) if n >= 1 => n,
        Ok(_) => fail_img(BootImgError::NoApplication),
        Err(e) => fail_img(e),
    };
    // #region agent log
    uart::write_str("STAGE1 DBG psddr n=");
    write_hex_u32(nd as u32);
    uart::write_str("\n");
    // #endregion

    let loader_bytes = match zynq_bootimg::partition_bytes(img, &loader) {
        Ok(b) => b,
        Err(e) => fail_img(e),
    };
    let bit_bytes = match zynq_bootimg::partition_bytes(img, &bit_hdr) {
        Ok(b) => b,
        Err(e) => fail_img(e),
    };

    let mut klen = 0usize;
    let concat = (BOOT_SCRATCH + n) as *mut u8;
    for p in ddr.iter().take(nd) {
        let b = match zynq_bootimg::partition_bytes(img, p) {
            Ok(x) => x,
            Err(e) => fail_img(e),
        };
        if !auth::ddr_span(p.load_addr, b.len() as u32) {
            fail_img(BootImgError::BadRange);
        }
        if n + klen + b.len() > BOOT_MAX {
            fail("kernel concat");
        }
        unsafe {
            core::ptr::copy_nonoverlapping(b.as_ptr(), concat.add(klen), b.len());
        }
        klen += b.len();
    }
    let app_bytes = unsafe { core::slice::from_raw_parts(concat, klen) };

    let m = measure::measure(loader_bytes, bit_bytes, app_bytes);
    if let Err(e) = auth::verify_measure(&m, sig) {
        fail_img(e);
    }
    unsafe {
        measure::store(&m);
    }
    uart::write_str("STAGE1: AUTH PASS PH-R ");
    let mut hex = [0u8; 64];
    auth::root_hex(&m.image_root, &mut hex);
    for b in hex {
        uart::write_byte(b);
    }
    uart::write_str("\n");

    let raw = zynq_bootimg::strip_bit_header(bit_bytes).unwrap_or(bit_bytes);
    if raw.len() < 4 {
        fail("bit short");
    }
    if raw.len() > BIT_MAX {
        fail("bit too big");
    }
    uart::write_str("STAGE1 DBG bit n=");
    write_hex_u32(raw.len() as u32);
    uart::write_str(" first=");
    let first = u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]);
    write_hex_u32(first);
    uart::write_str("\n");
    // #region agent log
    uart::write_str("STAGE1 DBG lock=");
    write_hex_u32(pcap::lock_reg());
    uart::write_str("\n");
    // #endregion
    unsafe {
        core::ptr::copy_nonoverlapping(raw.as_ptr(), BIT_SCRATCH as *mut u8, raw.len());
    }
    let aligned = unsafe { core::slice::from_raw_parts(BIT_SCRATCH as *const u8, raw.len() & !3) };
    if let Err(e) = auth::check_pcap_src(BIT_SCRATCH as u32, aligned.len() as u32) {
        fail_img(e);
    }
    uart::write_str("STAGE1: PCAP\n");
    if let Err(e) = unsafe { pcap::program(aligned) } {
        let d = pcap::last_dbg();
        // #region agent log
        uart::write_str("STAGE1 DBG pcap e=");
        write_hex_u32(d.err as u32);
        uart::write_str(" int=");
        write_hex_u32(d.int_sts);
        uart::write_str(" sts=");
        write_hex_u32(d.status);
        uart::write_str(" ctrl=");
        write_hex_u32(d.ctrl);
        uart::write_str(" mctrl=");
        write_hex_u32(d.mctrl);
        uart::write_str(" words=");
        write_hex_u32(d.words);
        uart::write_str(" src=");
        write_hex_u32(d.src);
        uart::write_str("\n");
        uart::write_str("STAGE1 DBG lock=");
        write_hex_u32(d.lock);
        uart::write_str("\n");
        // #endregion
        fail(match e {
            pcap::PcapError::Unlock => "pcap unlock",
            pcap::PcapError::InitB => "pcap initb",
            pcap::PcapError::Dma => "pcap dma",
            pcap::PcapError::DmaTimeout => "pcap dma to",
            pcap::PcapError::Done => "pcap done",
            pcap::PcapError::Empty => "pcap empty",
        });
    }
    unsafe {
        pcap::post_config();
    }
    uart::write_str("STAGE1: PL PASS\n");

    unsafe {
        for p in ddr.iter().take(nd) {
            let b = match zynq_bootimg::partition_bytes(img, p) {
                Ok(x) => x,
                Err(e) => fail_img(e),
            };
            core::ptr::copy_nonoverlapping(b.as_ptr(), p.load_addr as *mut u8, b.len());
        }
        uart::write_str("STAGE1: JUMP ");
        write_hex_u32(app_hdr.exec_addr);
        uart::write_str("\n");
        handoff::jump(app_hdr.exec_addr);
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    uart::write_str("STAGE1 panic\n");
    hang();
}
