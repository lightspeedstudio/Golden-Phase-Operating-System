//! PZ7020-SL-C memory map — seller PS7 / BSP derived.

/// Usable DDR (below 1 MiB reserved / OCM window).
pub const DDR_BASE: u32 = 0x0010_0000;
pub const DDR_HIGH: u32 = 0x1FFF_FFFF;

/// PS UART0 (COM12 evidence port).
pub const UART0_BASE: u32 = 0xE000_0000;

/// G1P AXI-Lite window (V2). Layer-0 registers `0x00`–`0x4C`.
pub const G1P_AXI_BASE: u32 = 0x4000_0000;

/// Path-1 tensor bank offset inside the G1P window (T8). Not `0x40001000`.
/// Occupied offsets: `0x100`–`0x118`. There is no `0x1FC` register in this bit.
pub const G1P_TENSOR_OFFSET: u32 = 0x100;
pub const G1P_TENSOR_LAST: u32 = 0x118;

/// G64 research sidecar (proposed; confirm in Vivado Address Editor).
pub const G64_AXI_BASE: u32 = 0x4000_1000;

/// GEM0 / ENET0 (Cadence MAC).
pub const GEM0_BASE: u32 = 0xE000_B000;
