//! Zynq-7000 GEM0 ladder (PZ7020 ENET0).
//!
//! Proven against GP-OS-V5 COM12 seal: MDIO scan + MAC local loopback.
//! Base `0xE000B000`, MDIO MIO 52–53, RGMII RTL8211F family (scan, not hard-coded addr).
//!
//! PN-WIRE-1: multi-BD external raw EtherType `0x88B5` TX/RX (no GEM reinit between
//! request and reply). Not a PhaseNet codec; not an IP stack.
//!
//! Classifications:
//! - register / MDIO / PHY ID / MAC loopback: eligible for PHYSICALLY VERIFIED after COM12
//! - link-up: ETH-1 physical PASS when partner present
//! - external TX/RX: PN-WIRE-1 evidence only (never claim from MAC loopback)
//! - software-only claims are never "physical Ethernet"

use super::memory_map::GEM0_BASE;
use crate::board::gem_cadence_hw::{dmacr as gem_dmacr, nwcfg as gem_nwcfg, rxbd as gem_rxbd};
use crate::platform::String;
use crate::protocol::pnw1_wire::{
    self, BOARD_MAC, ETHERTYPE as GP_ETHERTYPE, FRAME_LEN as WIRE_FRAME_LEN,
};

const NWCTRL: u32 = 0x00;
const NWCFG: u32 = 0x04;
const NWSR: u32 = 0x08;
const DMACR: u32 = 0x10;
const TXSR: u32 = 0x14;
const RXQBASE: u32 = 0x18;
const TXQBASE: u32 = 0x1C;
const RXSR: u32 = 0x20;
const ISR: u32 = 0x24;
const IER: u32 = 0x28;
const IDR: u32 = 0x2C;

/// GEM interrupt/status bit 1 = "Receive complete" (frame received). Same bit
/// position in ISR/IER/IDR; RXSR bit 1 = "Frame received".
const INT_RX_COMPLETE: u32 = 1 << 1;
const RXSR_FRAME_RX: u32 = 1 << 1;
/// TXSR: used-bit read / retry / complete / underrun / hresp (UG585).
const TXSR_USED_READ: u32 = 1 << 0;
const TXSR_RETRY_EXC: u32 = 1 << 2;
const TXSR_TXGO: u32 = 1 << 3;
const TXSR_COMPLETE: u32 = 1 << 5;
const TXSR_UNDERRUN: u32 = 1 << 6;
const TXSR_HRESP: u32 = 1 << 8;
const PHYMNTNC: u32 = 0x34;
const SA1B: u32 = 0x88;
const SA1T: u32 = 0x8C;

const NWCTRL_LOOP: u32 = 1 << 1;
const NWCTRL_RXEN: u32 = 1 << 2;
const NWCTRL_TXEN: u32 = 1 << 3;
const NWCTRL_MDEN: u32 = 1 << 4;

const NWSR_MDIOIDLE: u32 = 1 << 2;

/// Cadence Clause-22: SOF|READ|CODE(0b10).
const MAN_READ: u32 = 0x6000_0000 | 0x0002_0000;
/// Cadence Clause-22: SOF|WRITE|CODE(0b10).
const MAN_WRITE: u32 = 0x5000_0000 | 0x0002_0000;

const MDIO_TIMEOUT: u32 = 0x000F_4240; // matches V5 phy_read budget

const PHY_REG_BMCR: u8 = 0;
const PHY_REG_BMSR: u8 = 1;
const PHY_REG_ID1: u8 = 2;
const PHY_REG_ID2: u8 = 3;
const PHY_REG_ANLPAR: u8 = 5;
const PHY_REG_GBSR: u8 = 10;
/// RTL8211F page-select register (Clause-22).
const PHY_REG_PAGE_SELECT: u8 = 0x1F;
/// RTL8211F PHY Specific Status on page `0xA43`.
const PHY_REG_RTL8211F_PHYSR: u8 = 0x1A;
const RTL8211F_PAGE_PHYSR: u16 = 0x0A43;
/// Datasheet Table 22 BMCR: bit14 PCS loopback (RGMII TX → RGMII RX).
const BMCR_LOOPBACK: u16 = 1 << 14;
const BMCR_SPEED100: u16 = 1 << 13; // with Speed[1]=0 → 100 Mbps
const BMCR_ANE: u16 = 1 << 12;
const BMCR_PWD: u16 = 1 << 11;
const BMCR_ISOLATE: u16 = 1 << 10;
const BMCR_RESTART_AN: u16 = 1 << 9;
const BMCR_DUPLEX: u16 = 1 << 8;
const BMCR_SPEED1000: u16 = 1 << 6; // Speed[1]
const BMSR_LINK: u16 = 1 << 2;
const BMSR_AN_COMPLETE: u16 = 1 << 5;

/// RTL8211F RGMII delay page and registers (Clause-22 ext page 0x0D08).
const PHY_PAGE_RGMII_DELAY: u16 = 0x0D08;
const PHY_REG_TXDLY: u8 = 0x11; // bit8 = add 2ns TX (TXC->TXD) delay
const PHY_REG_RXDLY: u8 = 0x15; // bit3 = add 2ns RX delay
const TXDLY_EN: u16 = 1 << 8;
const RXDLY_EN: u16 = 1 << 3;

/// SLCR (System Level Control) — GEM0 RGMII TX clock divisors.
/// On Zynq-7000 the RGMII TXC frequency comes from GEM0_CLK_CTRL, not NWCFG.
/// Mirrors Xilinx `SetUpSLCRDivisors` (xemacpsif_physpeed.c).
const SLCR_LOCK_ADDR: u32 = 0xF800_0004;
const SLCR_UNLOCK_ADDR: u32 = 0xF800_0008;
const SLCR_GEM0_CLK_CTRL: u32 = 0xF800_0140;
const SLCR_LOCK_KEY: u32 = 0x0000_767B;
const SLCR_UNLOCK_KEY: u32 = 0x0000_DF0D;
/// Preserve all bits except DIVISOR1[25:20] and DIVISOR0[13:8].
const SLCR_GEM_DIV_MASK: u32 = 0xFC0F_C0FF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GemError {
    MdioTimeout,
    NoPhy,
    LoopbackTimeout,
    LoopbackMismatch,
    WireNotInit,
    LinkDown,
    FrameTooLong,
    TxTimeout,
    TxBusy,
    RxTimeout,
    RxMismatch,
    ReinitDuringWire,
    DmaQuarantine,
}

impl GemError {
    pub fn as_str(self) -> &'static str {
        match self {
            GemError::MdioTimeout => "MDIO timeout",
            GemError::NoPhy => "no PHY ID on MDIO bus",
            GemError::LoopbackTimeout => "MAC loopback timeout",
            GemError::LoopbackMismatch => "MAC loopback payload mismatch",
            GemError::WireNotInit => "wire path not initialized (net wire init)",
            GemError::LinkDown => "PHY link DOWN",
            GemError::FrameTooLong => "frame exceeds wire buffer",
            GemError::TxTimeout => "TX descriptor timeout",
            GemError::TxBusy => "TX ring full",
            GemError::RxTimeout => "no REP heard (host peer or wire path)",
            GemError::RxMismatch => "RX frame bytes mismatch",
            GemError::ReinitDuringWire => "GEM reinit between TX and RX (forbidden)",
            GemError::DmaQuarantine => "dma containment refused descriptor (not IOMMU)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhyInfo {
    pub addr: u8,
    pub id1: u16,
    pub id2: u16,
}

/// Step-4 link report: BMSR plus negotiated speed/duplex when decodeable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkInfo {
    pub up: bool,
    pub bmsr: u16,
    /// Negotiated speed in Mb/s (`0` if link down or unknown).
    pub speed_mbps: u16,
    pub full_duplex: bool,
    /// True when speed/duplex came from a known PHY status decode.
    pub speed_known: bool,
}

impl LinkInfo {
    pub fn duplex_str(self) -> &'static str {
        if !self.up {
            "n/a"
        } else if !self.speed_known {
            "unknown"
        } else if self.full_duplex {
            "full"
        } else {
            "half"
        }
    }

    pub fn speed_str(self) -> &'static str {
        if !self.up {
            "n/a"
        } else if !self.speed_known {
            "unknown"
        } else {
            match self.speed_mbps {
                10 => "10",
                100 => "100",
                1000 => "1000",
                _ => "unknown",
            }
        }
    }
}

#[inline]
unsafe fn rr(off: u32) -> u32 {
    core::ptr::read_volatile((GEM0_BASE + off) as *const u32)
}

#[inline]
unsafe fn wr(off: u32, val: u32) {
    core::ptr::write_volatile((GEM0_BASE + off) as *mut u32, val);
}

fn wait_mdio_idle() -> Result<(), GemError> {
    for _ in 0..MDIO_TIMEOUT {
        if (unsafe { rr(NWSR) } & NWSR_MDIOIDLE) != 0 {
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err(GemError::MdioTimeout)
}

/// Step 1 — register presence (GEM revision-ish via NWCFG/NWSR readable).
pub fn probe_registers() -> (u32, u32, u32) {
    enable_mdio_clock();
    let nwctrl = unsafe { rr(NWCTRL) };
    let nwcfg = unsafe { rr(NWCFG) };
    let nwsr = unsafe { rr(NWSR) };
    (nwctrl, nwcfg, nwsr)
}

fn enable_mdio_clock() {
    // NWCFG: MDC÷48 + FCSREM + FD + 100. Do NOT force gigabit — that breaks
    // 100 Mbps RGMII wire TX when the partner is 100M.
    // Preserve RXEN/TXEN when the multi-BD wire path is live (MDIO must not kill TX).
    let cfg = gem_nwcfg::for_mdio();
    let live = unsafe { WIRE_LIVE };
    unsafe {
        if !live {
            wr(NWCFG, cfg);
        }
        let cur = rr(NWCTRL);
        let keep = if live {
            cur & (NWCTRL_RXEN | NWCTRL_TXEN)
        } else {
            0
        };
        wr(NWCTRL, NWCTRL_MDEN | keep);
    }
    // Short settle (V5 used ~10000 yields).
    for _ in 0..10_000 {
        core::hint::spin_loop();
    }
}

/// Step 2/3 — MDIO access + PHY ID (scan 0..31).
pub fn phy_read(phy: u8, reg: u8) -> Result<u16, GemError> {
    wait_mdio_idle()?;
    let man = MAN_READ | ((phy as u32) << 23) | ((reg as u32) << 18);
    unsafe { wr(PHYMNTNC, man) };
    wait_mdio_idle()?;
    let data = (unsafe { rr(PHYMNTNC) } & 0xFFFF) as u16;
    Ok(data)
}

pub fn phy_write(phy: u8, reg: u8, value: u16) -> Result<(), GemError> {
    wait_mdio_idle()?;
    let man = MAN_WRITE | ((phy as u32) << 23) | ((reg as u32) << 18) | (value as u32);
    unsafe { wr(PHYMNTNC, man) };
    wait_mdio_idle()?;
    Ok(())
}

pub fn scan_phy() -> Result<PhyInfo, GemError> {
    enable_mdio_clock();
    for addr in 0u8..32 {
        match phy_read(addr, PHY_REG_ID1) {
            Ok(id1) if id1 != 0 && id1 != 0xFFFF => {
                let id2 = phy_read(addr, PHY_REG_ID2).unwrap_or(0xFFFF);
                return Ok(PhyInfo { addr, id1, id2 });
            }
            _ => {}
        }
    }
    Err(GemError::NoPhy)
}

/// Step 4 — BMSR link bit (may be down without cable → report honestly).
///
/// Also attempts negotiated speed/duplex decode (RTL8211F PHYSR, then
/// Clause-22 ANLPAR/GBSR fallback). Does not enable external TX/RX.
pub fn link_status(phy: u8) -> Result<LinkInfo, GemError> {
    // Read twice; latch bit clears on first read for many PHYs.
    let _ = phy_read(phy, PHY_REG_BMSR)?;
    let bmsr = phy_read(phy, PHY_REG_BMSR)?;
    let up = (bmsr & BMSR_LINK) != 0;
    if !up {
        return Ok(LinkInfo {
            up: false,
            bmsr,
            speed_mbps: 0,
            full_duplex: false,
            speed_known: false,
        });
    }

    if let Some((speed, full)) = read_rtl8211f_physr(phy) {
        return Ok(LinkInfo {
            up: true,
            bmsr,
            speed_mbps: speed,
            full_duplex: full,
            speed_known: true,
        });
    }

    if let Some((speed, full)) = decode_clause22_speed_duplex(phy) {
        return Ok(LinkInfo {
            up: true,
            bmsr,
            speed_mbps: speed,
            full_duplex: full,
            speed_known: true,
        });
    }

    Ok(LinkInfo {
        up: true,
        bmsr,
        speed_mbps: 0,
        full_duplex: false,
        speed_known: false,
    })
}

/// Decode RTL8211F PHYSR bits (pure; used by hardware read + host tests).
pub fn decode_rtl8211f_physr_bits(physr: u16) -> Option<(u16, bool)> {
    let speed = match (physr >> 4) & 0x3 {
        0 => 10u16,
        1 => 100,
        2 => 1000,
        _ => return None,
    };
    let full = (physr & (1 << 3)) != 0;
    Some((speed, full))
}

/// RTL8211F: page `0xA43`, reg `0x1A` — bits[5:4]=speed, bit3=duplex.
fn read_rtl8211f_physr(phy: u8) -> Option<(u16, bool)> {
    let _ = phy_write(phy, PHY_REG_PAGE_SELECT, RTL8211F_PAGE_PHYSR);
    let physr = phy_read(phy, PHY_REG_RTL8211F_PHYSR).ok()?;
    let _ = phy_write(phy, PHY_REG_PAGE_SELECT, 0);
    decode_rtl8211f_physr_bits(physr)
}

/// Clause-22 fallback from GBSR (1000) then ANLPAR (100/10).
fn decode_clause22_speed_duplex(phy: u8) -> Option<(u16, bool)> {
    let gbsr = phy_read(phy, PHY_REG_GBSR).ok()?;
    // 1000BASE-T status: LP able full (bit 11) / half (bit 10).
    if (gbsr & (1 << 11)) != 0 {
        return Some((1000, true));
    }
    if (gbsr & (1 << 10)) != 0 {
        return Some((1000, false));
    }
    let anlpar = phy_read(phy, PHY_REG_ANLPAR).ok()?;
    // IEEE 802.3 ANLPAR: 100FULL=8, 100HALF=7, 10FULL=6, 10HALF=5.
    if (anlpar & (1 << 8)) != 0 {
        return Some((100, true));
    }
    if (anlpar & (1 << 7)) != 0 {
        return Some((100, false));
    }
    if (anlpar & (1 << 6)) != 0 {
        return Some((10, true));
    }
    if (anlpar & (1 << 5)) != 0 {
        return Some((10, false));
    }
    None
}

/// Enable the RTL8211F RGMII TX/RX delays (ext page `0x0D08`): TXDLY (reg 0x11
/// bit8) and RXDLY (reg 0x15 bit3).
///
/// On Zynq-7000 the PS GEM adds no internal RGMII delay, so the ~2 ns TXC->TXD
/// skew must come from the PHY. The seller's own data showed TXDLY OFF
/// (reg 0x11 = 0x0009) — which corrupts gigabit TX at the PHY even though the MAC
/// reports "complete". Mirrors the PL demo `mdio_ctrl.v` `rgmii-id` programming.
///
/// Quick MDIO writes only — no PHY reset and no blocking wait (a long wait here
/// starves the Zynq watchdog and reboots the board). Best-effort.
fn phy_set_rgmii_delay(phy: u8) {
    let _ = phy_write(phy, PHY_REG_PAGE_SELECT, PHY_PAGE_RGMII_DELAY);
    if let Ok(r) = phy_read(phy, PHY_REG_TXDLY) {
        let _ = phy_write(phy, PHY_REG_TXDLY, r | TXDLY_EN);
    }
    let _ = phy_write(phy, PHY_REG_PAGE_SELECT, PHY_PAGE_RGMII_DELAY);
    if let Ok(r) = phy_read(phy, PHY_REG_RXDLY) {
        let _ = phy_write(phy, PHY_REG_RXDLY, r | RXDLY_EN);
    }
    let _ = phy_write(phy, PHY_REG_PAGE_SELECT, 0x0000);
}

// DMA rings in .bss. D-cache/MMU are on after CPU1; MAC loopback must clean/invalidate.
#[derive(Clone, Copy)]
#[repr(C, align(8))]
struct Bd {
    addr: u32,
    status: u32,
}

// --- MAC loopback (1-BD; not wire proof) ---------------------------------
static mut TX_BD: Bd = Bd { addr: 0, status: 0 };
static mut RX_BD: Bd = Bd { addr: 0, status: 0 };
static mut TX_BUF: [u8; 160] = [0; 160];
static mut RX_BUF: [u8; 160] = [0; 160];

// --- PN-WIRE-1 multi-BD external path ------------------------------------
const WIRE_RING: usize = 4;
/// GEM wire DMA buffer size. Full-MTU (1536 = 24×64B RXBUF units) so PhaseLink
/// frames fit — the small PNW1 probe frames (64B) still use only what they need.
pub const WIRE_BUF_LEN: usize = 1536;
const TX_USED: u32 = 1 << 31;
const TX_WRAP: u32 = 1 << 30;
const TX_LAST: u32 = 1 << 15;
const NWCTRL_TXSTART: u32 = 1 << 9;
// NWCFG / DMACR bit names: see `crate::board::gem_cadence_hw` (xemacps_hw.h).
// Old wrong labels (REMOVED): FCS_REMOVE=1<<4 (was COPYALLEN), COPY_ALL=1<<13 (was PAUSEEN).

static mut WIRE_TX_BD: [Bd; WIRE_RING] = [Bd { addr: 0, status: 0 }; WIRE_RING];
static mut WIRE_RX_BD: [Bd; WIRE_RING] = [Bd { addr: 0, status: 0 }; WIRE_RING];
static mut WIRE_TX_BUF: [[u8; WIRE_BUF_LEN]; WIRE_RING] = [[0; WIRE_BUF_LEN]; WIRE_RING];
static mut WIRE_RX_BUF: [[u8; WIRE_BUF_LEN]; WIRE_RING] = [[0; WIRE_BUF_LEN]; WIRE_RING];

static mut WIRE_LIVE: bool = false;
static mut WIRE_INIT_COUNT: u32 = 0;
static mut WIRE_TX_HEAD: usize = 0;
static mut WIRE_RX_HEAD: usize = 0;
static mut WIRE_SEQ: u8 = 0;
static mut WIRE_TX_WRAPS: u32 = 0;
static mut WIRE_RX_WRAPS: u32 = 0;
static mut GEM_DMA: crate::kernel::dma_contain::DmaContainment =
    crate::kernel::dma_contain::DmaContainment::new();

/// Snapshot for acceptance: no GEM reinit between TX and RX reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireStatus {
    pub live: bool,
    pub init_count: u32,
    pub tx_head: usize,
    pub rx_head: usize,
    pub ring: usize,
    pub tx_wraps: u32,
    pub rx_wraps: u32,
    pub seq: u8,
    pub loop_bit: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireRoundtripReport {
    pub tx_len: usize,
    pub rx_len: usize,
    pub seq: u8,
    pub init_count_before: u32,
    pub init_count_after: u32,
    pub tx_bd: usize,
    pub rx_bd: usize,
    pub tx_wraps: u32,
    pub rx_wraps: u32,
    pub loop_bit: bool,
}

fn invalidate_wire_path() {
    unsafe {
        WIRE_LIVE = false;
    }
}

fn nwcfg_for_link(link: &LinkInfo) -> u32 {
    gem_nwcfg::for_link(link.speed_mbps, link.full_duplex, link.up)
}

fn wire_dmacr() -> u32 {
    // RXBUF units track WIRE_BUF_LEN (full-MTU 1536 = 24×64B units).
    gem_dmacr::build_for_buf_len(WIRE_BUF_LEN)
}

fn wire_traffic_bits() -> u32 {
    NWCTRL_RXEN | NWCTRL_TXEN | NWCTRL_MDEN
}

/// Bring up multi-BD GEM0 for external TX/RX (no MAC loopback).
///
/// Idempotent while live: returns Ok without reprogramming rings.
/// Call again after `net loopback` (which tears wire down) or link recover.
pub fn wire_init() -> Result<WireStatus, GemError> {
    if unsafe { WIRE_LIVE } {
        return Ok(wire_status());
    }

    enable_mdio_clock();
    let phy = scan_phy()?;
    let link = link_status(phy.addr)?;
    if !link.up {
        return Err(GemError::LinkDown);
    }
    // Enable the PHY RGMII TX/RX delay (the seller data showed TXDLY off) so the
    // MAC's frames are sampled correctly at the PHY. Quick MDIO — no reset/wait.
    phy_set_rgmii_delay(phy.addr);
    // Match the GEM TX clock divisor (SLCR) to the negotiated speed.
    set_gem_tx_clk(link.speed_mbps);

    let cfg = nwcfg_for_link(&link);
    unsafe {
        wr(NWCTRL, NWCTRL_MDEN);
        wr(IDR, 0xFFFF_FFFF);
        wr(TXSR, 0xFFFF_FFFF);
        wr(RXSR, 0xFFFF_FFFF);
        wr(NWCFG, cfg);
        wr(DMACR, wire_dmacr());
        // SA1 = 02:00:00:47:01:50 (little-endian bottom/top words, V5-compatible)
        wr(SA1B, 0x4700_0002);
        wr(SA1T, 0x0150);

        for i in 0..WIRE_RING {
            let tx_buf = core::ptr::addr_of_mut!(WIRE_TX_BUF[i]) as u32;
            let rx_buf = core::ptr::addr_of_mut!(WIRE_RX_BUF[i]) as u32;
            let rx_wrap = i + 1 == WIRE_RING;
            let tx_wrap = if i + 1 == WIRE_RING { TX_WRAP } else { 0 };
            WIRE_TX_BD[i].addr = tx_buf;
            WIRE_TX_BD[i].status = TX_USED | tx_wrap; // idle: owned by software
            WIRE_RX_BD[i].addr = gem_rxbd::addr_word_for_hw(rx_buf, rx_wrap);
            WIRE_RX_BD[i].status = 0;
        }
        GEM_DMA = crate::kernel::dma_contain::DmaContainment::new();
        for i in 0..WIRE_RING {
            let tx_buf = core::ptr::addr_of_mut!(WIRE_TX_BUF[i]) as u32;
            let rx_buf = core::ptr::addr_of_mut!(WIRE_RX_BUF[i]) as u32;
            let _ = GEM_DMA.register_pool(tx_buf, WIRE_BUF_LEN as u32);
            let _ = GEM_DMA.register_pool(rx_buf, WIRE_BUF_LEN as u32);
        }
    }

    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    wire_cache_commit_rings();

    unsafe {
        wr(TXQBASE, core::ptr::addr_of_mut!(WIRE_TX_BD[0]) as u32);
        wr(RXQBASE, core::ptr::addr_of_mut!(WIRE_RX_BD[0]) as u32);

        WIRE_TX_HEAD = 0;
        WIRE_RX_HEAD = 0;
        WIRE_TX_WRAPS = 0;
        WIRE_RX_WRAPS = 0;
        WIRE_INIT_COUNT = WIRE_INIT_COUNT.wrapping_add(1);
        WIRE_LIVE = true;

        wr(NWCTRL, wire_traffic_bits());
    }

    // I225 at 1G drops a prefix of the first idle burst (host bcast_delta=21,
    // dumpcap 0 drops; second burst after traffic is 32/32). Always prime so
    // a speed-decode miss cannot skip it.
    wire_prime_frames(WIRE_1G_PRIME)?;

    Ok(wire_status())
}

const WIRE_1G_PRIME: u32 = 16;

fn wire_prime_frames(count: u32) -> Result<(), GemError> {
    for _ in 0..count {
        let seq = unsafe {
            WIRE_SEQ = WIRE_SEQ.wrapping_add(1);
            WIRE_SEQ
        };
        let mut frame = [0u8; WIRE_FRAME_LEN];
        let len = pnw1_wire::build_frame(&mut frame, pnw1_wire::KIND_REQ, seq, &BOARD_MAC);
        wire_tx_raw(&frame[..len])?;
    }
    Ok(())
}

pub fn wire_status() -> WireStatus {
    let nwctrl = unsafe { rr(NWCTRL) };
    unsafe {
        WireStatus {
            live: WIRE_LIVE,
            init_count: WIRE_INIT_COUNT,
            tx_head: WIRE_TX_HEAD,
            rx_head: WIRE_RX_HEAD,
            ring: WIRE_RING,
            tx_wraps: WIRE_TX_WRAPS,
            rx_wraps: WIRE_RX_WRAPS,
            seq: WIRE_SEQ,
            loop_bit: (nwctrl & NWCTRL_LOOP) != 0,
        }
    }
}

/// Re-sync NWCFG to current link without resetting BD rings (disconnect/reconnect).
pub fn wire_recover_link() -> Result<WireStatus, GemError> {
    if !unsafe { WIRE_LIVE } {
        return wire_init();
    }
    let phy = scan_phy()?;
    let link = link_status(phy.addr)?;
    if !link.up {
        return Err(GemError::LinkDown);
    }
    let cfg = nwcfg_for_link(&link);
    unsafe {
        // Do not clear RXEN/TXEN rings; only refresh speed/duplex + keep traffic on.
        wr(NWCFG, cfg);
        wr(NWCTRL, wire_traffic_bits());
    }
    Ok(wire_status())
}

/// A16: after GEM released a used RX BD to the CPU. Software ring only;
/// does not store HwOwned; not IOMMU.
#[cfg(all(feature = "netsess", any(feature = "std", feature = "baremetal")))]
fn record_rx_take(frame_len: usize) {
    crate::kernel::netsess::note_gem_rx_taken(frame_len as u32);
}

#[cfg(not(all(feature = "netsess", any(feature = "std", feature = "baremetal"))))]
fn record_rx_take(_frame_len: usize) {}

/// Non-blocking RX poll (one BD). Used to drain before PHY loopback TX.
/// Ownership is address-word bit0 (xemacps NEW/USED), not status bit31.
fn wire_rx_poll(out: &mut [u8]) -> Option<usize> {
    if !unsafe { WIRE_LIVE } {
        return None;
    }
    let idx = unsafe { WIRE_RX_HEAD };
    let rx_buf = unsafe { core::ptr::addr_of_mut!(WIRE_RX_BUF[idx]) as u32 };
    let rx_bd = unsafe { core::ptr::addr_of_mut!(WIRE_RX_BD[idx]) as u32 };
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    dma_invalidate(rx_bd, 8);
    let (addr_w, st) = unsafe { (WIRE_RX_BD[idx].addr, WIRE_RX_BD[idx].status) };
    let parsed = gem_rxbd::take_frame(addr_w, st, WIRE_BUF_LEN).ok()?;
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    dma_invalidate(rx_buf, WIRE_BUF_LEN as u32);
    let copy = core::cmp::min(parsed.len, out.len());
    unsafe {
        out[..copy].copy_from_slice(&WIRE_RX_BUF[idx][..copy]);
        let rx_wrap = idx + 1 == WIRE_RING;
        WIRE_RX_BD[idx].status = 0;
        WIRE_RX_BD[idx].addr = gem_rxbd::return_to_hw(rx_buf, rx_wrap);
        WIRE_RX_HEAD = (idx + 1) % WIRE_RING;
        if WIRE_RX_HEAD == 0 {
            WIRE_RX_WRAPS = WIRE_RX_WRAPS.wrapping_add(1);
        }
    }
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    dma_clean(rx_bd, 8);
    record_rx_take(copy);
    Some(copy)
}

fn wire_rx_drain() {
    let mut tmp = [0u8; WIRE_BUF_LEN];
    for _ in 0..(WIRE_RING * 4) {
        if wire_rx_poll(&mut tmp).is_none() {
            break;
        }
    }
}

/// Repeat the same broadcast PNW1 frame on the existing wire TX path (scope probe aid).
pub fn wire_tx_burst(count: u32) -> Result<String, GemError> {
    let _ = wire_init()?;
    if (unsafe { rr(NWCTRL) } & NWCTRL_LOOP) != 0 {
        return Err(GemError::ReinitDuringWire);
    }
    let n = if count == 0 { 1 } else { count.min(256) };
    let mut sent = 0u32;
    for _ in 0..n {
        let seq = unsafe {
            WIRE_SEQ = WIRE_SEQ.wrapping_add(1);
            WIRE_SEQ
        };
        let mut frame = [0u8; WIRE_FRAME_LEN];
        let len = pnw1_wire::build_frame(&mut frame, pnw1_wire::KIND_REQ, seq, &BOARD_MAC);
        wire_tx_raw(&frame[..len])?;
        sent = sent.wrapping_add(1);
    }
    Ok(format!(
        "gem wire txburst: OK n={sent} ethertype=0x88B5 dst=ff:ff:ff:ff:ff:ff src=02:00:00:47:01:50 (reuse wire_tx_raw; not PhaseNet)"
    ))
}

// #region agent log
/// SLCR MMIO read (debug instrumentation for GEM↔PHY isolation).
#[inline]
unsafe fn slcr_rr(addr: u32) -> u32 {
    core::ptr::read_volatile(addr as *const u32)
}

/// SLCR MMIO write (GEM0 TX clock divisor programming).
#[inline]
unsafe fn slcr_wr(addr: u32, val: u32) {
    core::ptr::write_volatile(addr as *mut u32, val);
}

/// Program the GEM0 RGMII TX clock (SLCR GEM0_CLK_CTRL) for the negotiated speed.
///
/// The missing step vs the manufacturer driver: NWCFG's speed bit alone does not
/// change the physical TXC frequency — that comes from this divisor. Without it,
/// non-gigabit links transmit garbage and gigabit relies on the frozen FSBL value.
/// DIV0/DIV1 for a 1000 MHz IO-PLL source (gigabit proven live = `0x00100801`).
fn set_gem_tx_clk(speed_mbps: u16) {
    let (div1, div0): (u32, u32) = match speed_mbps {
        1000 => (1, 8),
        100 => (5, 8),
        10 => (50, 8),
        _ => return, // unknown speed: leave FSBL default untouched
    };
    unsafe {
        slcr_wr(SLCR_UNLOCK_ADDR, SLCR_UNLOCK_KEY);
        let cur = slcr_rr(SLCR_GEM0_CLK_CTRL) & SLCR_GEM_DIV_MASK;
        slcr_wr(SLCR_GEM0_CLK_CTRL, cur | (div1 << 20) | (div0 << 8));
        slcr_wr(SLCR_LOCK_ADDR, SLCR_LOCK_KEY);
    }
}

/// Seller PS7 expected values (from `ps7_init_data` / clock init).
const EXP_GEM0_RCLK: u32 = 0x0000_0001; // 0xF8000138
const EXP_GEM0_CLK: u32 = 0x0010_0801; // 0xF8000140
const EXP_MIO16: u32 = 0x0000_1202; // ENET0 start
const EXP_MIO22: u32 = 0x0000_1203; // ENET0 rx_clk region
const EXP_MIO52: u32 = 0x0000_1280; // MDC
const EXP_MIO53: u32 = 0x0000_1280; // MDIO

fn dbg_line(out: &mut String, hyp: &str, key: &str, detail: &str) {
    // Host capture parses lines starting with "#dbg " into debug-2a2f53.log
    out.push_str(&format!("#dbg hyp={hyp} key={key} {detail}\n"));
}

/// Decode NWCFG using Xilinx `xemacps_hw.h` bit names (hypothesis J).
fn dump_nwcfg_xilinx_bits(out: &mut String, nwcfg: u32) {
    // #region agent log
    let copyall = (nwcfg & gem_nwcfg::COPYALLEN) != 0;
    let pauseen = (nwcfg & gem_nwcfg::PAUSEEN) != 0;
    let fcsrem = (nwcfg & gem_nwcfg::FCSREM) != 0;
    let speed100 = (nwcfg & gem_nwcfg::SPEED100) != 0;
    let fd = (nwcfg & gem_nwcfg::FD) != 0;
    let gbit = (nwcfg & gem_nwcfg::GBIT) != 0;
    out.push_str(&format!(
        "NWCFG xilinx-bits: raw=0x{nwcfg:08X} SPEED100={speed100} FD={fd} GBIT={gbit} COPYALLEN={copyall} PAUSEEN={pauseen} FCSREM={fcsrem}\n"
    ));
    dbg_line(
        out,
        "J",
        "nwcfg_bits",
        &format!(
            "raw=0x{nwcfg:08X} COPYALLEN={copyall} PAUSEEN={pauseen} FCSREM={fcsrem} SPEED100={speed100} FD={fd}"
        ),
    );
    // #endregion
}

fn dump_slcr_gem_mio(out: &mut String) {
    let rclk = unsafe { slcr_rr(0xF800_0138) };
    let gclk = unsafe { slcr_rr(0xF800_0140) };
    let aper = unsafe { slcr_rr(0xF800_012C) };
    let mio16 = unsafe { slcr_rr(0xF800_0740) };
    let mio17 = unsafe { slcr_rr(0xF800_0744) };
    let mio22 = unsafe { slcr_rr(0xF800_0758) };
    let mio27 = unsafe { slcr_rr(0xF800_076C) };
    let mio52 = unsafe { slcr_rr(0xF800_07D0) };
    let mio53 = unsafe { slcr_rr(0xF800_07D4) };
    out.push_str("SLCR live vs seller PS7 expected:\n");
    out.push_str(&format!(
        "  GEM0_RCLK_CTRL@F8000138=0x{rclk:08X} exp=0x{EXP_GEM0_RCLK:08X} match={}\n",
        rclk == EXP_GEM0_RCLK
    ));
    out.push_str(&format!(
        "  GEM0_CLK_CTRL@F8000140=0x{gclk:08X} exp=0x{EXP_GEM0_CLK:08X} match={}\n",
        gclk == EXP_GEM0_CLK
    ));
    out.push_str(&format!("  APER_CLK_CTRL@F800012C=0x{aper:08X}\n"));
    out.push_str(&format!(
        "  MIO16@F8000740=0x{mio16:08X} exp=0x{EXP_MIO16:08X} match={}\n",
        mio16 == EXP_MIO16
    ));
    out.push_str(&format!("  MIO17@F8000744=0x{mio17:08X}\n"));
    out.push_str(&format!(
        "  MIO22@F8000758=0x{mio22:08X} exp=0x{EXP_MIO22:08X} match={}\n",
        mio22 == EXP_MIO22
    ));
    out.push_str(&format!("  MIO27@F800076C=0x{mio27:08X}\n"));
    out.push_str(&format!(
        "  MIO52@F80007D0=0x{mio52:08X} exp=0x{EXP_MIO52:08X} match={}\n",
        mio52 == EXP_MIO52
    ));
    out.push_str(&format!(
        "  MIO53@F80007D4=0x{mio53:08X} exp=0x{EXP_MIO53:08X} match={}\n",
        mio53 == EXP_MIO53
    ));
    dbg_line(
        out,
        "A",
        "slcr",
        &format!(
            "rclk=0x{rclk:08X} gclk=0x{gclk:08X} mio16=0x{mio16:08X} mio22=0x{mio22:08X} mio52=0x{mio52:08X} mio53=0x{mio53:08X} rclk_ok={} gclk_ok={} mio16_ok={} mio22_ok={} mdio_ok={}",
            rclk == EXP_GEM0_RCLK,
            gclk == EXP_GEM0_CLK,
            mio16 == EXP_MIO16,
            mio22 == EXP_MIO22,
            mio52 == EXP_MIO52 && mio53 == EXP_MIO53
        ),
    );
}

fn dump_rx_ring(out: &mut String, tag: &str) {
    let nwctrl = unsafe { rr(NWCTRL) };
    let nwcfg = unsafe { rr(NWCFG) };
    let rxq = unsafe { rr(RXQBASE) };
    let rxsr = unsafe { rr(RXSR) };
    let isr = unsafe { rr(ISR) };
    let head = unsafe { WIRE_RX_HEAD };
    out.push_str(&format!("RX ring {tag}:\n"));
    out.push_str(&format!(
        "  NWCTRL=0x{nwctrl:08X} RXEN={} TXEN={} LOOP={} NWCFG=0x{nwcfg:08X}\n",
        (nwctrl & NWCTRL_RXEN) != 0,
        (nwctrl & NWCTRL_TXEN) != 0,
        (nwctrl & NWCTRL_LOOP) != 0
    ));
    out.push_str(&format!(
        "  RXQBASE=0x{rxq:08X} rx_head={head} RXSR=0x{rxsr:08X} ISR=0x{isr:08X}\n"
    ));
    unsafe {
        for i in 0..WIRE_RING {
            let a = WIRE_RX_BD[i].addr;
            let s = WIRE_RX_BD[i].status;
            out.push_str(&format!(
                "  RXBD[{i}] addr=0x{a:08X} status=0x{s:08X} OWN(sw)={} WRAP={} SOF={} EOF={} len={}\n",
                gem_rxbd::owned_by_sw(a),
                gem_rxbd::wrap(a),
                (s & gem_rxbd::STATUS_SOF) != 0,
                (s & gem_rxbd::STATUS_EOF) != 0,
                s & gem_rxbd::STATUS_LEN
            ));
        }
    }
    dbg_line(
        out,
        "B",
        tag,
        &format!(
            "RXEN={} RXQBASE=0x{rxq:08X} head={head} RXSR=0x{rxsr:08X} ISR=0x{isr:08X} bd0_addr=0x{:08X} bd0_own={} bd0_status=0x{:08X}",
            (nwctrl & NWCTRL_RXEN) != 0,
            unsafe { WIRE_RX_BD[0].addr },
            unsafe { gem_rxbd::owned_by_sw(WIRE_RX_BD[0].addr) },
            unsafe { WIRE_RX_BD[0].status }
        ),
    );
}
// #endregion

/// Test A — RTL8211F PCS digital loopback (BMCR bit14, datasheet Table 22).
///
/// GEM TX → external RGMII → PHY PCS loop → RGMII RX → GEM RX.
/// Does not change descriptor-ring design or PhaseNet. Restores BMCR after.
pub fn wire_phy_pcs_loopback() -> Result<String, GemError> {
    let _ = wire_init()?;
    if (unsafe { rr(NWCTRL) } & NWCTRL_LOOP) != 0 {
        return Err(GemError::ReinitDuringWire);
    }

    enable_mdio_clock();
    let phy = scan_phy()?;
    let bmcr_before = phy_read(phy.addr, PHY_REG_BMCR)?;
    let bmsr_before = {
        let _ = phy_read(phy.addr, PHY_REG_BMSR)?;
        phy_read(phy.addr, PHY_REG_BMSR)?
    };

    // Datasheet-only status (no invented TX counters): PHYSR MDI plug / ALDPS.
    let _ = phy_write(phy.addr, PHY_REG_PAGE_SELECT, RTL8211F_PAGE_PHYSR);
    let physr = phy_read(phy.addr, PHY_REG_RTL8211F_PHYSR).unwrap_or(0);
    let _ = phy_write(phy.addr, PHY_REG_PAGE_SELECT, 0);

    let mut out = String::from(
        "gem wire phyloop — RTL8211F PCS digital loopback (diagnostic, not wire PASS)\n",
    );
    out.push_str("Authority: RTL8211F(D) datasheet Table 22 BMCR bit0.14 Loopback\n");
    out.push_str("  (RGMII transmit data routed to RGMII receive data path)\n");
    out.push_str(
        "Datasheet notes: ANE off for forced speed/duplex; link bit may be meaningless in PCS loopback\n",
    );
    // #region agent log
    dump_slcr_gem_mio(&mut out);
    // #endregion
    out.push_str(&format!(
        "PHY status before: addr={} BMCR=0x{:04X} BMSR=0x{:04X} link={} isolate={} pwd={} loop={} ane={}\n",
        phy.addr,
        bmcr_before,
        bmsr_before,
        (bmsr_before & BMSR_LINK) != 0,
        (bmcr_before & BMCR_ISOLATE) != 0,
        (bmcr_before & BMCR_PWD) != 0,
        (bmcr_before & BMCR_LOOPBACK) != 0,
        (bmcr_before & BMCR_ANE) != 0
    ));
    out.push_str(&format!(
        "PHYSR(page 0xA43 reg0x1A)=0x{:04X} ALDPS={} MDI_plug={} (no TX packet counter in datasheet)\n",
        physr,
        (physr & (1 << 14)) != 0,
        (physr & (1 << 13)) != 0
    ));
    out.push_str("PHY TX count: unknown (not defined in RTL8211F register map used here)\n");
    out.push_str("Test B remote/cable loopback: unsupported (no datasheet register found)\n");

    // Hypothesis F — datasheet Table 22 Note 1: changes to 0.12/0.13/0.6/0.8
    // take effect on Restart_AN (or reset/PWD). Soft-reset restores defaults, so
    // use Restart_AN to latch forced 100/full, then set PCS loopback (bit 0.14).
    let bmcr_force = BMCR_SPEED100 | BMCR_DUPLEX; // ANE=0, Speed1=0 → 100/full
    phy_write(phy.addr, PHY_REG_BMCR, bmcr_force)?;
    let bmcr_force_rb = phy_read(phy.addr, PHY_REG_BMCR)?;
    phy_write(phy.addr, PHY_REG_BMCR, bmcr_force | BMCR_RESTART_AN)?;
    for _ in 0..500_000 {
        core::hint::spin_loop();
    }
    let bmcr_latched = phy_read(phy.addr, PHY_REG_BMCR)?;
    let bmsr_latched = {
        let _ = phy_read(phy.addr, PHY_REG_BMSR)?;
        phy_read(phy.addr, PHY_REG_BMSR).unwrap_or(0)
    };
    out.push_str(&format!(
        "BMCR Note1 force: wrote=0x{:04X} read=0x{:04X} after_restart_an=0x{:04X} ane={} speed100={} duplex={} Speed1={}\n",
        bmcr_force,
        bmcr_force_rb,
        bmcr_latched,
        (bmcr_latched & BMCR_ANE) != 0,
        (bmcr_latched & BMCR_SPEED100) != 0,
        (bmcr_latched & BMCR_DUPLEX) != 0,
        (bmcr_latched & BMCR_SPEED1000) != 0
    ));
    out.push_str(&format!(
        "BMSR after Note1 latch: 0x{:04X} link={} an_complete={}\n",
        bmsr_latched,
        (bmsr_latched & BMSR_LINK) != 0,
        (bmsr_latched & BMSR_AN_COMPLETE) != 0
    ));
    // #region agent log
    dbg_line(
        &mut out,
        "F",
        "bmcr_note1",
        &format!(
            "force=0x{bmcr_force:04X} force_rb=0x{bmcr_force_rb:04X} latched=0x{bmcr_latched:04X} ane={} speed100={} duplex={} speed1={}",
            (bmcr_latched & BMCR_ANE) != 0,
            (bmcr_latched & BMCR_SPEED100) != 0,
            (bmcr_latched & BMCR_DUPLEX) != 0,
            (bmcr_latched & BMCR_SPEED1000) != 0
        ),
    );
    // #endregion

    let bmcr_lb = BMCR_LOOPBACK | BMCR_SPEED100 | BMCR_DUPLEX;
    phy_write(phy.addr, PHY_REG_BMCR, bmcr_lb)?;
    for _ in 0..200_000 {
        core::hint::spin_loop();
    }
    let bmcr_on = phy_read(phy.addr, PHY_REG_BMCR)?;
    let bmsr_on = {
        let _ = phy_read(phy.addr, PHY_REG_BMSR)?;
        phy_read(phy.addr, PHY_REG_BMSR).unwrap_or(0)
    };
    out.push_str(&format!(
        "BMCR loopback on: wrote=0x{:04X} readback=0x{:04X} loop={} isolate={} ane={} speed100={} duplex={} Speed1={}\n",
        bmcr_lb,
        bmcr_on,
        (bmcr_on & BMCR_LOOPBACK) != 0,
        (bmcr_on & BMCR_ISOLATE) != 0,
        (bmcr_on & BMCR_ANE) != 0,
        (bmcr_on & BMCR_SPEED100) != 0,
        (bmcr_on & BMCR_DUPLEX) != 0,
        (bmcr_on & BMCR_SPEED1000) != 0
    ));
    out.push_str(&format!(
        "BMSR during PCS loopback: 0x{:04X} link={} (may be meaningless in loopback)\n",
        bmsr_on,
        (bmsr_on & BMSR_LINK) != 0
    ));
    // #region agent log
    dbg_line(
        &mut out,
        "D",
        "bmcr_lb",
        &format!(
            "wrote=0x{bmcr_lb:04X} read=0x{bmcr_on:04X} bmsr=0x{bmsr_on:04X} link_during_lb={}",
            (bmsr_on & BMSR_LINK) != 0
        ),
    );
    dbg_line(
        &mut out,
        "F",
        "bmcr_lb_after_note1",
        &format!(
            "loop={} ane={} speed100={} duplex={} speed1={} matched_speed_bits={}",
            (bmcr_on & BMCR_LOOPBACK) != 0,
            (bmcr_on & BMCR_ANE) != 0,
            (bmcr_on & BMCR_SPEED100) != 0,
            (bmcr_on & BMCR_DUPLEX) != 0,
            (bmcr_on & BMCR_SPEED1000) != 0,
            (bmcr_on & (BMCR_SPEED100 | BMCR_DUPLEX | BMCR_ANE | BMCR_SPEED1000))
                == (BMCR_SPEED100 | BMCR_DUPLEX)
        ),
    );
    // #endregion

    wire_rx_drain();
    // Clear sticky RX/ISR before the probe TX so after bits are from this frame.
    unsafe {
        wr(RXSR, 0xFFFF_FFFF);
        let _ = rr(ISR); // clear-on-read
    }
    // #region agent log
    dump_rx_ring(&mut out, "before_tx");
    // #endregion

    let seq = unsafe {
        WIRE_SEQ = WIRE_SEQ.wrapping_add(1);
        WIRE_SEQ
    };
    let mut frame = [0u8; WIRE_FRAME_LEN];
    let frame_len = pnw1_wire::build_frame(&mut frame, pnw1_wire::KIND_REQ, seq, &BOARD_MAC);
    let tx_bd = match wire_tx_raw(&frame[..frame_len]) {
        Ok(i) => i,
        Err(e) => {
            let _ = phy_write(phy.addr, PHY_REG_BMCR, bmcr_before);
            return Err(e);
        }
    };
    let txsr = unsafe { rr(TXSR) };
    out.push_str(&format!(
        "TX: bd={tx_bd} len={frame_len} TXSR=0x{txsr:08X} complete={} underrun={} hresp={}\n",
        (txsr & TXSR_COMPLETE) != 0,
        (txsr & TXSR_UNDERRUN) != 0,
        (txsr & TXSR_HRESP) != 0
    ));

    let mut rx = [0u8; WIRE_BUF_LEN];
    let mut matched = false;
    let mut rx_len = 0usize;
    let mut any_rx = false;
    let mut reject_or_other = 0u32;
    let deadline_spins = MDIO_TIMEOUT;
    for _ in 0..deadline_spins {
        if let Some(n) = wire_rx_poll(&mut rx) {
            any_rx = true;
            rx_len = n;
            let et = frame_ethertype(&rx[..n]);
            let et_ok = et == Some(GP_ETHERTYPE);
            let src_ok = n >= 12 && rx[6..12] == BOARD_MAC;
            if et_ok && src_ok {
                matched = true;
                break;
            }
            reject_or_other = reject_or_other.wrapping_add(1);
            out.push_str(&format!(
                "RX other frame: len={n} et={:#06x} src={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}\n",
                et.unwrap_or(0),
                if n > 6 { rx[6] } else { 0 },
                if n > 7 { rx[7] } else { 0 },
                if n > 8 { rx[8] } else { 0 },
                if n > 9 { rx[9] } else { 0 },
                if n > 10 { rx[10] } else { 0 },
                if n > 11 { rx[11] } else { 0 }
            ));
            continue;
        }
        core::hint::spin_loop();
    }

    let rxsr_after = unsafe { rr(RXSR) };
    let isr_after = unsafe { rr(ISR) };
    // #region agent log
    dump_rx_ring(&mut out, "after_rx");
    dbg_line(
        &mut out,
        "C",
        "rx_events",
        &format!(
            "any_rx={any_rx} matched={matched} other={reject_or_other} RXSR=0x{rxsr_after:08X} ISR=0x{isr_after:08X}"
        ),
    );
    dbg_line(
        &mut out,
        "E",
        "match",
        &format!("matched={matched} any_rx={any_rx} last_len={rx_len} other={reject_or_other}"),
    );
    // #endregion

    // Restore prior BMCR (re-enable AN/link as before).
    let _ = phy_write(phy.addr, PHY_REG_BMCR, bmcr_before);
    for _ in 0..200_000 {
        core::hint::spin_loop();
    }
    let bmcr_after = phy_read(phy.addr, PHY_REG_BMCR).unwrap_or(0xFFFF);
    out.push_str(&format!(
        "BMCR restored: 0x{:04X} -> wrote 0x{:04X} -> read 0x{:04X}\n",
        bmcr_before, bmcr_before, bmcr_after
    ));

    if matched {
        out.push_str(&format!(
            "RX: MATCH len={rx_len} ethertype=0x88B5 src=02:00:00:47:01:50\n"
        ));
        out.push_str(
            "Verdict: PCS loopback received frame → GEM↔PHY RGMII TX/RX path works; fault is after PHY digital (analog/magnetics/jack/cable/PC)\n",
        );
    } else if any_rx {
        out.push_str(&format!(
            "RX: frames seen but NO MATCH (last_len={rx_len} other={reject_or_other})\n"
        ));
        out.push_str(
            "Verdict: RX DMA got data but filter/decode rejected PNW1 — check MAC filter vs looped frame bytes\n",
        );
    } else {
        out.push_str(&format!(
            "RX: NO MATCH (last_len={rx_len}) within timeout; RXSR=0x{rxsr_after:08X} ISR=0x{isr_after:08X}\n"
        ));
        out.push_str(
            "Verdict: PCS loopback got nothing → GEM↔PHY RGMII boundary or RX return still open; use SLCR/RXBD dumps + scope on TXC/TX_CTL/TXD\n",
        );
    }
    out.push_str(
        "Jack check: use PS GPHY jack (schematic U15 RTL8211F PS_GPHY / sheet GPHY(PS)); not PL U16 PL_GPHY\n",
    );
    Ok(out)
}

/// Queue one raw frame on the live multi-BD TX ring. Does not reinit GEM.
pub fn wire_tx_raw(frame: &[u8]) -> Result<usize, GemError> {
    if !unsafe { WIRE_LIVE } {
        return Err(GemError::WireNotInit);
    }
    if frame.len() > WIRE_BUF_LEN {
        return Err(GemError::FrameTooLong);
    }
    if (unsafe { rr(NWCTRL) } & NWCTRL_LOOP) != 0 {
        return Err(GemError::ReinitDuringWire);
    }

    if crate::kernel::dma_contain::gem_quarantined() {
        return Err(GemError::DmaQuarantine);
    }
    let idx = unsafe { WIRE_TX_HEAD };
    let tx_buf = unsafe { core::ptr::addr_of_mut!(WIRE_TX_BUF[idx]) as u32 };
    let tx_bd = unsafe { core::ptr::addr_of_mut!(WIRE_TX_BD[idx]) as u32 };
    // Wait until BD free (USED set by MAC after prior TX, or still idle USED).
    for _ in 0..MDIO_TIMEOUT {
        #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
        dma_invalidate(tx_bd, 8);
        let st = unsafe { WIRE_TX_BD[idx].status };
        if (st & TX_USED) != 0 {
            break;
        }
        core::hint::spin_loop();
    }
    if (unsafe { WIRE_TX_BD[idx].status } & TX_USED) == 0 {
        return Err(GemError::TxBusy);
    }
    let _ = unsafe { GEM_DMA.complete(tx_buf) };
    unsafe {
        if GEM_DMA
            .submit(
                tx_buf,
                frame.len() as u32,
                crate::kernel::dma_contain::kernel_dma_allowed(),
            )
            .is_err()
        {
            WIRE_LIVE = false;
            return Err(GemError::DmaQuarantine);
        }
    }

    unsafe {
        WIRE_TX_BUF[idx] = [0; WIRE_BUF_LEN];
        WIRE_TX_BUF[idx][..frame.len()].copy_from_slice(frame);
        let wrap = if idx + 1 == WIRE_RING { TX_WRAP } else { 0 };
        WIRE_TX_BD[idx].addr = tx_buf;
        // Clear USED so MAC owns the descriptor.
        WIRE_TX_BD[idx].status = (frame.len() as u32) | TX_LAST | wrap;
        WIRE_TX_HEAD = (idx + 1) % WIRE_RING;
        if WIRE_TX_HEAD == 0 {
            WIRE_TX_WRAPS = WIRE_TX_WRAPS.wrapping_add(1);
        }
    }
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        dma_clean(tx_buf, WIRE_BUF_LEN as u32);
        dma_clean(tx_bd, 8);
    }
    unsafe {
        wr(NWCTRL, wire_traffic_bits() | NWCTRL_TXSTART);
    }

    // Wait for ownership return.
    for _ in 0..MDIO_TIMEOUT {
        #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
        dma_invalidate(tx_bd, 8);
        if (unsafe { WIRE_TX_BD[idx].status } & TX_USED) != 0 {
            return Ok(idx);
        }
        core::hint::spin_loop();
    }
    Err(GemError::TxTimeout)
}

/// Poll RX ring for next used BD; copies frame, recycles BD. No GEM reinit.
/// Ownership is address-word bit0 (xemacps NEW/USED), not status bit31.
pub fn wire_rx_raw(out: &mut [u8]) -> Result<usize, GemError> {
    if !unsafe { WIRE_LIVE } {
        return Err(GemError::WireNotInit);
    }
    if (unsafe { rr(NWCTRL) } & NWCTRL_LOOP) != 0 {
        return Err(GemError::ReinitDuringWire);
    }

    for _ in 0..MDIO_TIMEOUT {
        if let Some(n) = wire_rx_poll(out) {
            return Ok(n);
        }
        core::hint::spin_loop();
    }
    Err(GemError::RxTimeout)
}

/// Non-blocking single-poll RX: returns Some(len) if a frame is waiting, else
/// None immediately. For poll-driven stacks (smoltcp) — never spins the ring.
pub fn wire_rx_nb(out: &mut [u8]) -> Option<usize> {
    if !unsafe { WIRE_LIVE } {
        return None;
    }
    wire_rx_poll(out)
}

// ── Interrupt-driven RX (feature ssh-bm / bm-net serve loops) ───────────────
// The serve loops still poll smoltcp, but between events they `wfi` instead of
// busy-spinning. GEM RX-complete wakes the CPU; the handler masks its own
// interrupt (one-shot) so a level-high SPI can't storm, and the poll loop
// drains the ring then re-arms.

/// Set by the GEM RX IRQ handler; consumed (cleared) by the poll loop.
pub static RX_READY: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Unmask the GEM RX-complete interrupt at the controller.
pub fn enable_rx_irq() {
    unsafe { wr(IER, INT_RX_COMPLETE) };
}

/// IRQ context: mask the RX interrupt (one-shot), clear the source, flag ready.
pub fn on_rx_irq() {
    unsafe {
        wr(IDR, INT_RX_COMPLETE); // mask until the poll loop re-arms
        let isr = rr(ISR);
        wr(ISR, isr); // write-1-to-clear the asserted status bits
        let rxsr = rr(RXSR);
        wr(RXSR, rxsr & RXSR_FRAME_RX); // clear frame-received status
    }
    RX_READY.store(true, core::sync::atomic::Ordering::Release);
}

/// Poll context: re-arm the RX interrupt after draining received frames.
pub fn rearm_rx_irq() {
    unsafe { wr(IER, INT_RX_COMPLETE) };
}

/// Take-and-clear the RX-ready flag.
pub fn take_rx_ready() -> bool {
    RX_READY.swap(false, core::sync::atomic::Ordering::AcqRel)
}

fn frame_ethertype(frame: &[u8]) -> Option<u16> {
    if frame.len() < 14 {
        return None;
    }
    Some(((frame[12] as u16) << 8) | frame[13] as u16)
}

fn push_hex_bytes(out: &mut String, data: &[u8]) {
    for (i, b) in data.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{b:02x}"));
    }
}

fn decode_txsr(txsr: u32) -> &'static str {
    if (txsr & TXSR_HRESP) != 0 {
        "HRESP_NOT_OK"
    } else if (txsr & TXSR_UNDERRUN) != 0 {
        "UNDERRUN"
    } else if (txsr & TXSR_RETRY_EXC) != 0 {
        "RETRY_EXCEEDED"
    } else if (txsr & TXSR_USED_READ) != 0 {
        "USED_BIT_READ"
    } else if (txsr & TXSR_COMPLETE) != 0 {
        "COMPLETE_OK"
    } else if (txsr & TXSR_TXGO) != 0 {
        "TX_GO_ACTIVE"
    } else {
        "NO_COMPLETE_FLAG"
    }
}

/// PN-WIRE-1 TX emission diagnostic (not PhaseNet).
///
/// Transmits one broadcast `0x88B5` PNW1 REQ and reports BD + GEM register
/// evidence before/after. Does **not** require a peer reply — goal is to prove
/// whether GEM believes the frame left the MAC.
pub fn wire_tx_debug_report() -> Result<String, GemError> {
    let st0 = wire_init()?;
    if st0.loop_bit {
        return Err(GemError::ReinitDuringWire);
    }

    let seq = unsafe {
        WIRE_SEQ = WIRE_SEQ.wrapping_add(1);
        WIRE_SEQ
    };
    let mut frame = [0u8; WIRE_FRAME_LEN];
    let frame_len = pnw1_wire::build_frame(&mut frame, pnw1_wire::KIND_REQ, seq, &BOARD_MAC);

    let idx = unsafe { WIRE_TX_HEAD };
    // Wait for free BD.
    for _ in 0..MDIO_TIMEOUT {
        if (unsafe { WIRE_TX_BD[idx].status } & TX_USED) != 0 {
            break;
        }
        core::hint::spin_loop();
    }
    if (unsafe { WIRE_TX_BD[idx].status } & TX_USED) == 0 {
        return Err(GemError::TxBusy);
    }

    let bd_ptr = unsafe { core::ptr::addr_of_mut!(WIRE_TX_BD[idx]) as u32 };
    let buf_ptr = unsafe { core::ptr::addr_of_mut!(WIRE_TX_BUF[idx]) as u32 };
    let wrap = if idx + 1 == WIRE_RING { TX_WRAP } else { 0 };

    let mut out = String::from("gem wire ping debug — TX emission evidence (not PhaseNet)\n");
    out.push_str("TX frame:\n");
    out.push_str(&format!(
        "  dst={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}\n",
        frame[0], frame[1], frame[2], frame[3], frame[4], frame[5]
    ));
    out.push_str(&format!(
        "  src={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}\n",
        frame[6], frame[7], frame[8], frame[9], frame[10], frame[11]
    ));
    out.push_str(&format!(
        "  ethertype=0x{:02x}{:02x} ({:#06x})\n",
        frame[12], frame[13], GP_ETHERTYPE
    ));
    out.push_str(&format!("  len={frame_len} (min Ethernet 60 before FCS)\n"));
    out.push_str("  payload[0..48]=");
    push_hex_bytes(&mut out, &frame[14..core::cmp::min(frame_len, 62)]);
    out.push('\n');
    out.push_str(&format!(
        "  checks: broadcast_dst={} src_nonzero={} ethertype_ok={} len_ok={}\n",
        frame[0..6].iter().all(|b| *b == 0xff),
        frame[6..12].iter().any(|b| *b != 0),
        frame[12] == 0x88 && frame[13] == 0xb5,
        frame_len >= 60
    ));
    out.push_str("  cache_clean=n/a (startup leaves D-cache off; DMA from .bss)\n");

    // Clear sticky TX status so AFTER bits are from this frame.
    unsafe {
        wr(TXSR, 0xFFFF_FFFF);
    }

    let bd_status_before = unsafe { WIRE_TX_BD[idx].status };
    let nwctrl_b = unsafe { rr(NWCTRL) };
    let nwcfg_b = unsafe { rr(NWCFG) };
    let txq_b = unsafe { rr(TXQBASE) };
    let txsr_b = unsafe { rr(TXSR) };
    let isr_b = unsafe { rr(ISR) };

    out.push_str("TX BD before:\n");
    out.push_str(&format!(
        "  bd_index={idx} bd_ptr=0x{bd_ptr:08X} buf_ptr=0x{buf_ptr:08X}\n"
    ));
    out.push_str(&format!(
        "  addr_word=0x{:08X} status_word=0x{:08X}\n",
        unsafe { WIRE_TX_BD[idx].addr },
        bd_status_before
    ));
    out.push_str(&format!(
        "  USED={} LAST={} WRAP={} desc_len={}\n",
        (bd_status_before & TX_USED) != 0,
        (bd_status_before & TX_LAST) != 0,
        (bd_status_before & TX_WRAP) != 0,
        bd_status_before & 0x3FFF
    ));

    out.push_str("TX registers before:\n");
    out.push_str(&format!(
        "  NWCTRL=0x{nwctrl_b:08X} TXEN={} RXEN={} MDEN={} LOOP={} TXSTART={}\n",
        (nwctrl_b & NWCTRL_TXEN) != 0,
        (nwctrl_b & NWCTRL_RXEN) != 0,
        (nwctrl_b & NWCTRL_MDEN) != 0,
        (nwctrl_b & NWCTRL_LOOP) != 0,
        (nwctrl_b & NWCTRL_TXSTART) != 0
    ));
    out.push_str(&format!("  NWCFG=0x{nwcfg_b:08X} TXQBASE=0x{txq_b:08X}\n"));
    dump_nwcfg_xilinx_bits(&mut out, nwcfg_b);
    let dmacrs = unsafe { rr(DMACR) };
    let expect = wire_dmacr();
    let df = gem_dmacr::decode(dmacrs);
    out.push_str(&format!(
        "  DMACR=0x{dmacrs:08X} expect=0x{expect:08X} rxbuf_bytes={} rxsize={} txsize={} incr16={}\n",
        df.rxbuf_bytes, df.rxsize, df.txsize, df.incr16
    ));
    // #region agent log
    dbg_line(
        &mut out,
        "J",
        "dmacr",
        &format!(
            "raw=0x{dmacrs:08X} expect=0x{expect:08X} rxbuf_bytes={} match={}",
            df.rxbuf_bytes,
            dmacrs == expect
        ),
    );
    // #endregion
    out.push_str(&format!(
        "  TXSR=0x{txsr_b:08X} ({}) ISR=0x{isr_b:08X}\n",
        decode_txsr(txsr_b)
    ));

    // Hand buffer to GEM.
    unsafe {
        WIRE_TX_BUF[idx] = [0; WIRE_BUF_LEN];
        WIRE_TX_BUF[idx][..frame_len].copy_from_slice(&frame[..frame_len]);
        WIRE_TX_BD[idx].addr = buf_ptr;
        // Clear USED — ownership to hardware.
        WIRE_TX_BD[idx].status = (frame_len as u32) | TX_LAST | wrap;
        WIRE_TX_HEAD = (idx + 1) % WIRE_RING;
        if WIRE_TX_HEAD == 0 {
            WIRE_TX_WRAPS = WIRE_TX_WRAPS.wrapping_add(1);
        }
        wr(NWCTRL, wire_traffic_bits() | NWCTRL_TXSTART);
    }

    let submit_status = unsafe { WIRE_TX_BD[idx].status };
    out.push_str("TX BD submitted (HW owns):\n");
    out.push_str(&format!(
        "  status_word=0x{submit_status:08X} USED={} LAST={} WRAP={} desc_len={}\n",
        (submit_status & TX_USED) != 0,
        (submit_status & TX_LAST) != 0,
        (submit_status & TX_WRAP) != 0,
        submit_status & 0x3FFF
    ));

    let mut ownership_returned = false;
    let mut timed_out = true;
    for _ in 0..MDIO_TIMEOUT {
        if (unsafe { WIRE_TX_BD[idx].status } & TX_USED) != 0 {
            ownership_returned = true;
            timed_out = false;
            break;
        }
        core::hint::spin_loop();
    }

    let bd_status_after = unsafe { WIRE_TX_BD[idx].status };
    let nwctrl_a = unsafe { rr(NWCTRL) };
    let txsr_a = unsafe { rr(TXSR) };
    let isr_a = unsafe { rr(ISR) };
    let complete = (txsr_a & TXSR_COMPLETE) != 0;
    let err = decode_txsr(txsr_a);

    out.push_str("TX BD after:\n");
    out.push_str(&format!(
        "  status_word=0x{bd_status_after:08X} USED={} LAST={} WRAP={} desc_len={}\n",
        (bd_status_after & TX_USED) != 0,
        (bd_status_after & TX_LAST) != 0,
        (bd_status_after & TX_WRAP) != 0,
        bd_status_after & 0x3FFF
    ));
    out.push_str(&format!(
        "  ownership_returned={} timeout={}\n",
        ownership_returned, timed_out
    ));

    out.push_str("TX registers after:\n");
    out.push_str(&format!(
        "  NWCTRL=0x{nwctrl_a:08X} TXEN={} LOOP={}\n",
        (nwctrl_a & NWCTRL_TXEN) != 0,
        (nwctrl_a & NWCTRL_LOOP) != 0
    ));
    out.push_str(&format!(
        "  TXSR=0x{txsr_a:08X} complete={} error={err} TXGO={} USED_READ={} UNDERRUN={} HRESP={}\n",
        complete,
        (txsr_a & TXSR_TXGO) != 0,
        (txsr_a & TXSR_USED_READ) != 0,
        (txsr_a & TXSR_UNDERRUN) != 0,
        (txsr_a & TXSR_HRESP) != 0
    ));
    out.push_str(&format!("  ISR=0x{isr_a:08X}\n"));

    out.push_str("Verdict:\n");
    if timed_out {
        out.push_str(
            "  DMA/descriptor: BD ownership did not return — descriptor setup or DMA stall\n",
        );
    } else if (txsr_a & (TXSR_HRESP | TXSR_UNDERRUN | TXSR_RETRY_EXC | TXSR_USED_READ)) != 0 {
        out.push_str(
            "  MAC/DMA/clock: TX error status set with ownership returned — check config/clocks\n",
        );
    } else if complete || ownership_returned {
        out.push_str("  GEM believes TX occurred (ownership returned");
        if complete {
            out.push_str(" + TXSR.COMPLETE");
        }
        out.push_str("). If PC/sniff still sees no frame → PHY/RGMII/path/capture-iface problem\n");
    } else {
        out.push_str("  inconclusive — inspect raw TXSR/BD words above\n");
    }
    out.push_str("RX result: not required for TX-leaves-GEM0 gate (use unfiltered PC sniff)\n");
    Ok(out)
}

/// TX PNW1 REQ then RX matching REP without GEM reinit (PN-WIRE-1 gate).
pub fn wire_roundtrip() -> Result<WireRoundtripReport, GemError> {
    let st0 = wire_init()?;
    if st0.loop_bit {
        return Err(GemError::ReinitDuringWire);
    }
    let init_before = st0.init_count;

    let seq = unsafe {
        WIRE_SEQ = WIRE_SEQ.wrapping_add(1);
        WIRE_SEQ
    };

    let mut tx = [0u8; WIRE_FRAME_LEN];
    let tx_len = pnw1_wire::build_frame(&mut tx, pnw1_wire::KIND_REQ, seq, &BOARD_MAC);
    let tx_bd = wire_tx_raw(&tx[..tx_len])?;

    // Drain RX until matching REP or timeout. Do not call wire_init / loopback.
    let mut rx = [0u8; WIRE_BUF_LEN];
    let mut rx_len = 0usize;
    let mut rx_bd = 0usize;
    let expect = pnw1_wire::expected_payload(pnw1_wire::KIND_REP, seq);
    let attempts = 64u32;
    let mut got = false;
    for _ in 0..attempts {
        if unsafe { WIRE_INIT_COUNT } != init_before {
            return Err(GemError::ReinitDuringWire);
        }
        if (unsafe { rr(NWCTRL) } & NWCTRL_LOOP) != 0 {
            return Err(GemError::ReinitDuringWire);
        }
        match wire_rx_raw(&mut rx) {
            Ok(n) => {
                rx_len = n;
                rx_bd = (unsafe { WIRE_RX_HEAD } + WIRE_RING - 1) % WIRE_RING;
                if frame_ethertype(&rx[..n]) != Some(GP_ETHERTYPE) {
                    continue;
                }
                if n < 14 + expect.len() {
                    continue;
                }
                if rx[14..14 + expect.len()] == expect {
                    got = true;
                    break;
                }
            }
            Err(GemError::RxTimeout) => break,
            Err(e) => return Err(e),
        }
    }
    if !got {
        return Err(GemError::RxTimeout);
    }

    let st1 = wire_status();
    if st1.init_count != init_before {
        return Err(GemError::ReinitDuringWire);
    }
    Ok(WireRoundtripReport {
        tx_len,
        rx_len,
        seq,
        init_count_before: init_before,
        init_count_after: st1.init_count,
        tx_bd,
        rx_bd,
        tx_wraps: st1.tx_wraps,
        rx_wraps: st1.rx_wraps,
        loop_bit: st1.loop_bit,
    })
}

/// Exercise TX ring wrap (RING+1 submits) without GEM reinit. Requires live link;
/// frames are broadcast PNW1 REQ (peer may ignore).
pub fn wire_tx_wrap_test() -> Result<WireStatus, GemError> {
    let st0 = wire_init()?;
    let init_before = st0.init_count;
    for i in 0..(WIRE_RING + 1) {
        if unsafe { WIRE_INIT_COUNT } != init_before {
            return Err(GemError::ReinitDuringWire);
        }
        let mut tx = [0u8; WIRE_FRAME_LEN];
        let seq = (i as u8).wrapping_add(0x40);
        let n = pnw1_wire::build_frame(&mut tx, pnw1_wire::KIND_REQ, seq, &BOARD_MAC);
        let _ = wire_tx_raw(&tx[..n])?;
    }
    let st1 = wire_status();
    if st1.init_count != init_before || st1.tx_wraps == 0 {
        // tx_wraps must increment once we passed the last BD.
        if st1.init_count != init_before {
            return Err(GemError::ReinitDuringWire);
        }
        if st1.tx_wraps == 0 {
            return Err(GemError::TxBusy);
        }
    }
    Ok(st1)
}

/// Cortex-A9 L1 line. GEM DMA reads DDR; CPU writes hit D-cache after CPU1 MMU-on.
const A9_DCACHE_LINE: u32 = 32;

#[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
fn dma_clean(addr: u32, len: u32) {
    if len == 0 {
        return;
    }
    let start = addr & !(A9_DCACHE_LINE - 1);
    let end = addr.wrapping_add(len).wrapping_add(A9_DCACHE_LINE - 1) & !(A9_DCACHE_LINE - 1);
    let mut a = start;
    while a < end {
        unsafe {
            core::arch::asm!(
                "mcr p15, 0, {}, c7, c10, 1",
                in(reg) a,
                options(nostack, preserves_flags)
            );
        }
        a = a.wrapping_add(A9_DCACHE_LINE);
    }
    unsafe {
        core::arch::asm!("dsb", options(nostack, preserves_flags));
    }
}

#[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
fn dma_invalidate(addr: u32, len: u32) {
    if len == 0 {
        return;
    }
    let start = addr & !(A9_DCACHE_LINE - 1);
    let end = addr.wrapping_add(len).wrapping_add(A9_DCACHE_LINE - 1) & !(A9_DCACHE_LINE - 1);
    let mut a = start;
    while a < end {
        unsafe {
            core::arch::asm!(
                "mcr p15, 0, {}, c7, c6, 1",
                in(reg) a,
                options(nostack, preserves_flags)
            );
        }
        a = a.wrapping_add(A9_DCACHE_LINE);
    }
    unsafe {
        core::arch::asm!("dsb", options(nostack, preserves_flags));
    }
}

#[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
fn wire_cache_commit_rings() {
    for i in 0..WIRE_RING {
        let (tx_buf, rx_buf, tx_bd, rx_bd) = unsafe {
            (
                core::ptr::addr_of_mut!(WIRE_TX_BUF[i]) as u32,
                core::ptr::addr_of_mut!(WIRE_RX_BUF[i]) as u32,
                core::ptr::addr_of_mut!(WIRE_TX_BD[i]) as u32,
                core::ptr::addr_of_mut!(WIRE_RX_BD[i]) as u32,
            )
        };
        dma_clean(tx_buf, WIRE_BUF_LEN as u32);
        dma_clean(rx_buf, WIRE_BUF_LEN as u32);
        dma_clean(tx_bd, 8);
        dma_clean(rx_bd, 8);
        dma_invalidate(rx_buf, WIRE_BUF_LEN as u32);
        dma_invalidate(rx_bd, 8);
    }
}

/// Step 5 — MAC local loopback (not wire Ethernet). Invalidates wire path.
pub fn mac_loopback_test() -> Result<usize, GemError> {
    invalidate_wire_path();
    enable_mdio_clock();

    let tx_buf = unsafe { core::ptr::addr_of_mut!(TX_BUF) as u32 };
    let rx_buf = unsafe { core::ptr::addr_of_mut!(RX_BUF) as u32 };
    let tx_bd = unsafe { core::ptr::addr_of_mut!(TX_BD) as u32 };
    let rx_bd = unsafe { core::ptr::addr_of_mut!(RX_BD) as u32 };

    // Build a minimal Ethernet frame: dst/src + ethertype 0x88B5 + payload.
    let frame_len = 64u32;
    unsafe {
        TX_BUF = [0; 160];
        RX_BUF = [0; 160];
        // dst FF:FF:FF:FF:FF:FF
        for b in TX_BUF.iter_mut().take(6) {
            *b = 0xFF;
        }
        // src 02:00:00:47:01:50 (matches V5 SA programming)
        TX_BUF[6] = 0x02;
        TX_BUF[7] = 0x00;
        TX_BUF[8] = 0x00;
        TX_BUF[9] = 0x47;
        TX_BUF[10] = 0x01;
        TX_BUF[11] = 0x50;
        TX_BUF[12] = (GP_ETHERTYPE >> 8) as u8;
        TX_BUF[13] = (GP_ETHERTYPE & 0xFF) as u8;
        for (i, b) in TX_BUF[14..frame_len as usize].iter_mut().enumerate() {
            *b = (0xA0 + i) as u8;
        }
    }

    unsafe {
        // Disable traffic, clear status, program DMA.
        wr(NWCTRL, NWCTRL_MDEN);
        wr(IDR, 0xFFFF_FFFF);
        wr(TXSR, 0xFFFF_FFFF);
        wr(RXSR, 0xFFFF_FFFF);
        // MAC loopback buffers are 160 bytes — DMACR RXBUF must match that object.
        wr(DMACR, gem_dmacr::build_for_buf_len(160));
        wr(SA1B, 0x4700_0002);
        wr(SA1T, 0x0150);

        TX_BD.addr = tx_buf;
        TX_BD.status = frame_len | (1 << 15) /* last */ | (1 << 30) /* wrap */;
        RX_BD.addr = gem_rxbd::addr_word_for_hw(rx_buf, true);
        RX_BD.status = 0;
    }

    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        dma_clean(tx_buf, 160);
        dma_clean(rx_buf, 160);
        dma_clean(tx_bd, 8);
        dma_clean(rx_bd, 8);
        dma_invalidate(rx_buf, 160);
        dma_invalidate(rx_bd, 8);
    }

    unsafe {
        wr(TXQBASE, tx_bd);
        wr(RXQBASE, rx_bd);

        // LOOP + RX + TX + MDEN
        wr(
            NWCTRL,
            NWCTRL_LOOP | NWCTRL_RXEN | NWCTRL_TXEN | NWCTRL_MDEN,
        );
        // Start transmit
        wr(
            NWCTRL,
            NWCTRL_LOOP | NWCTRL_RXEN | NWCTRL_TXEN | NWCTRL_MDEN | (1 << 9),
        );
    }

    // Wait for RX ownership (addr bit0) / frame received.
    for _ in 0..MDIO_TIMEOUT {
        #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
        {
            dma_invalidate(rx_bd, 8);
            dma_invalidate(tx_bd, 8);
        }
        let addr_w = unsafe { RX_BD.addr };
        let st = unsafe { RX_BD.status };
        if gem_rxbd::owned_by_sw(addr_w) && gem_rxbd::parse_status(st, 160).is_ok() {
            break;
        }
        // Also check RXSR frame received bit (bit 1).
        if (unsafe { rr(RXSR) } & 0x02) != 0 && gem_rxbd::owned_by_sw(unsafe { RX_BD.addr }) {
            break;
        }
        core::hint::spin_loop();
    }

    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        dma_invalidate(rx_bd, 8);
        dma_invalidate(tx_bd, 8);
        dma_invalidate(rx_buf, 160);
    }

    let addr_w = unsafe { RX_BD.addr };
    let st = unsafe { RX_BD.status };
    let rx_len = match gem_rxbd::take_frame(addr_w, st, 160) {
        Ok(p) => p.len,
        Err(_) => {
            unsafe { wr(NWCTRL, NWCTRL_MDEN) };
            return Err(GemError::LoopbackTimeout);
        }
    };
    record_rx_take(rx_len);
    if rx_len < 14 {
        unsafe { wr(NWCTRL, NWCTRL_MDEN) };
        return Err(GemError::LoopbackTimeout);
    }

    // Compare ethertype + first payload bytes (ignore FCS if stripped).
    let ok = unsafe {
        RX_BUF[12] == TX_BUF[12]
            && RX_BUF[13] == TX_BUF[13]
            && RX_BUF[14] == TX_BUF[14]
            && RX_BUF[15] == TX_BUF[15]
    };

    unsafe { wr(NWCTRL, NWCTRL_MDEN) };

    if !ok {
        return Err(GemError::LoopbackMismatch);
    }
    Ok(rx_len)
}

/// Run ladder steps 1–5; step 6 points at `net wire` (peer required — not MAC loopback).
pub fn run_ladder_report() -> String {
    let mut out = String::from("GEM0 ladder @ 0xE000B000\n");

    let (c, cfg, s) = probe_registers();
    out.push_str(&format!(
        "1 register presence: NWCTRL=0x{c:08X} NWCFG=0x{cfg:08X} NWSR=0x{s:08X} OK\n"
    ));

    match scan_phy() {
        Ok(phy) => {
            out.push_str(&format!(
                "2 MDIO access: OK\n3 PHY ID: addr={} id1=0x{:04X} id2=0x{:04X} OK\n",
                phy.addr, phy.id1, phy.id2
            ));
            match link_status(phy.addr) {
                Ok(link) => {
                    if link.up {
                        out.push_str(&format!(
                            "4 link status: UP bmsr=0x{:04X} speed={} duplex={} OK\n",
                            link.bmsr,
                            link.speed_str(),
                            link.duplex_str()
                        ));
                    } else {
                        out.push_str(&format!(
                            "4 link status: DOWN bmsr=0x{:04X} speed=n/a duplex=n/a PARTIAL (no partner/cable)\n",
                            link.bmsr
                        ));
                    }
                }
                Err(e) => out.push_str(&format!("4 link status: FAIL {}\n", e.as_str())),
            }
        }
        Err(e) => {
            out.push_str(&format!(
                "2 MDIO access: FAIL {}\n3 PHY ID: FAIL\n4 link status: SKIP\n",
                e.as_str()
            ));
        }
    }

    match mac_loopback_test() {
        Ok(n) => out.push_str(&format!(
            "5 MAC loopback: PASS rx_len={n} ethertype=0x{GP_ETHERTYPE:04X} (local MAC only — not wire Ethernet)\n"
        )),
        Err(e) => out.push_str(&format!("5 MAC loopback: FAIL {}\n", e.as_str())),
    }

    let ws = wire_status();
    if ws.live {
        out.push_str(&format!(
            "6 external TX/RX: wire LIVE ring={} init_count={} tx_head={} rx_head={} (use net wire ping; not proven by loopback)\n",
            ws.ring, ws.init_count, ws.tx_head, ws.rx_head
        ));
    } else {
        out.push_str(
            "6 external TX/RX: PENDING peer — run `net wire init` + host peer + `net wire ping` (MAC loopback is not evidence)\n",
        );
    }
    out
}
