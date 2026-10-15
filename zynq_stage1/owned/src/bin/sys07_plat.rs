// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial

#![no_std]
#![no_main]

core::arch::global_asm!(include_str!("../start.S"));

#[used]
static KEEP_RUST_MAIN: extern "C" fn() -> ! = zynq_stage1_owned::sd::sys07::rust_main;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}
