//! QEMU boot image (RISC-V `virt`, M-mode, link `0x80000000`).

#![no_std]
#![no_main]

extern crate alloc;

use kairos::boot::kernel_main;

extern "C" {
    fn _start();
}

// Keep the reset vector linked into the final ELF.
#[used]
static _FORCE_START: unsafe extern "C" fn() = _start;

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    kernel_main()
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    kairos::boot::arch::riscv64::uart::write_str("panic\n");
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}
