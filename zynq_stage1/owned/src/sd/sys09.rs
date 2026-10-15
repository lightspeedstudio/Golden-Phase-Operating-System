// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! System 9: System 8 PCAP then Utility PT_LOAD handoff to `0x00100060`.

#[cfg(any(test, not(feature = "std")))]
use crate::handoff;
#[cfg(any(test, not(feature = "std")))]
use crate::handoff::{HANDOFF_FAIL_LINE, HANDOFF_PASS_LINE};
#[cfg(any(test, not(feature = "std")))]
use crate::sd::bus::SdBus;
#[cfg(any(test, not(feature = "std")))]
use crate::sd::sys06::BOOT_CAP;
#[cfg(not(feature = "std"))]
use crate::sd::sys04::FAT_PASS_PREFIX;
#[cfg(not(feature = "std"))]
use crate::sd::sys06::load_boot_full_on;
#[cfg(not(feature = "std"))]
use crate::sd::sys08::{map_sys06, pcap_pl, write_diag, PCAP_BEGIN_LINE, PL_PASS_LINE};
#[cfg(all(test, feature = "std"))]
use crate::sd::sys08::PL_PASS_LINE;
#[cfg(not(feature = "std"))]
use crate::sd::SdCard;

#[cfg(all(not(feature = "std"), feature = "sys09", not(feature = "sys10a")))]
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    let mut io = crate::platform::mmio::DeviceIo;
    let _ = sys09_device(&mut io);
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}

#[cfg(not(feature = "std"))]
pub fn sys09_device(io: &mut crate::platform::mmio::DeviceIo) -> Result<(), u32> {
    use crate::platform::entry::sys03_platform_ready;
    use crate::platform::uart;
    use crate::sd::bus::DeviceSd;
    use crate::sd::sys04::fmt_dec_u32;
    use crate::sd::sys04::fmt_hex_u32;
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
    let h = handoff::headers(&mut bus, size).map_err(|_| 0xE3u32)?;
    uart::write_str(io, "IMAGE HEADER PASS\n");
    uart::write_str(io, "PARTITIONS count=");
    let c = fmt_dec_u32(h.partition_count as u32, &mut digits);
    let cs = core::str::from_utf8(c).map_err(|_| 0xE3u32)?;
    uart::write_str(io, cs);
    uart::write_str(io, "\n");
    uart::write_str(io, PCAP_BEGIN_LINE);
    match pcap_pl(&mut bus, io, size) {
        Ok(()) => uart::write_str(io, PL_PASS_LINE),
        Err((e, d)) => {
            write_diag(io, &d, e);
            return Err(0xE8);
        }
    }
    match handoff_app(&mut bus, size) {
        Ok(entry) => {
            uart::write_str(io, HANDOFF_PASS_LINE);
            uart::write_str(io, "JUMP ");
            let mut hex = [0u8; 8];
            let h = fmt_hex_u32(entry, &mut hex);
            if let Ok(s) = core::str::from_utf8(h) {
                uart::write_str(io, s);
            }
            uart::write_str(io, "\n");
            unsafe { handoff::jump(entry) }
        }
        Err(_) => {
            uart::write_str(io, HANDOFF_FAIL_LINE);
            Err(0xE9)
        }
    }
}

#[cfg(any(test, not(feature = "std")))]
fn handoff_app<B: SdBus>(bus: &mut B, file_len: u32) -> Result<u32, handoff::Error> {
    if file_len > BOOT_CAP {
        return Err(handoff::Error::Bounds);
    }
    let st = handoff::run(bus, file_len)?;
    if !handoff::matches_working(&st) {
        return Err(handoff::Error::Load);
    }
    match st.ops[6] {
        handoff::Op::Jump(entry) => Ok(entry),
        _ => Err(handoff::Error::Entry),
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::bootimg::pack::{pack, Part};
    use crate::bootimg::parse::{ATTR_PL, ATTR_PS, HEADER_WINDOW_LEN};
    use crate::handoff::{self, UTIL_FILESZ, UTIL_LOAD};
    use crate::sd::bus::HostSd;
    use crate::sd::sys04::BOOT16_PHYS;
    use crate::UTILITY_ENTRY;

    #[test]
    fn uart_lines_are_locked() {
        assert_eq!(HANDOFF_PASS_LINE, "HANDOFF PASS\n");
        assert_eq!(HANDOFF_FAIL_LINE, "HANDOFF FAIL\n");
        assert_eq!(handoff::JUMP_LINE, "JUMP 00100060\n");
        assert_ne!(PL_PASS_LINE, crate::sd::sys08::HOLD_LINE);
        let src = include_str!("sys09.rs");
        assert!(src.contains("handoff::jump"));
    }

    #[test]
    fn run_rejects_wrong_utility_filesz() {
        let mut bit = vec![0u8; 253];
        bit.extend_from_slice(&[0xAA, 0x99, 0x55, 0x66, 0x20, 0x00, 0x00, 0x00]);
        let app = vec![0xEAu8; 16];
        let img = pack(
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
                data: &app,
                load_addr: UTIL_LOAD,
                exec_addr: UTILITY_ENTRY,
                attributes: ATTR_PS,
            },
        )
        .unwrap();
        let mut bus = HostSd::new();
        bus.phys_write(BOOT16_PHYS, &img[..HEADER_WINDOW_LEN.min(img.len())]);
        assert_eq!(handoff_app(&mut bus, img.len() as u32), Err(handoff::Error::Load));
        let _ = UTIL_FILESZ;
    }
}
