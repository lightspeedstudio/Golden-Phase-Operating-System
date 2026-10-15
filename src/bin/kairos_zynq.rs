//! PZ7020 boot image (Zynq-7000 Cortex-A9, link DDR `0x00100000`).
//!
//! Packaged by bootgen as the post-bitstream partition of `BOOT.BIN`;
//! entered by the FSBL after PS7 init and PL configuration.

#![no_std]
#![no_main]

extern crate alloc;

use kairos::board::zynq7000::{kernel_main, uart};

extern "C" {
    fn _start();
}

// Keep the reset vector linked into the final ELF.
#[used]
static _FORCE_START: unsafe extern "C" fn() = _start;

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    kernel_main::kernel_main()
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    uart::write_str("panic\n");
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}
