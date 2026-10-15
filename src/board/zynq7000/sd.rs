//! Zynq-7000 SD0 host (0xE0100000) — polled PIO reads, 512-byte blocks.
//!
//! Read-only port of the board-proven driver from `zynq_a9_os/src/sd.rs`
//! (same register sequence; card state carried in [`SdCard`] instead of
//! statics). Used by the Golden Phase stage-0 to load the kernel file.

use core::ptr::{read_volatile, write_volatile};

const SD0: u32 = 0xE010_0000;

const BLK_SIZE: u32 = 0x04;
const BLK_CNT: u32 = 0x06;
const ARG: u32 = 0x08;
const XFER: u32 = 0x0C;
const RESP0: u32 = 0x10;
const RESP1: u32 = 0x14;
const RESP2: u32 = 0x18;
const RESP3: u32 = 0x1C;
const BUF: u32 = 0x20;
const PRES: u32 = 0x24;
const HOST: u32 = 0x28;
const POWER: u32 = 0x29;
const CLK: u32 = 0x2C;
const TIMEOUT: u32 = 0x2E;
const SWRST: u32 = 0x2F;
const NISTR: u32 = 0x30;
const EISTR: u32 = 0x32;
const NISTER: u32 = 0x34;
const EISTER: u32 = 0x36;

const PRES_CMDINHIBIT: u32 = 1 << 0;
const PRES_DATAINHIBIT: u32 = 1 << 1;
const PRES_BUFWREN: u32 = 1 << 10;
const PRES_BUFRDEN: u32 = 1 << 11;
const PRES_CARDINS: u32 = 1 << 16;

/// Typed media states. `Removed` is only for a wired CD pin (`SD0_HAS_CARD_DETECT`).
/// PZ7020: yank leaves `CARD_INSRT` set; init `CmdError`/`Timeout` → `Unavailable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MediaState {
    Present = 0,
    Initializing = 1,
    Ready = 2,
    Removed = 3,
    Unavailable = 4,
    Faulted = 5,
}

static LAST_MEDIA: core::sync::atomic::AtomicU8 =
    core::sync::atomic::AtomicU8::new(MediaState::Present as u8);

fn set_media(s: MediaState) {
    LAST_MEDIA.store(s as u8, core::sync::atomic::Ordering::Relaxed);
}

pub fn last_media() -> MediaState {
    match LAST_MEDIA.load(core::sync::atomic::Ordering::Relaxed) {
        1 => MediaState::Initializing,
        2 => MediaState::Ready,
        3 => MediaState::Removed,
        4 => MediaState::Unavailable,
        5 => MediaState::Faulted,
        _ => MediaState::Present,
    }
}

/// Seller PS7 / xparameters: `PCW_SD0_GRP_CD_ENABLE=0`, `XPAR_PS7_SD_0_HAS_CD=0`.
pub const SD0_HAS_CARD_DETECT: bool = false;
const NI_CMDCMP: u16 = 1 << 0;
const NI_TRFCMP: u16 = 1 << 1;
const NI_BWRR: u16 = 1 << 4;
const NI_BRRR: u16 = 1 << 5;
const NI_ERR: u16 = 1 << 15;

/// SDHCI command flags (bits in CMD register).
const RESP_NONE: u16 = 0;
const RESP_R48: u16 = 2;
const RESP_R48B: u16 = 3;
const RESP_R136: u16 = 1;
const CMD_CRC: u16 = 1 << 3;
const CMD_IDX: u16 = 1 << 4;
const DATA_PRESENT: u16 = 1 << 5;

/// Transfer mode: block count enable, read.
const TM_BLKCNT_EN: u16 = 1 << 1;
const TM_READ: u16 = 1 << 4;

fn r8(off: u32) -> u8 {
    unsafe { read_volatile((SD0 + off) as *const u8) }
}
fn w8(off: u32, v: u8) {
    unsafe { write_volatile((SD0 + off) as *mut u8, v) }
}
fn r16(off: u32) -> u16 {
    unsafe { read_volatile((SD0 + off) as *const u16) }
}
fn w16(off: u32, v: u16) {
    unsafe { write_volatile((SD0 + off) as *mut u16, v) }
}
fn r32(off: u32) -> u32 {
    unsafe { read_volatile((SD0 + off) as *const u32) }
}
fn w32(off: u32, v: u32) {
    unsafe { write_volatile((SD0 + off) as *mut u32, v) }
}

fn delay(loops: u32) {
    for _ in 0..loops {
        core::hint::spin_loop();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SdFail {
    pub class: u8,
    pub present: u32,
    pub nistr: u16,
    pub eistr: u16,
}

pub const SD_CLASS_TIMEOUT: u8 = 0;
pub const SD_CLASS_CRC: u8 = 1;
pub const SD_CLASS_CONTROLLER: u8 = 2;
pub const SD_CLASS_MEDIA: u8 = 3;
pub const SD_CLASS_INHIBIT: u8 = 4;
pub const SD_CLASS_NOTREADY: u8 = 5;

static LAST_REINIT: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);

pub fn last_reinit() -> u8 {
    LAST_REINIT.load(core::sync::atomic::Ordering::Relaxed)
}

fn snapshot() -> (u32, u16, u16) {
    (r32(PRES), r16(NISTR), r16(EISTR))
}

fn class_from_eistr(eistr: u16) -> u8 {
    if (eistr & ((1 << 0) | (1 << 4))) != 0 {
        SD_CLASS_TIMEOUT
    } else if (eistr & ((1 << 1) | (1 << 5))) != 0 {
        SD_CLASS_CRC
    } else {
        SD_CLASS_CONTROLLER
    }
}

fn wait_clear(mask: u32, spins: u32) -> Result<(), SdError> {
    for _ in 0..spins {
        if (r32(PRES) & mask) == 0 {
            return Ok(());
        }
        core::hint::spin_loop();
    }
    let (present, nistr, eistr) = snapshot();
    let class = if (present & (PRES_CMDINHIBIT | PRES_DATAINHIBIT)) != 0 {
        SD_CLASS_INHIBIT
    } else {
        SD_CLASS_TIMEOUT
    };
    Err(SdError::Detail(SdFail {
        class,
        present,
        nistr,
        eistr,
    }))
}

fn wait_ni(mask: u16, spins: u32) -> Result<(), SdError> {
    for _ in 0..spins {
        let ni = r16(NISTR);
        if (ni & NI_ERR) != 0 {
            let ei = r16(EISTR);
            w16(EISTR, 0xFFFF);
            w16(NISTR, NI_ERR);
            set_media(if SD0_HAS_CARD_DETECT {
                MediaState::Faulted
            } else {
                MediaState::Unavailable
            });
            let present = r32(PRES);
            return Err(SdError::Detail(SdFail {
                class: class_from_eistr(ei),
                present,
                nistr: ni,
                eistr: ei,
            }));
        }
        if (ni & mask) == mask {
            w16(NISTR, mask);
            return Ok(());
        }
        core::hint::spin_loop();
    }
    set_media(if SD0_HAS_CARD_DETECT {
        MediaState::Faulted
    } else {
        MediaState::Unavailable
    });
    let (present, nistr, eistr) = snapshot();
    Err(SdError::Detail(SdFail {
        class: SD_CLASS_TIMEOUT,
        present,
        nistr,
        eistr,
    }))
}

fn cmd_raw(index: u8, arg: u32, flags: u16, xfer: u16) -> Result<u32, SdError> {
    wait_clear(PRES_CMDINHIBIT | PRES_DATAINHIBIT, 2_000_000)?;
    w16(NISTR, 0xFFFF);
    w16(EISTR, 0xFFFF);
    w32(ARG, arg);
    // Atomic 32-bit write of XFER+CMD is preferred on this controller
    let cmd = ((index as u16) << 8) | flags;
    w32(XFER, (cmd as u32) << 16 | (xfer as u32));
    wait_ni(NI_CMDCMP, 2_000_000)?;
    Ok(r32(RESP0))
}

fn cmd(index: u8, arg: u32, flags: u16) -> Result<u32, SdError> {
    cmd_raw(index, arg, flags, 0)
}

fn set_clock_div(div: u16) {
    // Stop SD clock
    let mut c = r16(CLK);
    c &= !0x0004;
    w16(CLK, c);
    delay(1000);
    // Divisor in bits 15:8 (8-bit div) for SDHCI 1.00 / Zynq
    c = (div << 8) | 0x0001; // internal clock enable
    w16(CLK, c);
    // Wait internal clock stable (bit 1)
    for _ in 0..100_000 {
        if (r16(CLK) & 0x0002) != 0 {
            break;
        }
        core::hint::spin_loop();
    }
    w16(CLK, c | 0x0004); // SD clock enable
    delay(10_000);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdError {
    NoCard,
    Timeout,
    CmdError,
    NotSdV2,
    NotReady,
    Detail(SdFail),
}

/// An initialized card (identify + select done, 4-bit bus, data clock).
pub struct SdCard {
    rca: u16,
    /// SDHC/SDXC high capacity — block addressing.
    hcc: bool,
    /// 512-byte sector count from CSD, or 0 if unknown.
    pub device_sectors: u32,
    /// Raw SDHCI RESP0..RESP3 after CMD9.
    pub csd_resp: [u32; 4],
}

impl SdCard {
    fn acmd(&self, index: u8, arg: u32, flags: u16) -> Result<u32, SdError> {
        cmd(55, (self.rca as u32) << 16, RESP_R48 | CMD_CRC | CMD_IDX)?;
        cmd(index, arg, flags)
    }

    pub fn init() -> Result<SdCard, SdError> {
        LAST_REINIT.store(1, core::sync::atomic::Ordering::Relaxed);
        // Software reset all
        w8(SWRST, 0x01);
        for _ in 0..1_000_000 {
            if (r8(SWRST) & 0x01) == 0 {
                break;
            }
            core::hint::spin_loop();
        }

        w8(POWER, 0x0F); // 3.3V + power on
        delay(50_000);

        w8(TIMEOUT, 0x0E);
        w16(NISTER, 0x01FF);
        w16(EISTER, 0xF3FF);
        w8(HOST, 0x00); // 1-bit, LED off

        // Slow clock for identify (~400 kHz).
        set_clock_div(128);

        set_media(MediaState::Initializing);
        if SD0_HAS_CARD_DETECT && (r32(PRES) & PRES_CARDINS) == 0 {
            set_media(MediaState::Removed);
            return Err(SdError::NoCard);
        }

        let mut card = SdCard {
            rca: 0,
            hcc: false,
            device_sectors: 0,
            csd_resp: [0; 4],
        };

        cmd(0, 0, RESP_NONE)?;
        delay(10_000);

        // CMD8 — SD v2
        let r7 = cmd(8, 0x0000_01AA, RESP_R48 | CMD_CRC | CMD_IDX)?;
        if (r7 & 0xFF) != 0xAA {
            set_media(MediaState::Unavailable);
            return Err(SdError::NotSdV2);
        }

        // ACMD41 until ready; HCS bit
        let mut ready = false;
        for _ in 0..1000 {
            let ocr = card.acmd(41, 0x4030_0000, RESP_R48)?;
            if (ocr & (1 << 31)) != 0 {
                card.hcc = (ocr & (1 << 30)) != 0;
                ready = true;
                break;
            }
            delay(50_000);
        }
        if !ready {
            set_media(MediaState::Unavailable);
            return Err(SdError::NotReady);
        }

        cmd(2, 0, RESP_R136 | CMD_CRC)?;
        let rca_resp = cmd(3, 0, RESP_R48 | CMD_CRC | CMD_IDX)?;
        card.rca = (rca_resp >> 16) as u16;

        cmd(9, (card.rca as u32) << 16, RESP_R136 | CMD_CRC)?; // CSD
        let r0 = r32(RESP0);
        let r1 = r32(RESP1);
        let r2 = r32(RESP2);
        let r3 = r32(RESP3);
        card.csd_resp = [r0, r1, r2, r3];
        #[cfg(feature = "phasefs-journal")]
        {
            card.device_sectors = crate::fs::journal::sdhci_csd_sectors(r0, r1, r2, r3);
        }
        cmd(7, (card.rca as u32) << 16, RESP_R48B | CMD_CRC | CMD_IDX)?; // select

        // Faster clock for data
        set_clock_div(4);
        // 4-bit bus
        card.acmd(6, 2, RESP_R48 | CMD_CRC | CMD_IDX)?;
        w8(HOST, 0x02); // DTW 4-bit

        cmd(16, 512, RESP_R48 | CMD_CRC | CMD_IDX)?;
        set_media(MediaState::Ready);
        Ok(card)
    }

    fn block_addr(&self, lba: u32) -> u32 {
        if self.hcc {
            lba
        } else {
            lba.saturating_mul(512)
        }
    }

    pub fn read_block(&mut self, lba: u32, buf: &mut [u8; 512]) -> Result<(), SdError> {
        wait_clear(PRES_CMDINHIBIT | PRES_DATAINHIBIT, 2_000_000)?;
        w16(BLK_SIZE, 512);
        w16(BLK_CNT, 1);
        let xfer = TM_BLKCNT_EN | TM_READ;
        let flags = RESP_R48 | CMD_CRC | CMD_IDX | DATA_PRESENT;
        cmd_raw(17, self.block_addr(lba), flags, xfer)?;

        // PIO read
        let mut off = 0usize;
        while off < 512 {
            wait_ni(NI_BRRR, 2_000_000)?;
            while off < 512 && (r32(PRES) & PRES_BUFRDEN) != 0 {
                let w = r32(BUF);
                buf[off] = (w & 0xFF) as u8;
                buf[off + 1] = ((w >> 8) & 0xFF) as u8;
                buf[off + 2] = ((w >> 16) & 0xFF) as u8;
                buf[off + 3] = ((w >> 24) & 0xFF) as u8;
                off += 4;
            }
        }
        let _ = wait_ni(NI_TRFCMP, 2_000_000);
        Ok(())
    }

    /// Write one 512-byte block (CMD24, polled PIO). Mirror of [`read_block`]
    /// with the write direction (TM_READ clear, buffer-write-ready path).
    pub fn write_block(&mut self, lba: u32, buf: &[u8; 512]) -> Result<(), SdError> {
        wait_clear(PRES_CMDINHIBIT | PRES_DATAINHIBIT, 2_000_000)?;
        w16(BLK_SIZE, 512);
        w16(BLK_CNT, 1);
        let xfer = TM_BLKCNT_EN; // write = TM_READ bit clear
        let flags = RESP_R48 | CMD_CRC | CMD_IDX | DATA_PRESENT;
        cmd_raw(24, self.block_addr(lba), flags, xfer)?;

        // PIO write
        let mut off = 0usize;
        while off < 512 {
            wait_ni(NI_BWRR, 2_000_000)?;
            while off < 512 && (r32(PRES) & PRES_BUFWREN) != 0 {
                let w = (buf[off] as u32)
                    | ((buf[off + 1] as u32) << 8)
                    | ((buf[off + 2] as u32) << 16)
                    | ((buf[off + 3] as u32) << 24);
                w32(BUF, w);
                off += 4;
            }
        }
        wait_ni(NI_TRFCMP, 2_000_000)?;
        Ok(())
    }

    /// CMD13 wait until the card reports TRAN. Best-effort completion wait,
    /// not a cache flush and not a proven power-fail barrier.
    #[cfg(feature = "phasefs-journal")]
    pub fn wait_tran(&self) -> Result<(), SdError> {
        for _ in 0..1_000_000 {
            let r1 = cmd(13, (self.rca as u32) << 16, RESP_R48 | CMD_CRC | CMD_IDX)?;
            let state = (r1 >> 9) & 0xF;
            if state == 4 {
                return Ok(());
            }
            delay(100);
        }
        let (present, nistr, eistr) = snapshot();
        Err(SdError::Detail(SdFail {
            class: SD_CLASS_NOTREADY,
            present,
            nistr,
            eistr,
        }))
    }

    #[cfg(feature = "phasefs-journal")]
    pub fn wait_ready(&self) -> Result<(), SdError> {
        wait_clear(PRES_CMDINHIBIT | PRES_DATAINHIBIT, 2_000_000)?;
        self.wait_tran()
    }
}
