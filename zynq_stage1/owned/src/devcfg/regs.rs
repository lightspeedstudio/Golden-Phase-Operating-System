// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! UG585 DevCfg addresses and masks. Owned names.

pub const BASE: u32 = 0xF800_7000;
pub const CTRL: u32 = BASE;
pub const INT_STS: u32 = BASE + 0x0C;
pub const INT_MASK: u32 = BASE + 0x10;
pub const STATUS: u32 = BASE + 0x14;
pub const DMA_SRC: u32 = BASE + 0x18;
pub const DMA_DST: u32 = BASE + 0x1C;
pub const DMA_SRC_LEN: u32 = BASE + 0x20;
pub const DMA_DST_LEN: u32 = BASE + 0x24;
pub const UNLOCK: u32 = BASE + 0x34;
pub const MCTRL: u32 = BASE + 0x80;

pub const UNLOCK_KEY: u32 = 0x757B_DF0D;
pub const RATE_EN: u32 = 0x0200_0000;
pub const PROG_B: u32 = 0x4000_0000;
pub const PCAP_PR: u32 = 0x0800_0000;
pub const PCAP_MODE: u32 = 0x0400_0000;
pub const POR_B: u32 = 0x0000_0100;
pub const SLCR_LOCK: u32 = 0xF800_0004;
pub const SLCR_UNLOCK: u32 = 0xF800_0008;
pub const SLCR_LOCK_KEY: u32 = 0x767B;
pub const SLCR_UNLOCK_KEY: u32 = 0xDF0D;
pub const FPGA_RST_CTRL: u32 = 0xF800_0240;
pub const LVL_SHFTR_EN: u32 = 0xF800_0900;
pub const LVL_POST: u32 = 0xF;
pub const PL_RESETS: u32 = 0xF;
/// CTRL bits 11:9. Owned code must not set or clear these.
#[allow(dead_code)]
pub const AES_EN: u32 = 0x0000_0E00;
pub const LOOPBACK: u32 = 0x0000_0010;
pub const LAST_CMD: u32 = 1;
pub const LEN_MASK: u32 = 0x07FF_FFFF;
pub const PL_DEST: u32 = 0xFFFF_FFFF;

pub const PCFG_INIT: u32 = 0x0000_0010;
pub const DMA_Q_FULL: u32 = 0x8000_0000;
pub const DMA_Q_EMPTY: u32 = 0x4000_0000;
pub const DMA_DONE_CNT: u32 = 0x3000_0000;

pub const DMA_DONE: u32 = 0x0000_2000;
pub const PCFG_DONE: u32 = 0x0000_0004;
pub const HMAC_ERR: u32 = 0x0000_0040;
pub const AXI_WTO: u32 = 0x0080_0000;
pub const AXI_WERR: u32 = 0x0040_0000;
pub const AXI_RTO: u32 = 0x0020_0000;
pub const AXI_RERR: u32 = 0x0010_0000;
pub const RX_OV: u32 = 0x0004_0000;
pub const DMA_CMD_ERR: u32 = 0x0000_8000;
pub const DMA_Q_OV: u32 = 0x0000_4000;
pub const P2D_LEN: u32 = 0x0000_0800;
pub const SEU_ERR: u32 = 0x0000_0020;

/// FSBL poll/clear bits plus AXI write-timeout from the DevCfg error mask.
/// SEU is classified as CRC (fabric config check).
pub const FATAL_ISR: u32 = AXI_WTO
    | AXI_WERR
    | AXI_RTO
    | AXI_RERR
    | RX_OV
    | DMA_CMD_ERR
    | DMA_Q_OV
    | P2D_LEN
    | HMAC_ERR
    | SEU_ERR;
