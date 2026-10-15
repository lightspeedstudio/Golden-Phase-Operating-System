// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! SWDT stop — same registers as working `stage1::wdt::stop`. UG585 SWDT.

use super::mmio::Io;

const SWDT_MODE: u32 = 0xF800_5000;
const SWDT_CONTROL: u32 = 0xF800_5004;
const SWDT_RESTART: u32 = 0xF800_5008;
const MODE_ENABLE: u32 = 1;
const RESTART_KEY: u32 = 0x1999;

pub fn stop<I: Io>(io: &mut I) {
    let mode = io.read(SWDT_MODE);
    if (mode & MODE_ENABLE) != 0 {
        io.write(SWDT_RESTART, RESTART_KEY);
        io.write(SWDT_CONTROL, 0);
        io.write(SWDT_MODE, mode & !MODE_ENABLE);
    }
}

/// Write the restart key if SWDT is enabled. No-op if sys03 already stopped it.
pub fn kick<I: Io>(io: &mut I) {
    if (io.read(SWDT_MODE) & MODE_ENABLE) != 0 {
        io.write(SWDT_RESTART, RESTART_KEY);
    }
}
