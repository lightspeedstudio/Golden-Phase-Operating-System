//! PS UART0 MMIO — TX and RX (polled). Baud/MIO come up in `ps7_init`.

use core::ptr::{read_volatile, write_volatile};

use super::memory_map::UART0_BASE;
use crate::platform::String;

const SR: u32 = 0x2C;
const FIFO: u32 = 0x30;
const SR_TXEMPTY: u32 = 1 << 3;
const SR_RXEMPTY: u32 = 1 << 1;

#[inline]
fn reg(offset: u32) -> *mut u32 {
    (UART0_BASE + offset) as *mut u32
}

pub fn putc(c: u8) {
    unsafe {
        while (read_volatile(reg(SR)) & SR_TXEMPTY) == 0 {}
        write_volatile(reg(FIFO), c as u32);
    }
}

/// Raw byte write (no `\n`→`\r\n` translation) — used for echo.
#[inline]
pub fn write_byte(c: u8) {
    putc(c);
}

pub fn write_str(s: &str) {
    for b in s.bytes() {
        if b == b'\n' {
            putc(b'\r');
        }
        putc(b);
    }
}

/// TX that will not spin forever if UART clocks are still down (Stage 1).
pub fn write_str_bounded(s: &str) {
    for b in s.bytes() {
        if b == b'\n' {
            putc_bounded(b'\r');
        }
        putc_bounded(b);
    }
}

fn putc_bounded(c: u8) {
    unsafe {
        for _ in 0..200_000u32 {
            if (read_volatile(reg(SR)) & SR_TXEMPTY) != 0 {
                write_volatile(reg(FIFO), c as u32);
                return;
            }
        }
    }
}

/// Bounded poll until TX FIFO empty (or iteration cap). Used before SLCR.
pub fn drain_tx_bounded() {
    for _ in 0..1_000_000u32 {
        let empty = unsafe { (read_volatile(reg(SR)) & SR_TXEMPTY) != 0 };
        if empty {
            for _ in 0..10_000u32 {
                core::hint::spin_loop();
            }
            return;
        }
        core::hint::spin_loop();
    }
}

/// Block until a byte is available in the RX FIFO, then return it.
pub fn read_byte_blocking() -> u8 {
    unsafe {
        while (read_volatile(reg(SR)) & SR_RXEMPTY) != 0 {}
        (read_volatile(reg(FIFO)) & 0xFF) as u8
    }
}

/// Non-blocking read: `Some(byte)` if the RX FIFO is non-empty, else `None`.
pub fn try_read_byte() -> Option<u8> {
    unsafe {
        if (read_volatile(reg(SR)) & SR_RXEMPTY) != 0 {
            None
        } else {
            Some((read_volatile(reg(FIFO)) & 0xFF) as u8)
        }
    }
}

/// Read one line from the console **without echoing any typed byte** —
/// the real no-echo UART primitive `kernel::identity`'s `passwd` command
/// (Gate 4) needs for secret entry. Backspace/delete still edit the
/// in-progress buffer (so a typo can be corrected), but produce no visual
/// feedback either, since any echo at all (even `*`) would leak the
/// password's length. Only `\r`/`\n` produces output: a real `\r\n` so the
/// next prompt line starts cleanly. This is the same UART0 MMIO FIFO as
/// [`read_line`]/[`read_byte_blocking`] — a real hardware read, not a stub.
pub fn read_line_silent() -> String {
    let mut buf = String::new();
    loop {
        let byte = read_byte_blocking();
        match byte {
            b'\r' | b'\n' => {
                write_byte(b'\r');
                write_byte(b'\n');
                return buf;
            }
            0x08 | 0x7f => {
                buf.pop();
            }
            byte => {
                buf.push(byte as char);
            }
        }
    }
}

/// Read one line from the console, echoing input and handling backspace.
/// Mirrors the RISC-V console (`boot::serial::read_line`).
pub fn read_line() -> String {
    let mut buf = String::new();
    loop {
        let byte = read_byte_blocking();
        match byte {
            b'\r' | b'\n' => {
                write_byte(b'\r');
                write_byte(b'\n');
                return buf;
            }
            0x08 | 0x7f => {
                if buf.pop().is_some() {
                    write_str("\x08 \x08");
                }
            }
            byte => {
                write_byte(byte);
                buf.push(byte as char);
            }
        }
    }
}
