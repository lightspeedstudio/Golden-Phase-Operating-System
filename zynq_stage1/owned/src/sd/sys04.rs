// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! System 4: PS7 then ADMA2 CMD18 first 16 KiB of BOOT.BIN.

use crate::sd::bus::SdBus;
use crate::sd::fat::FatVol;
use crate::sd::{read_runs_adma, LbaRun, SdCard, SdError, MAX_ADMA_DESC, READ_16K};

pub const ADMA_TABLE_PHYS: u32 = 0x0000_8000;
pub const FAT_SCRATCH_PHYS: u32 = 0x0000_8100;
pub const BOOT16_PHYS: u32 = 0x0200_0000;

pub const FAT_PASS_PREFIX: &str = "SD FAT BOOT.BIN PASS bytes=";
pub const ADMA_PASS_LINE: &str = "SD ADMA2 CMD18 PASS bytes=16384\n";

pub fn fmt_dec_u32(n: u32, buf: &mut [u8; 10]) -> &[u8] {
    if n == 0 {
        buf[9] = b'0';
        return &buf[9..];
    }
    let mut x = n;
    let mut i = 10;
    while x > 0 {
        i -= 1;
        buf[i] = b'0' + (x % 10) as u8;
        x /= 10;
    }
    &buf[i..]
}

pub fn fmt_hex_u32(n: u32, buf: &mut [u8; 8]) -> &[u8] {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut i = 0usize;
    while i < 8 {
        let sh = 28 - (i * 4);
        buf[i] = H[((n >> sh) & 0xF) as usize];
        i += 1;
    }
    buf
}

/// Width-detect at `dest+0x20` and XLNX ident at `dest+0x24`, as separate reads.
pub fn validate_boot16<B: SdBus>(bus: &mut B, dest: u32) -> Result<(), SdError> {
    let mut sync_b = [0u8; 4];
    let mut ident_b = [0u8; 4];
    bus.phys_read(dest + 0x20, &mut sync_b);
    bus.phys_read(dest + 0x24, &mut ident_b);
    if u32::from_le_bytes(sync_b) != 0xAA99_5566 {
        return Err(SdError::Adma);
    }
    if u32::from_le_bytes(ident_b) != 0x584C_4E58 {
        return Err(SdError::Adma);
    }
    Ok(())
}

pub fn load_boot_first_16k_on<B: SdBus>(
    card: &mut SdCard,
    bus: &mut B,
) -> Result<u32, SdError> {
    let vol = FatVol::mount(card, bus, ADMA_TABLE_PHYS, FAT_SCRATCH_PHYS)
        .map_err(|_| SdError::CmdError)?;
    let (first, size) = vol
        .find_boot_bin(card, bus, ADMA_TABLE_PHYS, FAT_SCRATCH_PHYS)
        .map_err(|_| SdError::CmdError)?;
    if size < READ_16K as u32 {
        return Err(SdError::Adma);
    }
    let mut runs = [LbaRun { lba: 0, blocks: 0 }; MAX_ADMA_DESC];
    let n = vol
        .file_runs(
            card,
            bus,
            first,
            READ_16K as u32,
            ADMA_TABLE_PHYS,
            FAT_SCRATCH_PHYS,
            &mut runs,
        )
        .map_err(|_| SdError::Fragmented)?;
    read_runs_adma(card, bus, &runs[..n], BOOT16_PHYS, ADMA_TABLE_PHYS)?;
    if card.pio_used {
        return Err(SdError::PioForbidden);
    }
    validate_boot16(bus, BOOT16_PHYS)?;
    Ok(size)
}

pub fn load_boot_first_16k<B: SdBus>(bus: &mut B) -> Result<u32, SdError> {
    let mut card = SdCard::init(bus)?;
    load_boot_first_16k_on(&mut card, bus)
}

#[cfg(all(
    not(feature = "std"),
    feature = "sys04",
    not(feature = "sys05"),
    not(feature = "sys06"),
    not(feature = "sys07"),
    not(feature = "sys08"),
    not(feature = "sys09"),
    not(feature = "sys10a")
))]
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    let mut io = crate::platform::mmio::DeviceIo;
    let _ = sys04_device(&mut io);
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}

#[cfg(not(feature = "std"))]
pub fn sys04_device(io: &mut crate::platform::mmio::DeviceIo) -> Result<(), u32> {
    use crate::platform::entry::sys03_platform_ready;
    use crate::platform::uart;
    use crate::sd::bus::DeviceSd;
    sys03_platform_ready(io)?;
    let mut bus = DeviceSd;
    let mut card = crate::sd::SdCard::init(&mut bus).map_err(|_| 0xE2u32)?;
    uart::write_str(io, "SD INIT PASS\n");
    let size = load_boot_first_16k_on(&mut card, &mut bus).map_err(|_| 0xE2u32)?;
    uart::write_str(io, FAT_PASS_PREFIX);
    let mut digits = [0u8; 10];
    let d = fmt_dec_u32(size, &mut digits);
    let s = core::str::from_utf8(d).map_err(|_| 0xE2u32)?;
    uart::write_str(io, s);
    uart::write_str(io, "\n");
    uart::write_str(io, ADMA_PASS_LINE);
    uart::write_str(io, "SYS04 HOLD\n");
    Ok(())
}
