// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial

use super::*;
use crate::platform::mmio::Io;
use regs::{
    AES_EN, AXI_RERR, AXI_RTO, AXI_WERR, AXI_WTO, BASE, CTRL, DMA_CMD_ERR, DMA_DONE, DMA_DONE_CNT,
    DMA_DST, DMA_DST_LEN, DMA_Q_EMPTY, DMA_Q_FULL, DMA_Q_OV, DMA_SRC, DMA_SRC_LEN, HMAC_ERR,
    INT_STS, LEN_MASK, LOOPBACK, MCTRL, P2D_LEN, PCFG_DONE, PCFG_INIT, PL_DEST, RATE_EN, RX_OV,
    SEU_ERR, STATUS, UNLOCK, UNLOCK_KEY,
};
use std::collections::BTreeMap;

const SWDT_MODE: u32 = 0xF800_5000;
const SWDT_RESTART: u32 = 0xF800_5008;
const SRC: u32 = 0x0010_0000;
const DST: u32 = 0x0300_0000;
const IMG: u32 = 8;
const DATA: u32 = 4;

#[derive(Clone, Copy, Debug)]
enum Acc {
    R(u32),
    W(u32, u32),
}

struct Fake {
    mem: BTreeMap<u32, u32>,
    log: Vec<Acc>,
    defer: u32,
    left: u32,
    started: bool,
    complete: bool,
    pl_done: bool,
    inject: u32,
    finished: bool,
}

impl Fake {
    fn new() -> Self {
        let mut mem = BTreeMap::new();
        mem.insert(CTRL, 0x0400_0E00);
        mem.insert(MCTRL, LOOPBACK);
        mem.insert(STATUS, PCFG_INIT | DMA_Q_EMPTY);
        mem.insert(INT_STS, 0);
        mem.insert(SWDT_MODE, 0);
        Self {
            mem,
            log: Vec::new(),
            defer: 0,
            left: 0,
            started: false,
            complete: true,
            pl_done: false,
            inject: 0,
            finished: false,
        }
    }

    fn finish_dma(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        let isr = self.g(INT_STS) | DMA_DONE | self.inject;
        let isr = if self.pl_done { isr | PCFG_DONE } else { isr };
        self.mem.insert(INT_STS, isr);
        let st = self.g(STATUS);
        let cnt = ((st & DMA_DONE_CNT) >> 28).saturating_add(1).min(3);
        self.mem.insert(STATUS, (st & !DMA_DONE_CNT) | (cnt << 28));
    }

    fn g(&self, a: u32) -> u32 {
        *self.mem.get(&a).unwrap_or(&0)
    }

    fn writes(&self) -> Vec<(u32, u32)> {
        self.log
            .iter()
            .filter_map(|a| match *a {
                Acc::W(addr, val) => Some((addr, val)),
                Acc::R(_) => None,
            })
            .collect()
    }
}

impl Io for Fake {
    fn read(&mut self, addr: u32) -> u32 {
        if addr == INT_STS && self.started && self.complete && !self.finished {
            if self.left == 0 {
                self.finish_dma();
            } else {
                self.left -= 1;
            }
        }
        let v = self.g(addr);
        self.log.push(Acc::R(addr));
        v
    }

    fn write(&mut self, addr: u32, val: u32) {
        self.log.push(Acc::W(addr, val));
        if addr == INT_STS {
            let cur = self.g(INT_STS);
            self.mem.insert(INT_STS, cur & !val);
        } else if addr == STATUS {
            let cur = self.g(STATUS);
            self.mem.insert(STATUS, cur & !(val & DMA_DONE_CNT));
        } else {
            self.mem.insert(addr, val);
        }
        if addr == DMA_DST_LEN {
            self.started = true;
            self.left = self.defer;
            if self.complete && self.defer == 0 {
                self.finish_dma();
            }
        }
    }

    fn poll(&mut self, _addr: u32, _mask: u32) -> Result<(), u32> {
        panic!("secure_transfer must not use Io::poll");
    }

    fn delay_ms(&mut self, _ms: u32) {}
}

fn ps() -> Request {
    Request {
        src_phys: SRC,
        dest_phys: DST,
        image_words: IMG,
        data_words: DATA,
        target: Target::Ps,
        caches_off: true,
        fabric_prepared: false,
        poll_limit: 32,
    }
}

fn pl() -> Request {
    Request {
        src_phys: SRC,
        dest_phys: 0,
        image_words: IMG,
        data_words: DATA,
        target: Target::Pl,
        caches_off: true,
        fabric_prepared: true,
        poll_limit: 32,
    }
}

fn run(f: &mut Fake, req: Request) -> Result<(), Error> {
    secure_transfer(f, &req)
}

fn pos_w(log: &[(u32, u32)], addr: u32) -> usize {
    log.iter().position(|(a, _)| *a == addr).expect("write")
}

#[test]
fn valid_ps_sequence() {
    let mut f = Fake::new();
    assert_eq!(run(&mut f, ps()), Ok(()));
    let w = f.writes();
    assert_eq!(w[0], (UNLOCK, UNLOCK_KEY));
    assert_eq!(w[1], (INT_STS, 0xFFFF_FFFF));
    let src_i = pos_w(&w, DMA_SRC);
    assert_eq!(w[src_i], (DMA_SRC, SRC | 1));
    assert_eq!(w[src_i + 1], (DMA_DST, DST | 1));
    assert_eq!(w[src_i + 2], (DMA_SRC_LEN, IMG));
    assert_eq!(w[src_i + 3], (DMA_DST_LEN, DATA));
    assert!(src_i + 3 < w.len());
    let mctrl = w.iter().find(|(a, _)| *a == MCTRL).unwrap().1;
    assert_eq!(mctrl & LOOPBACK, 0);
    let ctrl = w.iter().find(|(a, _)| *a == CTRL).unwrap().1;
    assert_eq!(ctrl & RATE_EN, RATE_EN);
    assert_eq!(ctrl & AES_EN, AES_EN);
    let after = w.iter().skip(src_i + 4).filter(|(a, _)| *a == INT_STS);
    let mut found_done = false;
    for (_, v) in after {
        assert_ne!(*v, 0xFFFF_FFFF, "post-DMA W1C must not be all-ones");
        if *v == DMA_DONE {
            found_done = true;
        }
    }
    assert!(found_done);
    assert_eq!(f.g(STATUS) & DMA_DONE_CNT, 0);
    assert_eq!(f.g(INT_STS) & DMA_DONE, 0);
    assert!(f.log.iter().any(|a| matches!(a, Acc::R(INT_STS))));
}

#[test]
fn valid_pl_polls_pcfg_done() {
    let mut f = Fake::new();
    f.pl_done = true;
    assert_eq!(run(&mut f, pl()), Ok(()));
    let w = f.writes();
    let src_i = pos_w(&w, DMA_SRC);
    assert_eq!(w[src_i + 1], (DMA_DST, PL_DEST));
    let sts: Vec<u32> = w
        .iter()
        .skip(src_i + 4)
        .filter(|(a, _)| *a == INT_STS)
        .map(|(_, v)| *v)
        .collect();
    assert!(sts.contains(&DMA_DONE));
    assert!(sts.contains(&PCFG_DONE));
    assert!(
        sts.iter().position(|v| *v == DMA_DONE).unwrap()
            < sts.iter().position(|v| *v == PCFG_DONE).unwrap()
    );
}

#[test]
fn pl_without_fabric_refuses_before_mmio() {
    let mut f = Fake::new();
    let mut r = pl();
    r.fabric_prepared = false;
    assert_eq!(run(&mut f, r), Err(Error::FabricNotPrepared));
    assert!(f.log.is_empty());
}

#[test]
fn caches_on_refuses_before_mmio() {
    let mut f = Fake::new();
    let mut r = ps();
    r.caches_off = false;
    assert_eq!(run(&mut f, r), Err(Error::CachesOn));
    assert!(f.log.is_empty());
}

#[test]
fn unaligned_and_zero_and_not_encrypted() {
    let mut f = Fake::new();
    let mut r = ps();
    r.src_phys = SRC + 1;
    assert_eq!(run(&mut f, r), Err(Error::Unaligned));
    r = ps();
    r.dest_phys = DST + 2;
    assert_eq!(run(&mut f, r), Err(Error::Unaligned));
    r = ps();
    r.image_words = 0;
    assert_eq!(run(&mut f, r), Err(Error::ZeroLength));
    r = ps();
    r.data_words = 0;
    assert_eq!(run(&mut f, r), Err(Error::ZeroLength));
    r = ps();
    r.image_words = DATA;
    r.data_words = DATA;
    assert_eq!(run(&mut f, r), Err(Error::NotEncrypted));
    r = ps();
    r.image_words = 3;
    r.data_words = 4;
    assert_eq!(run(&mut f, r), Err(Error::NotEncrypted));
    r = ps();
    r.image_words = LEN_MASK + 1;
    assert_eq!(run(&mut f, r), Err(Error::LengthField));
    assert!(f.log.is_empty());
}

#[test]
fn ram_window_and_mmio_dest() {
    let mut f = Fake::new();
    let mut r = ps();
    r.dest_phys = BASE;
    assert_eq!(run(&mut f, r), Err(Error::DestNotRam));
    r = ps();
    r.dest_phys = 0xE000_0000;
    assert_eq!(run(&mut f, r), Err(Error::DestNotRam));
    r = ps();
    r.src_phys = 0xF800_0008;
    assert_eq!(run(&mut f, r), Err(Error::SrcNotRam));
    r = ps();
    r.dest_phys = PL_DEST;
    assert_eq!(run(&mut f, r), Err(Error::DestNotRam));
    r = ps();
    r.src_phys = 0;
    r.dest_phys = 0;
    assert_eq!(run(&mut f, r), Ok(()));
}

#[test]
fn missing_pcfg_init() {
    let mut f = Fake::new();
    f.mem.insert(STATUS, DMA_Q_EMPTY);
    assert_eq!(run(&mut f, ps()), Err(Error::PcfgInit));
}

#[test]
fn timeout_without_dma_done() {
    let mut f = Fake::new();
    f.complete = false;
    let mut r = ps();
    r.poll_limit = 2;
    assert_eq!(run(&mut f, r), Err(Error::Timeout));
}

#[test]
fn hmac_and_other_fatal_bits() {
    let cases = [
        (HMAC_ERR, Error::Hmac),
        (SEU_ERR, Error::Crc),
        (AXI_WERR, Error::AxiWerr),
        (AXI_RTO, Error::AxiRto),
        (AXI_RERR, Error::AxiRerr),
        (AXI_WTO, Error::AxiWto),
        (RX_OV, Error::RxOverflow),
        (DMA_CMD_ERR, Error::DmaCmd),
        (DMA_Q_OV, Error::DmaQueueOverflow),
        (P2D_LEN, Error::P2dLength),
    ];
    for (bit, want) in cases {
        let mut f = Fake::new();
        f.inject = bit;
        let got = run(&mut f, ps());
        assert_eq!(got, Err(want), "{bit:#x}");
        assert_ne!(got, Ok(()));
    }
}

#[test]
fn leftover_dma_done_cnt_cleared_via_status() {
    let mut f = Fake::new();
    f.mem.insert(STATUS, PCFG_INIT | DMA_Q_EMPTY | (1 << 28));
    assert_eq!(run(&mut f, ps()), Ok(()));
    let w = f.writes();
    let status_ws: Vec<(usize, u32)> = w
        .iter()
        .enumerate()
        .filter(|(_, (a, _))| *a == STATUS)
        .map(|(i, (_, v))| (i, *v))
        .collect();
    assert!(!status_ws.is_empty());
    assert!(status_ws[0].1 & DMA_DONE_CNT != 0);
    assert_eq!(f.g(STATUS) & DMA_DONE_CNT, 0);
}

#[test]
fn wdt_kick_when_enabled_else_noop() {
    let mut on = Fake::new();
    on.mem.insert(SWDT_MODE, 1);
    on.defer = 2;
    assert_eq!(run(&mut on, ps()), Ok(()));
    assert!(on
        .writes()
        .iter()
        .any(|(a, v)| *a == SWDT_RESTART && *v == 0x1999));

    let mut off = Fake::new();
    off.defer = 2;
    assert_eq!(run(&mut off, ps()), Ok(()));
    assert!(!off.writes().iter().any(|(a, _)| *a == SWDT_RESTART));
}

#[test]
fn dma_queue_full() {
    let mut f = Fake::new();
    f.mem.insert(STATUS, PCFG_INIT | DMA_Q_FULL);
    assert_eq!(run(&mut f, ps()), Err(Error::DmaQueueFull));
}

#[test]
fn error_debug_has_no_secrets() {
    let s = format!("{:?} {:?}", Error::Hmac, ps());
    let low = s.to_ascii_lowercase();
    assert!(!low.contains("nky"));
    assert!(!low.contains("secret"));
    assert!(!low.contains("key 0"));
}

#[test]
fn sys05_does_not_invoke_devcfg() {
    assert!(!include_str!("../sd/sys05.rs").contains("devcfg"));
    assert!(!include_str!("../bin/sys05_plat.rs").contains("devcfg"));
}

#[test]
fn sys06_calls_secure_transfer() {
    assert!(include_str!("../sd/sys06.rs").contains("secure_transfer"));
    assert!(!include_str!("../sd/sys05.rs").contains("secure_transfer"));
}
