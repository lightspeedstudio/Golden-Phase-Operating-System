//! PS UART0 @ 0xE0000000 — seller COM12 path; FSBL already set baud.

use core::ptr::{read_volatile, write_volatile};

const UART0: u32 = 0xE000_0000;
const SR: u32 = 0x2C;
const FIFO: u32 = 0x30;
const SR_TXEMPTY: u32 = 1 << 3;

fn reg(off: u32) -> *mut u32 {
    (UART0 + off) as *mut u32
}

pub fn putc(c: u8) {
    unsafe {
        while (read_volatile(reg(SR)) & SR_TXEMPTY) == 0 {}
        write_volatile(reg(FIFO), c as u32);
    }
}

pub fn write_str(s: &str) {
    for b in s.bytes() {
        if b == b'\n' {
            putc(b'\r');
        }
        putc(b);
    }
}
