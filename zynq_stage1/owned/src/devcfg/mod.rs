// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial
//! DevCfg PCAP: 6B secure transfer and System 8 plaintext PL configure.

pub(crate) mod regs;

use crate::platform::mmio::{Io, POLL_SPINS};
use crate::platform::wdt;
use regs::{
    AXI_RERR, AXI_RTO, AXI_WERR, AXI_WTO, CTRL, DMA_CMD_ERR, DMA_DONE, DMA_DONE_CNT, DMA_DST,
    DMA_DST_LEN, DMA_Q_EMPTY, DMA_Q_FULL, DMA_Q_OV, DMA_SRC, DMA_SRC_LEN, FATAL_ISR, FPGA_RST_CTRL,
    HMAC_ERR, INT_MASK, INT_STS, LAST_CMD, LEN_MASK, LOOPBACK, LVL_POST, LVL_SHFTR_EN, MCTRL,
    P2D_LEN, PCAP_MODE, PCAP_PR, PCFG_DONE, PCFG_INIT, PL_DEST, PL_RESETS, POR_B, PROG_B, RATE_EN,
    RX_OV, SEU_ERR, SLCR_LOCK, SLCR_LOCK_KEY, SLCR_UNLOCK, SLCR_UNLOCK_KEY, STATUS, UNLOCK,
    UNLOCK_KEY,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Ps,
    Pl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Unaligned,
    ZeroLength,
    LengthField,
    NotEncrypted,
    DestNotRam,
    SrcNotRam,
    CachesOn,
    FabricNotPrepared,
    PcfgInit,
    DmaQueueFull,
    DmaStuck,
    Timeout,
    Hmac,
    AxiWto,
    AxiWerr,
    AxiRto,
    AxiRerr,
    RxOverflow,
    DmaCmd,
    DmaQueueOverflow,
    P2dLength,
    Unlock,
    FabricPower,
    InitB,
    Crc,
}

pub fn class_name(e: Error) -> &'static str {
    match e {
        Error::Unaligned => "UNALIGNED",
        Error::ZeroLength => "ZERO_LENGTH",
        Error::LengthField => "LENGTH",
        Error::NotEncrypted => "NOT_ENCRYPTED",
        Error::DestNotRam => "DEST",
        Error::SrcNotRam => "SRC",
        Error::CachesOn => "CACHES",
        Error::FabricNotPrepared => "FABRIC_PREP",
        Error::PcfgInit => "PCFG_INIT",
        Error::DmaQueueFull => "DMA_QUEUE",
        Error::DmaStuck => "DMA_STUCK",
        Error::Timeout => "TIMEOUT",
        Error::Hmac => "HMAC",
        Error::AxiWto => "AXI_WTO",
        Error::AxiWerr => "AXI_WERR",
        Error::AxiRto => "AXI_RTO",
        Error::AxiRerr => "AXI_RERR",
        Error::RxOverflow => "RX_OV",
        Error::DmaCmd => "DMA_CMD",
        Error::DmaQueueOverflow => "DMA_Q_OV",
        Error::P2dLength => "P2D",
        Error::Unlock => "UNLOCK",
        Error::FabricPower => "FABRIC_POWER",
        Error::InitB => "INITB",
        Error::Crc => "CRC",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub src_phys: u32,
    pub dest_phys: u32,
    pub image_words: u32,
    pub data_words: u32,
    pub target: Target,
    pub caches_off: bool,
    pub fabric_prepared: bool,
    pub poll_limit: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlRequest {
    pub src_phys: u32,
    pub words: u32,
    pub caches_off: bool,
    pub poll_limit: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlDiag {
    pub src_phys: u32,
    pub words: u32,
    pub src_bytes: [u8; 4],
    pub src_word_le: u32,
    pub pre_init: u32,
    pub pre_ctrl: u32,
    pub pre_status: u32,
    pub pre_mctrl: u32,
    pub pre_rst: u32,
    pub pre_lvl: u32,
    pub post_int: u32,
    pub post_status: u32,
    pub dma_src: u32,
    pub dma_dst: u32,
    pub dma_slen: u32,
    pub dma_dlen: u32,
    pub post_init: u32,
    pub post_done: u32,
    pub class: Option<Error>,
}

impl PlDiag {
    pub fn new(req: &PlRequest, src4: [u8; 4]) -> Self {
        Self {
            src_phys: req.src_phys,
            words: req.words,
            src_bytes: src4,
            src_word_le: u32::from_le_bytes(src4),
            pre_init: 0,
            pre_ctrl: 0,
            pre_status: 0,
            pre_mctrl: 0,
            pre_rst: 0,
            pre_lvl: 0,
            post_int: 0,
            post_status: 0,
            dma_src: 0,
            dma_dst: 0,
            dma_slen: 0,
            dma_dlen: 0,
            post_init: 0,
            post_done: 0,
            class: None,
        }
    }
}

fn snap_pre<I: Io>(io: &mut I, d: &mut PlDiag) {
    d.pre_ctrl = io.read(CTRL);
    d.pre_status = io.read(STATUS);
    d.pre_mctrl = io.read(MCTRL);
    d.pre_rst = io.read(FPGA_RST_CTRL);
    d.pre_lvl = io.read(LVL_SHFTR_EN);
    d.pre_init = u32::from((d.pre_status & PCFG_INIT) != 0);
}

fn snap_post<I: Io>(io: &mut I, d: &mut PlDiag) {
    d.post_int = io.read(INT_STS);
    d.post_status = io.read(STATUS);
    d.dma_src = io.read(DMA_SRC);
    d.dma_dst = io.read(DMA_DST);
    d.dma_slen = io.read(DMA_SRC_LEN);
    d.dma_dlen = io.read(DMA_DST_LEN);
    d.post_init = u32::from((d.post_status & PCFG_INIT) != 0);
    d.post_done = u32::from((d.post_int & PCFG_DONE) != 0);
}

fn fail<I: Io>(io: &mut I, d: &mut PlDiag, e: Error) -> Error {
    snap_post(io, d);
    d.class = Some(e);
    e
}

pub fn secure_transfer<I: Io>(io: &mut I, req: &Request) -> Result<(), Error> {
    validate(req)?;
    io.write(UNLOCK, UNLOCK_KEY);
    clear_completion(io)?;
    let st = io.read(STATUS);
    if (st & PCFG_INIT) == 0 {
        return Err(Error::PcfgInit);
    }
    if (st & DMA_Q_FULL) != 0 {
        return Err(Error::DmaQueueFull);
    }
    let m = io.read(MCTRL);
    io.write(MCTRL, m & !LOOPBACK);
    let c = io.read(CTRL);
    io.write(CTRL, c | RATE_EN);
    let src = req.src_phys | LAST_CMD;
    let dst = match req.target {
        Target::Ps => req.dest_phys | LAST_CMD,
        Target::Pl => PL_DEST,
    };
    io.write(DMA_SRC, src);
    io.write(DMA_DST, dst);
    io.write(DMA_SRC_LEN, req.image_words);
    io.write(DMA_DST_LEN, req.data_words);
    let spins = if req.poll_limit == 0 {
        POLL_SPINS
    } else {
        req.poll_limit
    };
    poll_mask(io, DMA_DONE, spins)?;
    io.write(INT_STS, DMA_DONE);
    if req.target == Target::Pl {
        poll_mask(io, PCFG_DONE, spins)?;
        io.write(INT_STS, PCFG_DONE);
    }
    clear_done_cnt(io);
    if let Some(e) = classify(io.read(INT_STS)) {
        return Err(e);
    }
    Ok(())
}

pub fn fabric_init<I: Io>(io: &mut I) -> Result<(), Error> {
    io.write(UNLOCK, UNLOCK_KEY);
    let before = io.read(CTRL);
    io.write(CTRL, before ^ RATE_EN);
    let flipped = io.read(CTRL);
    io.write(CTRL, before);
    if (flipped ^ before) & RATE_EN == 0 {
        return Err(Error::Unlock);
    }
    if (io.read(MCTRL) & POR_B) != POR_B {
        return Err(Error::FabricPower);
    }
    let ctrl = (io.read(CTRL) | PCAP_PR | PCAP_MODE) & !RATE_EN;
    io.write(CTRL, ctrl);
    io.write(SLCR_UNLOCK, SLCR_UNLOCK_KEY);
    io.write(FPGA_RST_CTRL, PL_RESETS);
    io.write(LVL_SHFTR_EN, 0);
    io.write(SLCR_LOCK, SLCR_LOCK_KEY);
    let ctrl = io.read(CTRL);
    io.write(CTRL, ctrl | PROG_B);
    let ctrl = io.read(CTRL);
    io.write(CTRL, ctrl & !PROG_B);
    io.delay_ms(5);
    if (io.read(STATUS) & PCFG_INIT) != 0 {
        io.delay_ms(5);
        if (io.read(STATUS) & PCFG_INIT) != 0 {
            return Err(Error::InitB);
        }
    }
    let ctrl = io.read(CTRL);
    io.write(CTRL, ctrl | PROG_B);
    io.delay_ms(5);
    let spins = POLL_SPINS;
    let mut n = 0u32;
    loop {
        if (io.read(STATUS) & PCFG_INIT) == PCFG_INIT {
            break;
        }
        if n == spins {
            return Err(Error::InitB);
        }
        n = n.saturating_add(1);
    }
    Ok(())
}

pub fn post_config<I: Io>(io: &mut I) {
    io.write(SLCR_UNLOCK, SLCR_UNLOCK_KEY);
    let lvl = io.read(LVL_SHFTR_EN);
    io.write(LVL_SHFTR_EN, (lvl & !LVL_POST) | LVL_POST);
    io.write(FPGA_RST_CTRL, 0);
    io.write(SLCR_LOCK, SLCR_LOCK_KEY);
}

pub fn program_pl<I: Io>(io: &mut I, req: &PlRequest) -> Result<(), Error> {
    let mut d = PlDiag::new(req, [0; 4]);
    program_pl_traced(io, req, [0; 4], &mut d)
}

pub fn program_pl_traced<I: Io>(
    io: &mut I,
    req: &PlRequest,
    src4: [u8; 4],
    diag: &mut PlDiag,
) -> Result<(), Error> {
    *diag = PlDiag::new(req, src4);
    if let Err(e) = validate_pl(req) {
        diag.class = Some(e);
        return Err(e);
    }
    if let Err(e) = fabric_init(io) {
        snap_pre(io, diag);
        return Err(fail(io, diag, e));
    }
    if let Err(e) = clear_completion(io) {
        snap_pre(io, diag);
        return Err(fail(io, diag, e));
    }
    snap_pre(io, diag);
    let st = io.read(STATUS);
    if (st & PCFG_INIT) == 0 {
        return Err(fail(io, diag, Error::PcfgInit));
    }
    if (st & DMA_Q_FULL) != 0 {
        return Err(fail(io, diag, Error::DmaQueueFull));
    }
    io.write(INT_MASK, 0);
    let m = io.read(MCTRL);
    io.write(MCTRL, m & !LOOPBACK);
    let c = io.read(CTRL);
    io.write(CTRL, c & !RATE_EN);
    io.write(DMA_SRC, req.src_phys | LAST_CMD);
    io.write(DMA_DST, PL_DEST);
    io.write(DMA_SRC_LEN, req.words);
    io.write(DMA_DST_LEN, req.words);
    let spins = if req.poll_limit == 0 {
        POLL_SPINS
    } else {
        req.poll_limit
    };
    if let Err(e) = poll_mask(io, DMA_DONE, spins) {
        return Err(fail(io, diag, e));
    }
    io.write(INT_STS, DMA_DONE);
    match poll_mask(io, PCFG_DONE, spins) {
        Ok(()) => {}
        Err(Error::Timeout) => return Err(fail(io, diag, Error::Crc)),
        Err(e) => return Err(fail(io, diag, e)),
    }
    snap_post(io, diag);
    io.write(INT_STS, PCFG_DONE);
    clear_done_cnt(io);
    if let Some(e) = classify(io.read(INT_STS)) {
        return Err(fail(io, diag, e));
    }
    post_config(io);
    Ok(())
}

fn validate_pl(req: &PlRequest) -> Result<(), Error> {
    if !req.caches_off {
        return Err(Error::CachesOn);
    }
    if req.words == 0 {
        return Err(Error::ZeroLength);
    }
    if (req.words & !LEN_MASK) != 0 {
        return Err(Error::LengthField);
    }
    if (req.src_phys & 3) != 0 {
        return Err(Error::Unaligned);
    }
    if !ram_phys(req.src_phys) {
        return Err(Error::SrcNotRam);
    }
    Ok(())
}

fn validate(req: &Request) -> Result<(), Error> {
    if !req.caches_off {
        return Err(Error::CachesOn);
    }
    if req.target == Target::Pl && !req.fabric_prepared {
        return Err(Error::FabricNotPrepared);
    }
    if req.image_words == 0 || req.data_words == 0 {
        return Err(Error::ZeroLength);
    }
    if (req.image_words & !LEN_MASK) != 0 || (req.data_words & !LEN_MASK) != 0 {
        return Err(Error::LengthField);
    }
    if req.image_words <= req.data_words {
        return Err(Error::NotEncrypted);
    }
    if (req.src_phys & 3) != 0 {
        return Err(Error::Unaligned);
    }
    if !ram_phys(req.src_phys) {
        return Err(Error::SrcNotRam);
    }
    if req.target == Target::Ps {
        if req.dest_phys == PL_DEST || !ram_phys(req.dest_phys) {
            return Err(Error::DestNotRam);
        }
        if (req.dest_phys & 3) != 0 {
            return Err(Error::Unaligned);
        }
    }
    Ok(())
}

fn ram_phys(addr: u32) -> bool {
    addr <= 0x3FFF_FFFC || (0xFFFC_0000..=0xFFFF_FFFC).contains(&addr)
}

fn classify(isr: u32) -> Option<Error> {
    if (isr & FATAL_ISR) == 0 {
        return None;
    }
    if (isr & HMAC_ERR) != 0 {
        return Some(Error::Hmac);
    }
    if (isr & SEU_ERR) != 0 {
        return Some(Error::Crc);
    }
    if (isr & AXI_WERR) != 0 {
        return Some(Error::AxiWerr);
    }
    if (isr & AXI_RTO) != 0 {
        return Some(Error::AxiRto);
    }
    if (isr & AXI_RERR) != 0 {
        return Some(Error::AxiRerr);
    }
    if (isr & AXI_WTO) != 0 {
        return Some(Error::AxiWto);
    }
    if (isr & RX_OV) != 0 {
        return Some(Error::RxOverflow);
    }
    if (isr & DMA_CMD_ERR) != 0 {
        return Some(Error::DmaCmd);
    }
    if (isr & DMA_Q_OV) != 0 {
        return Some(Error::DmaQueueOverflow);
    }
    if (isr & P2D_LEN) != 0 {
        return Some(Error::P2dLength);
    }
    None
}

fn clear_completion<I: Io>(io: &mut I) -> Result<(), Error> {
    io.write(INT_STS, 0xFFFF_FFFF);
    if let Some(e) = classify(io.read(INT_STS)) {
        return Err(e);
    }
    let st = io.read(STATUS);
    if (st & DMA_Q_FULL) != 0 {
        return Err(Error::DmaQueueFull);
    }
    if (st & DMA_Q_EMPTY) == 0 {
        let isr = io.read(INT_STS);
        if (isr & DMA_DONE) != DMA_DONE {
            return Err(Error::DmaStuck);
        }
        io.write(INT_STS, DMA_DONE);
    }
    clear_done_cnt(io);
    Ok(())
}

fn clear_done_cnt<I: Io>(io: &mut I) {
    let st = io.read(STATUS);
    if (st & DMA_DONE_CNT) != 0 {
        io.write(STATUS, st | DMA_DONE_CNT);
    }
}

fn poll_mask<I: Io>(io: &mut I, mask: u32, spins: u32) -> Result<(), Error> {
    let mut n = 0u32;
    loop {
        let isr = io.read(INT_STS);
        if let Some(e) = classify(isr) {
            return Err(e);
        }
        if (isr & mask) == mask {
            return Ok(());
        }
        if n == spins {
            return Err(Error::Timeout);
        }
        wdt::kick(io);
        n = n.saturating_add(1);
    }
}

#[cfg(all(test, feature = "std"))]
mod host_tests;
#[cfg(all(test, feature = "std"))]
mod pcap_tests;
