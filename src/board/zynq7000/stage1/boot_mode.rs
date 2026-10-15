// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Boot mode from SLCR `BOOT_MODE_USER` (UG585). PZ7020 ships SD = 0b101.

use super::mmio;

/// SLCR BOOT_MODE_USER.
const BOOT_MODE_REG: u32 = 0xF800_025C;
const BOOT_MODES_MASK: u32 = 0x7;

/// JTAG.
pub const MODE_JTAG: u32 = 0x0;
/// Quad-SPI (not used on this board's Stage 1 path).
pub const MODE_QSPI: u32 = 0x1;
/// NOR.
pub const MODE_NOR: u32 = 0x2;
/// NAND.
pub const MODE_NAND: u32 = 0x4;
/// SD / MMC — the only media this loader implements.
pub const MODE_SD: u32 = 0x5;

pub fn read_mode() -> u32 {
    unsafe { mmio::read(BOOT_MODE_REG) & BOOT_MODES_MASK }
}

pub fn is_sd() -> bool {
    read_mode() == MODE_SD
}
