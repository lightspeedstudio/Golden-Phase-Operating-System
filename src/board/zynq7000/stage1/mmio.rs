// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! Raw MMIO helpers for Stage 1 (UG585).

use core::ptr::{read_volatile, write_volatile};

pub const SLCR_BASE: u32 = 0xF800_0000;
pub const SLCR_LOCK: u32 = SLCR_BASE + 0x04;
pub const SLCR_UNLOCK: u32 = SLCR_BASE + 0x08;
pub const SLCR_LOCK_KEY: u32 = 0x767B;
pub const SLCR_UNLOCK_KEY: u32 = 0xDF0D;

pub const GTIMER_COUNT_L: u32 = 0xF8F0_0200;
pub const GTIMER_COUNT_U: u32 = 0xF8F0_0204;
pub const GTIMER_CONTROL: u32 = 0xF8F0_0208;

/// APU clock used for SCU global-timer delays (CPU_3x2x = APU/2).
pub const APU_FREQ_HZ: u32 = 766_666_687;

const POLL_SPINS: u32 = 100_000_000;

#[inline]
pub unsafe fn write(addr: u32, val: u32) {
    write_volatile(addr as *mut u32, val);
}

#[inline]
pub unsafe fn read(addr: u32) -> u32 {
    read_volatile(addr as *const u32)
}

#[inline]
pub unsafe fn mask_write(addr: u32, mask: u32, val: u32) {
    let cur = read(addr);
    write(addr, (val & mask) | (cur & !mask));
}

pub unsafe fn slcr_unlock() {
    write(SLCR_UNLOCK, SLCR_UNLOCK_KEY);
}

pub unsafe fn slcr_lock() {
    write(SLCR_LOCK, SLCR_LOCK_KEY);
}

pub fn poll(addr: u32, mask: u32) -> Result<(), u32> {
    let mut spins = 0u32;
    unsafe {
        while (read(addr) & mask) == 0 {
            if spins == POLL_SPINS {
                return Err(addr);
            }
            spins += 1;
        }
    }
    Ok(())
}

unsafe fn timer_start() {
    write(GTIMER_CONTROL, 0);
    write(GTIMER_COUNT_L, 0);
    write(GTIMER_COUNT_U, 0);
    write(GTIMER_CONTROL, (1 << 0) | (1 << 3));
}

/// Delay `ms` using the SCU global timer (runs at APU/2 after PS clocks).
pub unsafe fn delay_ms(ms: u32) {
    let target = ((APU_FREQ_HZ as u64) * (ms as u64) / 2000) as u32;
    timer_start();
    while read(GTIMER_COUNT_L) < target {}
}

pub fn spin(n: u32) {
    for _ in 0..n {
        core::hint::spin_loop();
    }
}
