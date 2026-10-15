// Copyright (c) 2026 LightSpeed Studio LLC // Golden Phase.
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial

use super::*;
use crate::platform::mmio::Io;
use regs::{
    AXI_RERR, AXI_RTO, AXI_WERR, AXI_WTO, CTRL, DMA_CMD_ERR, DMA_DONE, DMA_DONE_CNT, DMA_DST,
    DMA_DST_LEN, DMA_Q_EMPTY, DMA_Q_FULL, DMA_Q_OV, DMA_SRC, DMA_SRC_LEN, FPGA_RST_CTRL, HMAC_ERR,
    INT_MASK, INT_STS, LEN_MASK, LOOPBACK, LVL_POST, LVL_SHFTR_EN, MCTRL, P2D_LEN, PCAP_MODE,
    PCAP_PR, PCFG_DONE, PCFG_INIT, PL_DEST, PL_RESETS, POR_B, PROG_B, RATE_EN, RX_OV, SEU_ERR,
    SLCR_LOCK, SLCR_LOCK_KEY, SLCR_UNLOCK, SLCR_UNLOCK_KEY, STATUS, UNLOCK, UNLOCK_KEY,
};
use std::collections::BTreeMap;

const SRC: u32 = 0x0100_0000;
const WORDS: u32 = 8;

struct Fake {
    mem: BTreeMap<u32, u32>,
    log: Vec<(u32, u32)>,
    complete_dma: bool,
    complete_pl: bool,
    inject: u32,
}

impl Fake {
    fn new() -> Self {
        let mut mem = BTreeMap::new();
        mem.insert(CTRL, PCAP_MODE);
        mem.insert(MCTRL, POR_B | LOOPBACK);
        mem.insert(STATUS, PCFG_INIT | DMA_Q_EMPTY);
        mem.insert(INT_STS, 0);
        mem.insert(LVL_SHFTR_EN, 0);
        Self {
            mem,
            log: Vec::new(),
            complete_dma: true,
            complete_pl: true,
            inject: 0,
        }
    }

    fn g(&self, a: u32) -> u32 {
        *self.mem.get(&a).unwrap_or(&0)
    }

    fn writes(&self) -> &[(u32, u32)] {
        &self.log
    }
}

impl Io for Fake {
    fn read(&mut self, addr: u32) -> u32 {
        self.g(addr)
    }
    fn write(&mut self, addr: u32, val: u32) {
        self.log.push((addr, val));
        if addr == INT_STS {
            let cur = self.g(INT_STS);
            self.mem.insert(INT_STS, cur & !val);
            return;
        }
        if addr == STATUS {
            let cur = self.g(STATUS);
            self.mem.insert(STATUS, cur & !(val & DMA_DONE_CNT));
            return;
        }
        if addr == CTRL {
            let old = self.g(CTRL);
            self.mem.insert(CTRL, val);
            let st = self.g(STATUS);
            if (old & PROG_B) != 0 && (val & PROG_B) == 0 {
                self.mem.insert(STATUS, st & !PCFG_INIT);
            }
            if (val & PROG_B) != 0 {
                self.mem.insert(STATUS, self.g(STATUS) | PCFG_INIT);
            }
            return;
        }
        if addr == DMA_DST_LEN {
            self.mem.insert(addr, val);
            let mut isr = self.g(INT_STS) | self.inject;
            if self.complete_dma {
                isr |= DMA_DONE;
            }
            if self.complete_pl {
                isr |= PCFG_DONE;
            }
            self.mem.insert(INT_STS, isr);
            let st = self.g(STATUS);
            let cnt = ((st & DMA_DONE_CNT) >> 28).saturating_add(1).min(3);
            self.mem.insert(STATUS, (st & !DMA_DONE_CNT) | (cnt << 28));
            return;
        }
        self.mem.insert(addr, val);
    }
    fn poll(&mut self, addr: u32, mask: u32) -> Result<(), u32> {
        if (self.read(addr) & mask) == mask {
            Ok(())
        } else {
            Err(addr)
        }
    }
    fn delay_ms(&mut self, _ms: u32) {}
}

fn req() -> PlRequest {
    PlRequest {
        src_phys: SRC,
        words: WORDS,
        caches_off: true,
        poll_limit: 32,
    }
}

#[test]
fn fabric_init_pulses_prog_b_and_holds_pl_reset() {
    let mut f = Fake::new();
    fabric_init(&mut f).unwrap();
    assert!(f.writes().contains(&(UNLOCK, UNLOCK_KEY)));
    assert!(f.writes().contains(&(SLCR_UNLOCK, SLCR_UNLOCK_KEY)));
    assert!(f.writes().contains(&(FPGA_RST_CTRL, PL_RESETS)));
    assert!(f.writes().contains(&(LVL_SHFTR_EN, 0)));
    assert!(f.writes().contains(&(SLCR_LOCK, SLCR_LOCK_KEY)));
    let ctrl_w: Vec<u32> = f
        .writes()
        .iter()
        .filter(|(a, _)| *a == CTRL)
        .map(|(_, v)| *v)
        .collect();
    assert!(ctrl_w.iter().any(|v| (v & PROG_B) == 0));
    assert!(ctrl_w.last().copied().unwrap() & PROG_B != 0);
    assert_eq!(f.g(CTRL) & RATE_EN, 0);
    assert_eq!(f.g(CTRL) & (PCAP_PR | PCAP_MODE), PCAP_PR | PCAP_MODE);
}

#[test]
fn por_b_clear_is_fabric_power() {
    let mut f = Fake::new();
    f.mem.insert(MCTRL, LOOPBACK);
    assert_eq!(fabric_init(&mut f), Err(Error::FabricPower));
}

#[test]
fn unlock_unwritable_ctrl_fails() {
    struct Stuck;
    impl Io for Stuck {
        fn read(&mut self, addr: u32) -> u32 {
            if addr == CTRL {
                0
            } else if addr == MCTRL {
                POR_B
            } else {
                0
            }
        }
        fn write(&mut self, _a: u32, _v: u32) {}
        fn poll(&mut self, a: u32, _m: u32) -> Result<(), u32> {
            Err(a)
        }
        fn delay_ms(&mut self, _ms: u32) {}
    }
    assert_eq!(fabric_init(&mut Stuck), Err(Error::Unlock));
}

#[test]
fn program_pl_nonsecure_equal_words_dest_ffffffff() {
    let mut f = Fake::new();
    program_pl(&mut f, &req()).unwrap();
    assert!(f.writes().contains(&(DMA_SRC, SRC | 1)));
    assert!(f.writes().contains(&(DMA_DST, PL_DEST)));
    assert!(f.writes().contains(&(DMA_SRC_LEN, WORDS)));
    assert!(f.writes().contains(&(DMA_DST_LEN, WORDS)));
    assert!(f.writes().contains(&(INT_MASK, 0)));
    assert_eq!(f.g(CTRL) & RATE_EN, 0);
    assert_eq!(f.g(MCTRL) & LOOPBACK, 0);
    assert_eq!(f.g(FPGA_RST_CTRL), 0);
    assert_eq!(f.g(LVL_SHFTR_EN) & LVL_POST, LVL_POST);
    assert!(f.writes().contains(&(SLCR_LOCK, SLCR_LOCK_KEY)));
}

#[test]
fn program_pl_refuses_caches_and_unaligned() {
    let mut f = Fake::new();
    let mut r = req();
    r.caches_off = false;
    assert_eq!(program_pl(&mut f, &r), Err(Error::CachesOn));
    r.caches_off = true;
    r.src_phys = SRC + 1;
    assert_eq!(program_pl(&mut f, &r), Err(Error::Unaligned));
}

#[test]
fn init_stuck_high_is_initb() {
    struct StickyInit {
        inner: Fake,
    }
    impl Io for StickyInit {
        fn read(&mut self, addr: u32) -> u32 {
            if addr == STATUS {
                return self.inner.g(STATUS) | PCFG_INIT;
            }
            self.inner.read(addr)
        }
        fn write(&mut self, a: u32, v: u32) {
            self.inner.write(a, v);
        }
        fn poll(&mut self, a: u32, m: u32) -> Result<(), u32> {
            self.inner.poll(a, m)
        }
        fn delay_ms(&mut self, ms: u32) {
            self.inner.delay_ms(ms);
        }
    }
    let mut s = StickyInit { inner: Fake::new() };
    assert_eq!(fabric_init(&mut s), Err(Error::InitB));
}

const SYNC: [u8; 4] = [0xAA, 0x99, 0x55, 0x66];

fn traced(f: &mut Fake, r: PlRequest) -> (Result<(), Error>, PlDiag) {
    let mut d = PlDiag::new(&r, SYNC);
    let res = program_pl_traced(f, &r, SYNC, &mut d);
    (res, d)
}

#[test]
fn sync_file_bytes_le_word_is_665599aa() {
    assert_eq!(u32::from_le_bytes(SYNC), 0x6655_99AA);
    assert_eq!(u32::from_be_bytes(SYNC), 0xAA99_5566);
    let d = PlDiag::new(&req(), SYNC);
    assert_eq!(d.src_bytes, SYNC);
    assert_eq!(d.src_word_le, 0x6655_99AA);
    assert_eq!(d.src_phys, SRC);
    assert_eq!(d.words, WORDS);
}

#[test]
fn class_name_covers_every_error() {
    let all = [
        Error::Unaligned,
        Error::ZeroLength,
        Error::LengthField,
        Error::NotEncrypted,
        Error::DestNotRam,
        Error::SrcNotRam,
        Error::CachesOn,
        Error::FabricNotPrepared,
        Error::PcfgInit,
        Error::DmaQueueFull,
        Error::DmaStuck,
        Error::Timeout,
        Error::Hmac,
        Error::AxiWto,
        Error::AxiWerr,
        Error::AxiRto,
        Error::AxiRerr,
        Error::RxOverflow,
        Error::DmaCmd,
        Error::DmaQueueOverflow,
        Error::P2dLength,
        Error::Unlock,
        Error::FabricPower,
        Error::InitB,
        Error::Crc,
    ];
    assert_eq!(all.len(), 25);
    for e in all {
        let n = class_name(e);
        assert!(!n.is_empty(), "{e:?}");
        assert!(!n.as_bytes().contains(&b' '), "{n}");
    }
    assert_eq!(class_name(Error::Crc), "CRC");
    assert_eq!(class_name(Error::Hmac), "HMAC");
    assert_eq!(class_name(Error::InitB), "INITB");
    assert_eq!(class_name(Error::Timeout), "TIMEOUT");
    assert_eq!(class_name(Error::DmaQueueFull), "DMA_QUEUE");
}

#[test]
fn nonsecure_mmio_matches_fsbl_pcap_write() {
    let mut f = Fake::new();
    let (res, d) = traced(&mut f, req());
    res.unwrap();
    let w = f.writes();
    let dma = w.iter().position(|(a, _)| *a == DMA_SRC).expect("DMA_SRC");
    assert_eq!(w[dma], (DMA_SRC, SRC | 1));
    assert_eq!(w[dma + 1], (DMA_DST, PL_DEST));
    assert_eq!(w[dma + 2], (DMA_SRC_LEN, WORDS));
    assert_eq!(w[dma + 3], (DMA_DST_LEN, WORDS));
    let before = &w[..dma];
    let last_m = before.iter().rev().find(|(a, _)| *a == MCTRL).unwrap().1;
    let last_c = before.iter().rev().find(|(a, _)| *a == CTRL).unwrap().1;
    assert_eq!(last_m & LOOPBACK, 0);
    assert_eq!(last_c & RATE_EN, 0);
    assert!(before.iter().any(|(a, v)| *a == INT_MASK && *v == 0));
    assert_eq!(d.pre_init, 1);
    assert_eq!(d.pre_rst, PL_RESETS);
    assert_eq!(d.pre_lvl, 0);
    assert_eq!(d.dma_src, SRC | 1);
    assert_eq!(d.dma_dst, PL_DEST);
    assert_eq!(d.dma_slen, WORDS);
    assert_eq!(d.dma_dlen, WORDS);
    assert_eq!(d.post_done, 1);
    assert_eq!(d.post_init, 1);
    assert_eq!(d.class, None);
}

#[test]
fn program_pl_validate_typed_failures() {
    let cases = [
        (
            PlRequest {
                caches_off: false,
                ..req()
            },
            Error::CachesOn,
            "CACHES",
        ),
        (
            PlRequest {
                src_phys: SRC + 1,
                ..req()
            },
            Error::Unaligned,
            "UNALIGNED",
        ),
        (
            PlRequest {
                words: 0,
                ..req()
            },
            Error::ZeroLength,
            "ZERO_LENGTH",
        ),
        (
            PlRequest {
                words: LEN_MASK + 1,
                ..req()
            },
            Error::LengthField,
            "LENGTH",
        ),
        (
            PlRequest {
                src_phys: 0xF800_0008,
                ..req()
            },
            Error::SrcNotRam,
            "SRC",
        ),
    ];
    for (r, want, token) in cases {
        let mut f = Fake::new();
        let (got, d) = traced(&mut f, r);
        assert_eq!(got, Err(want), "{want:?}");
        assert_eq!(d.class, Some(want));
        assert_eq!(class_name(want), token);
        assert!(f.writes().is_empty(), "{want:?} must refuse before MMIO");
    }
}

#[test]
fn program_pl_unlock_and_fabric_power() {
    struct Stuck;
    impl Io for Stuck {
        fn read(&mut self, addr: u32) -> u32 {
            if addr == CTRL {
                0
            } else if addr == MCTRL {
                POR_B
            } else {
                0
            }
        }
        fn write(&mut self, _a: u32, _v: u32) {}
        fn poll(&mut self, a: u32, _m: u32) -> Result<(), u32> {
            Err(a)
        }
        fn delay_ms(&mut self, _ms: u32) {}
    }
    let mut d = PlDiag::new(&req(), SYNC);
    assert_eq!(
        program_pl_traced(&mut Stuck, &req(), SYNC, &mut d),
        Err(Error::Unlock)
    );
    assert_eq!(d.class, Some(Error::Unlock));
    assert_eq!(class_name(Error::Unlock), "UNLOCK");

    let mut f = Fake::new();
    f.mem.insert(MCTRL, LOOPBACK);
    let (got, d) = traced(&mut f, req());
    assert_eq!(got, Err(Error::FabricPower));
    assert_eq!(d.class, Some(Error::FabricPower));
    assert_eq!((d.pre_mctrl & POR_B), 0);
}

#[test]
fn program_pl_initb_sticky_high() {
    struct StickyInit {
        inner: Fake,
    }
    impl Io for StickyInit {
        fn read(&mut self, addr: u32) -> u32 {
            if addr == STATUS {
                return self.inner.g(STATUS) | PCFG_INIT;
            }
            self.inner.read(addr)
        }
        fn write(&mut self, a: u32, v: u32) {
            self.inner.write(a, v);
        }
        fn poll(&mut self, a: u32, m: u32) -> Result<(), u32> {
            self.inner.poll(a, m)
        }
        fn delay_ms(&mut self, ms: u32) {
            self.inner.delay_ms(ms);
        }
    }
    let mut s = StickyInit { inner: Fake::new() };
    let mut d = PlDiag::new(&req(), SYNC);
    assert_eq!(
        program_pl_traced(&mut s, &req(), SYNC, &mut d),
        Err(Error::InitB)
    );
    assert_eq!(d.class, Some(Error::InitB));
}

#[test]
fn program_pl_pcfg_init_after_fabric() {
    struct DropInit {
        inner: Fake,
        prog_fell: bool,
        post_rise: bool,
    }
    impl Io for DropInit {
        fn read(&mut self, addr: u32) -> u32 {
            let v = self.inner.read(addr);
            if addr == STATUS && self.prog_fell && (v & PCFG_INIT) != 0 {
                if self.post_rise {
                    return v & !PCFG_INIT;
                }
                self.post_rise = true;
            }
            v
        }
        fn write(&mut self, a: u32, v: u32) {
            if a == CTRL {
                let old = self.inner.g(CTRL);
                if (old & PROG_B) != 0 && (v & PROG_B) == 0 {
                    self.prog_fell = true;
                }
            }
            self.inner.write(a, v);
        }
        fn poll(&mut self, a: u32, m: u32) -> Result<(), u32> {
            self.inner.poll(a, m)
        }
        fn delay_ms(&mut self, ms: u32) {
            self.inner.delay_ms(ms);
        }
    }
    let mut s = DropInit {
        inner: Fake::new(),
        prog_fell: false,
        post_rise: false,
    };
    let mut d = PlDiag::new(&req(), SYNC);
    assert_eq!(
        program_pl_traced(&mut s, &req(), SYNC, &mut d),
        Err(Error::PcfgInit)
    );
    assert_eq!(d.class, Some(Error::PcfgInit));
    assert_eq!(d.pre_init, 0);
}

#[test]
fn program_pl_queue_stuck_timeout_crc() {
    let mut q = Fake::new();
    q.mem.insert(STATUS, PCFG_INIT | DMA_Q_FULL);
    let (got, d) = traced(&mut q, req());
    assert_eq!(got, Err(Error::DmaQueueFull));
    assert_eq!(d.class, Some(Error::DmaQueueFull));

    let mut stuck = Fake::new();
    stuck.mem.insert(STATUS, PCFG_INIT);
    let (got, d) = traced(&mut stuck, req());
    assert_eq!(got, Err(Error::DmaStuck));
    assert_eq!(d.class, Some(Error::DmaStuck));

    let mut t = Fake::new();
    t.complete_dma = false;
    t.complete_pl = false;
    let mut r = req();
    r.poll_limit = 2;
    let (got, d) = traced(&mut t, r);
    assert_eq!(got, Err(Error::Timeout));
    assert_eq!(d.class, Some(Error::Timeout));
    assert_eq!(d.dma_src, SRC | 1);
    assert_eq!(d.dma_dst, PL_DEST);
    assert_eq!(d.dma_slen, WORDS);
    assert_eq!(d.dma_dlen, WORDS);
    assert_eq!(d.post_done, 0);

    let mut crc = Fake::new();
    crc.complete_pl = false;
    let mut r = req();
    r.poll_limit = 2;
    let (got, d) = traced(&mut crc, r);
    assert_eq!(got, Err(Error::Crc));
    assert_eq!(d.class, Some(Error::Crc));
    assert_eq!(d.post_done, 0);
}

#[test]
fn program_pl_fatal_isr_typed_failures() {
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
        let (got, d) = traced(&mut f, req());
        assert_eq!(got, Err(want), "{bit:#x}");
        assert_eq!(d.class, Some(want), "{bit:#x}");
        assert_eq!(d.post_int & bit, bit, "{bit:#x}");
    }
}
