//! QEMU `virt` memory map for RISC-V.

pub const UART_BASE: usize = 0x1000_0000;
pub const VIRTIO_MMIO_BASE: usize = 0x1000_1000;
pub const VIRTIO_MMIO_STRIDE: usize = 0x1000;
pub const VIRTIO_MMIO_SLOTS: usize = 8;
pub const CLINT_BASE: usize = 0x0200_0000;
pub const MTIME: usize = CLINT_BASE + 0xBFF8;
pub const MTIMECMP: usize = CLINT_BASE + 0x4000;
