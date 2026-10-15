// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! System 3 owned entry. Does not replace `zynq_stage1/src/main.rs`.

use super::mmio::Io;
use super::ops_data::{CLOCK_OPS, DDR_OPS, MIO_OPS, PERIPH_REST_OPS, PLL_OPS};
use super::{boot_mode, run_ops, uart, wdt};

pub fn ddr_smoke<I: Io>(io: &mut I, addr: u32) -> bool {
    let p0 = addr;
    let p1 = addr.wrapping_add(4);
    io.write(p0, 0xA5A5_5A5A);
    io.write(p1, !0xA5A5_5A5A);
    io.read(p0) == 0xA5A5_5A5A && io.read(p1) == !0xA5A5_5A5A
}

pub fn sys03_platform_ready<I: Io>(io: &mut I) -> Result<(), u32> {
    wdt::stop(io);
    run_ops(io, MIO_OPS)?;
    run_ops(io, PLL_OPS)?;
    run_ops(io, CLOCK_OPS)?;
    uart::init(io)?;
    uart::write_str(io, "GP-S1 BEGIN\n");
    uart::write_str(io, "UART EARLY PASS\n");
    uart::write_str(io, "PS7 CLOCK PASS\n");
    uart::write_str(io, "DDR BEGIN\n");
    run_ops(io, DDR_OPS)?;
    if !ddr_smoke(io, 0x0010_0000) || !ddr_smoke(io, 0x0050_0000) {
        return Err(0xD0);
    }
    uart::write_str(io, "DDR PASS\n");
    run_ops(io, PERIPH_REST_OPS)?;
    uart::write_str(io, "WDT STOP PASS\n");
    if !boot_mode::is_sd(io) {
        return Err(0xB0);
    }
    uart::write_str(io, "BOOTMODE SD PASS\n");
    Ok(())
}

pub fn sys03_bringup<I: Io>(io: &mut I) -> Result<(), u32> {
    sys03_platform_ready(io)?;
    uart::write_str(io, "SYS03 HOLD\n");
    Ok(())
}

#[cfg(all(
    not(feature = "std"),
    not(feature = "sys04"),
    not(feature = "sys05"),
    not(feature = "sys06"),
    not(feature = "sys07"),
    not(feature = "sys08"),
    not(feature = "sys09"),
    not(feature = "sys10a")
))]
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    let mut io = super::mmio::DeviceIo;
    let _ = sys03_bringup(&mut io);
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}
