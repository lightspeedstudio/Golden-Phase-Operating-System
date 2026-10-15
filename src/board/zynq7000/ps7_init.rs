//! PS7 init bytecode interpreter — Rust mirror of `ps7_config` from the
//! sealed .xsa's `ps7_init.c`. Consumes the checked-in streams in
//! [`ps7_init_data`](super::ps7_init_data).
//!
//! This runs ONLY in the custom Golden Phase stage-0 (OCM, before DDR).
//! When the proven FSBL is the bootloader it already performs PS7 init and
//! this must NOT be executed a second time.

use core::ptr::{read_volatile, write_volatile};

use super::ps7_init_data as data;

const OPCODE_EXIT: u32 = 0;
const OPCODE_CLEAR: u32 = 1;
const OPCODE_WRITE: u32 = 2;
const OPCODE_MASKWRITE: u32 = 3;
const OPCODE_MASKPOLL: u32 = 4;
const OPCODE_MASKDELAY: u32 = 5;

/// Same iteration budget as `PS7_MASK_POLL_TIME` in ps7_init.c.
const MASK_POLL_TIME: u32 = 100_000_000;

/// SCU global timer (used by MASKDELAY, exactly as ps7_init.c does).
const GLOBAL_TIMER_COUNT_L32: u32 = 0xF8F0_0200;
const GLOBAL_TIMER_COUNT_U32: u32 = 0xF8F0_0204;
const GLOBAL_TIMER_CONTROL: u32 = 0xF8F0_0208;

/// devcfg MCTRL — PS_VERSION in bits 31:28.
const DEVCFG_MCTRL: u32 = 0xF800_7080;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ps7Error {
    /// Unknown opcode word encountered (stream corrupt).
    Corrupt(u32),
    /// A MASKPOLL timed out at this address.
    PollTimeout(u32),
    /// Stream ended without EMIT_EXIT.
    Truncated,
}

#[inline]
unsafe fn reg_write(addr: u32, val: u32) {
    write_volatile(addr as *mut u32, val);
}

#[inline]
unsafe fn reg_read(addr: u32) -> u32 {
    read_volatile(addr as *const u32)
}

/// Raw silicon version (devcfg MCTRL PS_VERSION field). 0 = 1.0, 1 = 2.0,
/// 2 = 3.0/3.1. The checked-in streams are the silicon-3.0 arrays.
pub fn silicon_version() -> u32 {
    unsafe { reg_read(DEVCFG_MCTRL) >> 28 }
}

/// Reset and start the SCU global timer (auto-increment, like
/// `perf_reset_and_start_timer` in ps7_init.c).
unsafe fn timer_reset_and_start() {
    reg_write(GLOBAL_TIMER_CONTROL, 0);
    reg_write(GLOBAL_TIMER_COUNT_L32, 0);
    reg_write(GLOBAL_TIMER_COUNT_U32, 0);
    reg_write(GLOBAL_TIMER_CONTROL, (1 << 0) | (1 << 3));
}

/// `get_number_of_cycles_for_delay(ms)` — global timer runs at APU/2.
fn cycles_for_ms(ms: u32) -> u32 {
    ((data::APU_FREQ_HZ as u64) * (ms as u64) / 2000) as u32
}

/// Interpret one operation stream (mirror of `ps7_config`).
///
/// # Safety
/// Performs raw MMIO writes to SLCR/DDRC/MIO registers. Must only run on the
/// PZ7020 in the stage-0 environment described in the module docs.
pub unsafe fn ps7_config(stream: &[u32]) -> Result<(), Ps7Error> {
    let mut i = 0usize;
    while i < stream.len() {
        let word = stream[i];
        let opcode = word >> 4;
        let nargs = (word & 0xF) as usize;
        if i + 1 + nargs > stream.len() {
            return Err(Ps7Error::Truncated);
        }
        let args = &stream[i + 1..i + 1 + nargs];
        i += 1 + nargs;

        match opcode {
            OPCODE_EXIT => return Ok(()),
            OPCODE_CLEAR => reg_write(args[0], 0),
            OPCODE_WRITE => reg_write(args[0], args[1]),
            OPCODE_MASKWRITE => {
                let (addr, mask, val) = (args[0], args[1], args[2]);
                let cur = reg_read(addr);
                reg_write(addr, (val & mask) | (cur & !mask));
            }
            OPCODE_MASKPOLL => {
                let (addr, mask) = (args[0], args[1]);
                let mut spins = 0u32;
                while (reg_read(addr) & mask) == 0 {
                    if spins == MASK_POLL_TIME {
                        return Err(Ps7Error::PollTimeout(addr));
                    }
                    spins += 1;
                }
            }
            OPCODE_MASKDELAY => {
                // args = (global timer count address, delay in ms)
                let (addr, ms) = (args[0], args[1]);
                let target = cycles_for_ms(ms);
                timer_reset_and_start();
                while reg_read(addr) < target {}
            }
            _ => return Err(Ps7Error::Corrupt(word)),
        }
    }
    Err(Ps7Error::Truncated)
}

/// Full PS7 bring-up in the sealed order: MIO → PLL → clocks → DDR →
/// peripherals. Stage-0 only (see module docs).
///
/// # Safety
/// See [`ps7_config`].
pub unsafe fn ps7_init() -> Result<(), Ps7Error> {
    ps7_config(data::PS7_MIO_INIT)?;
    ps7_config(data::PS7_PLL_INIT)?;
    ps7_config(data::PS7_CLOCK_INIT)?;
    ps7_config(data::PS7_DDR_INIT)?;
    ps7_config(data::PS7_PERIPHERALS_INIT)?;
    Ok(())
}

/// Post-PL-configuration settings (level shifters etc.). Run only after the
/// bitstream has been programmed.
///
/// # Safety
/// See [`ps7_config`].
pub unsafe fn ps7_post_config() -> Result<(), Ps7Error> {
    ps7_config(data::PS7_POST_CONFIG)
}
