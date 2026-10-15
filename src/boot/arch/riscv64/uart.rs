//! NS16550A UART on QEMU RISC-V `virt`.

#[cfg(all(feature = "baremetal", target_arch = "riscv64"))]
use crate::boot::memory_map::UART_BASE;

#[cfg(all(feature = "baremetal", target_arch = "riscv64"))]
use core::ptr::{read_volatile, write_volatile};

#[cfg(all(feature = "baremetal", target_arch = "riscv64"))]
const THR: usize = 0;
#[cfg(all(feature = "baremetal", target_arch = "riscv64"))]
const LSR: usize = 5;
#[cfg(all(feature = "baremetal", target_arch = "riscv64"))]
const LSR_TX_IDLE: u8 = 1 << 5;
#[cfg(all(feature = "baremetal", target_arch = "riscv64"))]
const LSR_RX_READY: u8 = 1;

#[cfg(all(feature = "baremetal", target_arch = "riscv64"))]
#[inline]
fn reg(offset: usize) -> *mut u8 {
    (UART_BASE + offset) as *mut u8
}

pub fn init() {
    #[cfg(all(feature = "baremetal", target_arch = "riscv64"))]
    unsafe {
        write_volatile(reg(1), 0x00);
        write_volatile(reg(3), 0x03);
    }
}

pub fn write_byte(byte: u8) {
    #[cfg(all(feature = "baremetal", target_arch = "riscv64"))]
    unsafe {
        for _ in 0..100_000 {
            if read_volatile(reg(LSR)) & LSR_TX_IDLE != 0 {
                write_volatile(reg(THR), byte);
                return;
            }
        }
        write_volatile(reg(THR), byte);
    }
    #[cfg(not(all(feature = "baremetal", target_arch = "riscv64")))]
    let _ = byte;
}

pub fn write_str(s: &str) {
    for byte in s.as_bytes() {
        write_byte(*byte);
    }
}

pub fn write_hex(mut value: usize) {
    uart_write_hex_impl(&mut value);
}

#[cfg(all(feature = "baremetal", target_arch = "riscv64"))]
fn uart_write_hex_impl(value: &mut usize) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut buf = [0u8; 16];
    for i in (0..16).rev() {
        buf[i] = HEX[*value & 0xf];
        *value >>= 4;
    }
    for byte in buf {
        write_byte(byte);
    }
}

#[cfg(not(all(feature = "baremetal", target_arch = "riscv64")))]
fn uart_write_hex_impl(_value: &mut usize) {}

pub fn try_read_byte() -> Option<u8> {
    #[cfg(all(feature = "baremetal", target_arch = "riscv64"))]
    {
        unsafe {
            if read_volatile(reg(LSR)) & LSR_RX_READY == 0 {
                return None;
            }
            Some(read_volatile(reg(THR)))
        }
    }
    #[cfg(not(all(feature = "baremetal", target_arch = "riscv64")))]
    {
        None
    }
}

pub fn read_byte_blocking() -> u8 {
    #[cfg(all(feature = "baremetal", target_arch = "riscv64"))]
    {
        loop {
            if let Some(byte) = try_read_byte() {
                return byte;
            }
        }
    }
    #[cfg(not(all(feature = "baremetal", target_arch = "riscv64")))]
    {
        b'\n'
    }
}
