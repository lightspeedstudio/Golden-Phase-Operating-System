// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! System 5: parse_all on the ADMA 16 KiB window. No payload load.

use crate::bootimg::parse::{parse_all, HEADER_WINDOW_LEN};
use crate::sd::bus::SdBus;
use crate::sd::sys04::BOOT16_PHYS;
use crate::sd::SdError;
#[cfg(not(feature = "std"))]
use crate::sd::sys04::{
    fmt_dec_u32, load_boot_first_16k_on, ADMA_PASS_LINE, FAT_PASS_PREFIX,
};
#[cfg(not(feature = "std"))]
use crate::sd::SdCard;

pub fn parse_window<B: SdBus>(bus: &mut B, fat_len: u32) -> Result<usize, SdError> {
    #[cfg(feature = "std")]
    {
        let mut window = [0u8; HEADER_WINDOW_LEN];
        bus.phys_read(BOOT16_PHYS, &mut window);
        let h = parse_all(&window, HEADER_WINDOW_LEN, fat_len as usize).map_err(|_| SdError::Adma)?;
        Ok(h.partition_count)
    }
    #[cfg(not(feature = "std"))]
    {
        let _ = bus;
        let window =
            unsafe { core::slice::from_raw_parts(BOOT16_PHYS as *const u8, HEADER_WINDOW_LEN) };
        let h = parse_all(window, HEADER_WINDOW_LEN, fat_len as usize).map_err(|_| SdError::Adma)?;
        Ok(h.partition_count)
    }
}

#[cfg(all(not(feature = "std"), feature = "sys05", not(feature = "sys06"), not(feature = "sys07"), not(feature = "sys08"), not(feature = "sys09"), not(feature = "sys10a")))]
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    let mut io = crate::platform::mmio::DeviceIo;
    let _ = sys05_device(&mut io);
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}

#[cfg(not(feature = "std"))]
pub fn sys05_device(io: &mut crate::platform::mmio::DeviceIo) -> Result<(), u32> {
    use crate::platform::entry::sys03_platform_ready;
    use crate::platform::uart;
    use crate::sd::bus::DeviceSd;
    sys03_platform_ready(io)?;
    let mut bus = DeviceSd;
    let mut card = SdCard::init(&mut bus).map_err(|_| 0xE2u32)?;
    uart::write_str(io, "SD INIT PASS\n");
    let size = load_boot_first_16k_on(&mut card, &mut bus).map_err(|_| 0xE2u32)?;
    uart::write_str(io, FAT_PASS_PREFIX);
    let mut digits = [0u8; 10];
    let d = fmt_dec_u32(size, &mut digits);
    let s = core::str::from_utf8(d).map_err(|_| 0xE2u32)?;
    uart::write_str(io, s);
    uart::write_str(io, "\n");
    uart::write_str(io, ADMA_PASS_LINE);
    let count = parse_window(&mut bus, size).map_err(|_| 0xE3u32)?;
    uart::write_str(io, "IMAGE HEADER PASS\n");
    uart::write_str(io, "PARTITIONS count=");
    let c = fmt_dec_u32(count as u32, &mut digits);
    let cs = core::str::from_utf8(c).map_err(|_| 0xE3u32)?;
    uart::write_str(io, cs);
    uart::write_str(io, "\n");
    if count == 0 {
        return Err(0xE3);
    }
    uart::write_str(io, "UTILITY ENTRY 00100060\n");
    uart::write_str(io, "SYS05 HOLD\n");
    Ok(())
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::sd::bus::HostSd;
    use std::path::PathBuf;

    #[test]
    fn parse_window_gold_and_sys04_count_3() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        for rel in [
            "fpga/zynq/release/PZ7020_G1P_PATH1_G64/BOOT.BIN",
            "zynq_stage1/owned/build/BOOT_SYS04_ADMA.BIN",
        ] {
            let boot = std::fs::read(root.join(rel)).unwrap();
            let mut h = HostSd::new();
            h.phys_write(BOOT16_PHYS, &boot[..HEADER_WINDOW_LEN]);
            assert_eq!(parse_window(&mut h, boot.len() as u32).unwrap(), 3);
        }
        let src = include_str!("sys05.rs");
        assert!(src.contains("IMAGE HEADER PASS"));
        assert!(src.contains("PARTITIONS count="));
        assert!(src.contains("UTILITY ENTRY 00100060"));
        assert!(src.contains("SYS05 HOLD"));
        assert!(!src.contains("uart::write_str(io, \"SYS04 HOLD\\n\")"));
    }
}
