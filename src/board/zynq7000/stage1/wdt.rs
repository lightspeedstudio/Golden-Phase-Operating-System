// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! System watchdog (UG585 SWDT at 0xF8005000).
//!
//! Stage 1 keeps the watchdog **stopped** for the whole load. We do not copy
//! Xilinx FSBL's 100-second expire policy.

use super::mmio;

const SWDT_BASE: u32 = 0xF800_5000;
const SWDT_MODE: u32 = SWDT_BASE;
const SWDT_CONTROL: u32 = SWDT_BASE + 0x04;
const SWDT_RESTART: u32 = SWDT_BASE + 0x08;
const MODE_ENABLE: u32 = 1;
const RESTART_KEY: u32 = 0x1999;

/// Stop SWDT if BootROM or a previous image left it running.
pub unsafe fn stop() {
    let mode = mmio::read(SWDT_MODE);
    if (mode & MODE_ENABLE) != 0 {
        mmio::write(SWDT_RESTART, RESTART_KEY);
        mmio::write(SWDT_CONTROL, 0);
        mmio::write(SWDT_MODE, mode & !MODE_ENABLE);
    }
}
