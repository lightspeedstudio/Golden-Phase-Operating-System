// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! SLCR BOOT_MODE_USER. Working firmware: SD = 0b101. UG585.

use super::mmio::Io;

const BOOT_MODE_REG: u32 = 0xF800_025C;
const BOOT_MODES_MASK: u32 = 0x7;
pub const MODE_SD: u32 = 0x5;

pub fn is_sd<I: Io>(io: &mut I) -> bool {
    (io.read(BOOT_MODE_REG) & BOOT_MODES_MASK) == MODE_SD
}
