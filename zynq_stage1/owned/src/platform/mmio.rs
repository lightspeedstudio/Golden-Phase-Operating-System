// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! MMIO / host recorder. Poll timeouts from working `POLL_SPINS` and `APU_FREQ_HZ`.

pub const APU_FREQ_HZ: u32 = 766_666_687;
pub const POLL_SPINS: u32 = 100_000_000;
/// SCU timer ticks = POLL_SPINS / 2 (timer at APU/2, POLL_SPINS as CPU-cycle budget).
pub const DDR_POLL_TICKS: u32 = POLL_SPINS / 2;
pub const PLL_STATUS: u32 = 0xF800_010C;
pub const GTIMER_COUNT_L: u32 = 0xF8F0_0200;
pub const GTIMER_COUNT_U: u32 = 0xF8F0_0204;
pub const GTIMER_CONTROL: u32 = 0xF8F0_0208;

pub trait Io {
    fn read(&mut self, addr: u32) -> u32;
    fn write(&mut self, addr: u32, val: u32);
    fn poll(&mut self, addr: u32, mask: u32) -> Result<(), u32>;
    fn delay_ms(&mut self, ms: u32);
}

#[cfg(not(feature = "std"))]
pub struct DeviceIo;

#[cfg(not(feature = "std"))]
impl DeviceIo {
    unsafe fn r32(addr: u32) -> u32 {
        core::ptr::read_volatile(addr as *const u32)
    }
    unsafe fn w32(addr: u32, v: u32) {
        core::ptr::write_volatile(addr as *mut u32, v);
    }
    unsafe fn timer_start() {
        Self::w32(GTIMER_CONTROL, 0);
        Self::w32(GTIMER_COUNT_L, 0);
        Self::w32(GTIMER_COUNT_U, 0);
        Self::w32(GTIMER_CONTROL, (1 << 0) | (1 << 3));
    }
}

#[cfg(not(feature = "std"))]
impl Io for DeviceIo {
    fn read(&mut self, addr: u32) -> u32 {
        unsafe { Self::r32(addr) }
    }
    fn write(&mut self, addr: u32, val: u32) {
        unsafe { Self::w32(addr, val) }
    }
    fn poll(&mut self, addr: u32, mask: u32) -> Result<(), u32> {
        if addr == PLL_STATUS {
            let mut spins = 0u32;
            while (self.read(addr) & mask) == 0 {
                if spins == POLL_SPINS {
                    return Err(addr);
                }
                spins += 1;
            }
            return Ok(());
        }
        unsafe {
            Self::timer_start();
            while (self.read(addr) & mask) == 0 {
                if Self::r32(GTIMER_COUNT_L) >= DDR_POLL_TICKS {
                    return Err(addr);
                }
            }
        }
        Ok(())
    }
    fn delay_ms(&mut self, ms: u32) {
        let target = ((APU_FREQ_HZ as u64) * (ms as u64) / 2000) as u32;
        unsafe {
            Self::timer_start();
            while Self::r32(GTIMER_COUNT_L) < target {}
        }
    }
}

#[cfg(feature = "std")]
pub struct HostIo {
    pub mem: std::collections::BTreeMap<u32, u32>,
}

#[cfg(feature = "std")]
impl HostIo {
    pub fn new() -> Self {
        Self {
            mem: std::collections::BTreeMap::new(),
        }
    }
}

#[cfg(feature = "std")]
impl Io for HostIo {
    fn read(&mut self, addr: u32) -> u32 {
        if addr == 0xE000_002C {
            return 1 << 3;
        }
        *self.mem.get(&addr).unwrap_or(&0)
    }
    fn write(&mut self, addr: u32, val: u32) {
        self.mem.insert(addr, val);
    }
    fn poll(&mut self, addr: u32, mask: u32) -> Result<(), u32> {
        let cur = self.read(addr);
        self.mem.insert(addr, cur | mask);
        Ok(())
    }
    fn delay_ms(&mut self, _ms: u32) {}
}
