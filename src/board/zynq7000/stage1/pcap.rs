// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! DevCfg / PCAP fabric configuration (UG585 Device Configuration).

use super::mmio;

const DEVCFG: u32 = 0xF800_7000;
const CTRL: u32 = DEVCFG;
const LOCK: u32 = DEVCFG + 0x04;
const INT_STS: u32 = DEVCFG + 0x0C;
const INT_MASK: u32 = DEVCFG + 0x10;
const STATUS: u32 = DEVCFG + 0x14;
const DMA_SRC: u32 = DEVCFG + 0x18;
const DMA_DST: u32 = DEVCFG + 0x1C;
const DMA_SRC_LEN: u32 = DEVCFG + 0x20;
const DMA_DST_LEN: u32 = DEVCFG + 0x24;
const UNLOCK: u32 = DEVCFG + 0x34;
const MCTRL: u32 = DEVCFG + 0x80;

const UNLOCK_KEY: u32 = 0x757B_DF0D;

const CTRL_PCFG_PROG_B: u32 = 1 << 30;
const CTRL_PCAP_PR: u32 = 1 << 27;
const CTRL_PCAP_MODE: u32 = 1 << 26;
const CTRL_QUARTER_RATE: u32 = 1 << 25;

const STATUS_PCFG_INIT: u32 = 1 << 4;
const IXR_PCFG_DONE: u32 = 1 << 2;
const IXR_DMA_DONE: u32 = 1 << 13;
// Fault bits only (AXI / DMA cmd / HMAC). Not DMA_DONE or D_P_DONE.
const IXR_ERROR: u32 = 0x00F4_C840;

const SLCR_FPGA_RST_CTRL: u32 = 0xF800_0240;
const SLCR_LVL_SHFTR_EN: u32 = 0xF800_0900;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcapError {
    Unlock,
    InitB,
    Dma,
    DmaTimeout,
    Done,
    Empty,
}

#[derive(Clone, Copy)]
pub struct PcapDbg {
    pub err: u8,
    pub int_sts: u32,
    pub status: u32,
    pub ctrl: u32,
    pub mctrl: u32,
    pub words: u32,
    pub src: u32,
    pub lock: u32,
}

static mut LAST: PcapDbg = PcapDbg {
    err: 0,
    int_sts: 0,
    status: 0,
    ctrl: 0,
    mctrl: 0,
    words: 0,
    src: 0,
    lock: 0,
};

pub fn last_dbg() -> PcapDbg {
    unsafe { LAST }
}

pub fn lock_reg() -> u32 {
    unsafe { mmio::read(LOCK) }
}

unsafe fn snap(err: u8, words: u32, src: u32) {
    LAST = PcapDbg {
        err,
        int_sts: mmio::read(INT_STS),
        status: mmio::read(STATUS),
        ctrl: mmio::read(CTRL),
        mctrl: mmio::read(MCTRL),
        words,
        src,
        lock: mmio::read(LOCK),
    };
}

pub fn silicon_version() -> u32 {
    unsafe { mmio::read(MCTRL) >> 28 }
}

unsafe fn unlock_devcfg() -> Result<(), PcapError> {
    // UG585: write 0x757BDF0D to UNLOCK (0x34) to open DevCfg APB writes.
    // LOCK (0x04) is a different register: sticky per-field CTRL locks,
    // cleared only by POR. BootROM often leaves LOCK != 0. That is not
    // unlock failure.
    mmio::write(UNLOCK, UNLOCK_KEY);
    let before = mmio::read(CTRL);
    mmio::write(CTRL, before ^ CTRL_QUARTER_RATE);
    let flipped = mmio::read(CTRL);
    mmio::write(CTRL, before);
    if (flipped ^ before) & CTRL_QUARTER_RATE == 0 {
        // #region agent log
        snap(0, 0, 0);
        // #endregion
        return Err(PcapError::Unlock);
    }
    Ok(())
}

/// Pulse PROG_B and wait for INIT_B (PCFG_INIT) to fall and rise.
unsafe fn fabric_init() -> Result<(), PcapError> {
    unlock_devcfg()?;

    let mut ctrl = mmio::read(CTRL);
    ctrl |= CTRL_PCAP_PR | CTRL_PCAP_MODE;
    ctrl &= !CTRL_QUARTER_RATE;
    mmio::write(CTRL, ctrl);

    mmio::slcr_unlock();
    mmio::write(SLCR_FPGA_RST_CTRL, 0xF);
    mmio::write(SLCR_LVL_SHFTR_EN, 0x0);
    mmio::slcr_lock();

    ctrl = mmio::read(CTRL);
    mmio::write(CTRL, ctrl & !CTRL_PCFG_PROG_B);
    mmio::delay_ms(5);
    if (mmio::read(STATUS) & STATUS_PCFG_INIT) != 0 {
        mmio::delay_ms(5);
        if (mmio::read(STATUS) & STATUS_PCFG_INIT) != 0 {
            // #region agent log
            snap(1, 0, 0);
            // #endregion
            return Err(PcapError::InitB);
        }
    }

    ctrl = mmio::read(CTRL);
    mmio::write(CTRL, ctrl | CTRL_PCFG_PROG_B);
    mmio::delay_ms(5);
    mmio::poll(STATUS, STATUS_PCFG_INIT).map_err(|_| {
        // #region agent log
        snap(1, 0, 0);
        // #endregion
        PcapError::InitB
    })?;
    Ok(())
}

/// Program the PL from a 32-bit-aligned bitstream already in DDR (raw or `.bit` with header stripped).
pub unsafe fn program(bit: &[u8]) -> Result<(), PcapError> {
    if bit.len() < 4 || (bit.as_ptr() as usize) % 4 != 0 {
        // #region agent log
        snap(5, bit.len() as u32, bit.as_ptr() as u32);
        // #endregion
        return Err(PcapError::Empty);
    }
    let words = (bit.len() / 4) as u32;
    if words == 0 {
        snap(5, 0, bit.as_ptr() as u32);
        return Err(PcapError::Empty);
    }

    fabric_init()?;

    mmio::write(INT_STS, 0xFFFF_FFFF);
    mmio::write(INT_MASK, 0);

    let src = (bit.as_ptr() as u32) | 1;
    mmio::write(DMA_SRC, src);
    mmio::write(DMA_DST, 0xFFFF_FFFF);
    mmio::write(DMA_SRC_LEN, words);
    mmio::write(DMA_DST_LEN, words);

    let mut spins = 0u32;
    loop {
        let sts = mmio::read(INT_STS);
        if (sts & IXR_ERROR) != 0 {
            // #region agent log
            snap(2, words, src);
            // #endregion
            return Err(PcapError::Dma);
        }
        if (sts & IXR_DMA_DONE) != 0 {
            break;
        }
        spins += 1;
        if spins == 100_000_000 {
            // #region agent log
            snap(3, words, src);
            // #endregion
            return Err(PcapError::DmaTimeout);
        }
    }

    spins = 0;
    loop {
        let sts = mmio::read(INT_STS);
        if (sts & IXR_PCFG_DONE) != 0 {
            break;
        }
        spins += 1;
        if spins == 100_000_000 {
            // #region agent log
            snap(4, words, src);
            // #endregion
            return Err(PcapError::Done);
        }
    }
    Ok(())
}

/// Level shifters and PL reset release (UG585 post-config).
pub unsafe fn post_config() {
    mmio::slcr_unlock();
    mmio::mask_write(SLCR_LVL_SHFTR_EN, 0xF, 0xF);
    mmio::write(SLCR_FPGA_RST_CTRL, 0);
    mmio::slcr_lock();
}
