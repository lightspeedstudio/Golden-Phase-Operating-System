//! Cadence GEM / Xilinx `emacps` register + RX descriptor helpers (host-testable).
//!
//! Bit definitions match seller-extracted `emacps_v3_9` `xemacps_hw.h`.
//! Pure logic only — no MMIO. Used by PZ7020 `gem.rs` and unit tests.

/// NWCFG — Network Configuration (xemacps_hw.h).
pub mod nwcfg {
    pub const SPEED100: u32 = 0x0000_0001;
    pub const FD: u32 = 0x0000_0002;
    pub const COPYALLEN: u32 = 0x0000_0010;
    pub const GBIT: u32 = 0x0000_0400;
    pub const PAUSEEN: u32 = 0x0000_2000;
    pub const FCSREM: u32 = 0x0002_0000;
    /// MDC clock divider field shift (div48 → value 3).
    pub const MDC_SHIFT: u32 = 18;

    pub fn mdc_div48() -> u32 {
        3u32 << MDC_SHIFT
    }

    /// Wire-path NWCFG: MDC÷48 + FCS remove + promiscuous + speed/duplex.
    pub fn for_link(speed_mbps: u16, full_duplex: bool, link_up: bool) -> u32 {
        let mut cfg = mdc_div48() | FCSREM | COPYALLEN;
        if full_duplex || !link_up {
            cfg |= FD;
        }
        match speed_mbps {
            1000 => cfg |= GBIT,
            100 => cfg |= SPEED100,
            _ => {}
        }
        cfg
    }

    /// MDIO bring-up NWCFG (no promiscuous requirement).
    pub fn for_mdio() -> u32 {
        mdc_div48() | FCSREM | FD | SPEED100
    }
}

/// DMACR — DMA Configuration (xemacps_hw.h).
pub mod dmacr {
    pub const RXBUF_SHIFT: u32 = 16;
    pub const RXBUF_MASK: u32 = 0x00FF_0000;
    pub const TXSIZE: u32 = 0x0000_0400;
    pub const RXSIZE: u32 = 0x0000_0300;
    pub const INCR16: u32 = 0x0000_0010;
    pub const INCR8: u32 = 0x0000_0008;
    pub const INCR4: u32 = 0x0000_0004;
    pub const BLENGTH_MASK: u32 = 0x0000_001F;
    /// Bytes per RXBUF unit (XEMACPS_RX_BUF_UNIT).
    pub const RX_BUF_UNIT: u32 = 64;

    /// Encode RX buffer size in bytes into the DMACR RXBUF field (ceil to units).
    pub fn rxbuf_field_from_bytes(buf_len: usize) -> u32 {
        let units = ((buf_len as u32) + RX_BUF_UNIT - 1) / RX_BUF_UNIT;
        (units << RXBUF_SHIFT) & RXBUF_MASK
    }

    /// Build DMACR for Zynq-7000 GEM (version ≤2 style): RXBUF + RXSIZE + TXSIZE.
    ///
    /// Does **not** set INCR16 (XEmacPs_Reset only adds INCR16 when Version > 2).
    /// `buf_len` must match the actual static RX buffer object size.
    pub fn build_for_buf_len(buf_len: usize) -> u32 {
        rxbuf_field_from_bytes(buf_len) | RXSIZE | TXSIZE
    }

    pub fn decode(raw: u32) -> DmaCrFields {
        DmaCrFields {
            rxbuf_units: (raw & RXBUF_MASK) >> RXBUF_SHIFT,
            rxbuf_bytes: ((raw & RXBUF_MASK) >> RXBUF_SHIFT) * RX_BUF_UNIT,
            rxsize: (raw & RXSIZE) != 0,
            txsize: (raw & TXSIZE) != 0,
            incr16: (raw & INCR16) != 0,
            burst: raw & BLENGTH_MASK,
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DmaCrFields {
        pub rxbuf_units: u32,
        pub rxbuf_bytes: u32,
        pub rxsize: bool,
        pub txsize: bool,
        pub incr16: bool,
        pub burst: u32,
    }
}

/// RX buffer descriptor word0/word1 (xemacps_hw.h / UG585).
pub mod rxbd {
    pub const ADDR_OWN: u32 = 0x0000_0001; // NEW/used — SW owns when set
    pub const ADDR_WRAP: u32 = 0x0000_0002;
    pub const ADDR_MASK: u32 = 0xFFFF_FFFC;
    pub const STATUS_EOF: u32 = 0x0000_8000;
    pub const STATUS_SOF: u32 = 0x0000_4000;
    pub const STATUS_LEN: u32 = 0x0000_1FFF;
    pub const STATUS_BCAST: u32 = 0x8000_0000;
    pub const STATUS_EXH: u32 = 0x0800_0000;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RxBdError {
        NotOwned,
        ZeroLength,
        Oversized,
        MissingEof,
        BufferExhausted,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RxParsed {
        pub len: usize,
        pub sof: bool,
        pub eof: bool,
        pub broadcast: bool,
    }

    #[inline]
    pub fn owned_by_sw(addr_word: u32) -> bool {
        (addr_word & ADDR_OWN) != 0
    }

    #[inline]
    pub fn wrap(addr_word: u32) -> bool {
        (addr_word & ADDR_WRAP) != 0
    }

    #[inline]
    pub fn buf_addr(addr_word: u32) -> u32 {
        addr_word & ADDR_MASK
    }

    /// Address word while hardware owns the BD (OWN clear).
    #[inline]
    pub fn addr_word_for_hw(buf: u32, wrap_bit: bool) -> u32 {
        (buf & ADDR_MASK) | if wrap_bit { ADDR_WRAP } else { 0 }
    }

    /// Return BD to hardware: clear OWN, keep WRAP, point at buffer.
    #[inline]
    pub fn return_to_hw(buf: u32, wrap_bit: bool) -> u32 {
        addr_word_for_hw(buf, wrap_bit)
    }

    pub fn parse_status(status: u32, max_len: usize) -> Result<RxParsed, RxBdError> {
        if (status & STATUS_EXH) != 0 {
            return Err(RxBdError::BufferExhausted);
        }
        let len = (status & STATUS_LEN) as usize;
        if len == 0 {
            return Err(RxBdError::ZeroLength);
        }
        if len > max_len {
            return Err(RxBdError::Oversized);
        }
        let sof = (status & STATUS_SOF) != 0;
        let eof = (status & STATUS_EOF) != 0;
        if !eof {
            return Err(RxBdError::MissingEof);
        }
        Ok(RxParsed {
            len,
            sof,
            eof,
            broadcast: (status & STATUS_BCAST) != 0,
        })
    }

    /// Full take: require SW ownership on addr word, then parse status.
    pub fn take_frame(addr_word: u32, status: u32, max_len: usize) -> Result<RxParsed, RxBdError> {
        if !owned_by_sw(addr_word) {
            return Err(RxBdError::NotOwned);
        }
        parse_status(status, max_len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nwcfg_bits_match_xemacps() {
        assert_eq!(nwcfg::COPYALLEN, 0x0000_0010);
        assert_eq!(nwcfg::PAUSEEN, 0x0000_2000);
        assert_eq!(nwcfg::FCSREM, 0x0002_0000);
        assert_eq!(nwcfg::SPEED100, 0x1);
        assert_eq!(nwcfg::FD, 0x2);
    }

    #[test]
    fn nwcfg_for_link_sets_copyall_and_fcsrem_not_pause() {
        let cfg = nwcfg::for_link(100, true, true);
        assert_ne!(cfg & nwcfg::COPYALLEN, 0);
        assert_ne!(cfg & nwcfg::FCSREM, 0);
        assert_eq!(cfg & nwcfg::PAUSEEN, 0);
        assert_ne!(cfg & nwcfg::SPEED100, 0);
        assert_ne!(cfg & nwcfg::FD, 0);
        // Old bug pattern 0x000C2013 had PAUSEEN and lacked FCSREM.
        assert_ne!(cfg, 0x000C_2013);
        assert_ne!(cfg & nwcfg::FCSREM, 0);
    }

    #[test]
    fn dmacr_encode_256_matches_buffer_not_1536_claim() {
        let v = dmacr::build_for_buf_len(256);
        assert_eq!(v, 0x0004_0700);
        let d = dmacr::decode(v);
        assert_eq!(d.rxbuf_units, 4);
        assert_eq!(d.rxbuf_bytes, 256);
        assert!(d.rxsize);
        assert!(d.txsize);
        assert!(!d.incr16);
        // Xilinx 1536-byte policy would be 0x00180700 — we deliberately do not.
        assert_ne!(v, 0x0018_0700);
    }

    #[test]
    fn dmacr_old_literal_differs() {
        let old = 0x0004_0410u32;
        let newv = dmacr::build_for_buf_len(256);
        assert_ne!(old, newv);
        let od = dmacr::decode(old);
        assert!(!od.rxsize);
        assert!(od.incr16);
    }

    #[test]
    fn rx_ownership_is_addr_bit0_not_status_bit31() {
        let buf = 0x0010_0000u32;
        let hw = rxbd::addr_word_for_hw(buf, false);
        assert!(!rxbd::owned_by_sw(hw));
        assert_eq!(rxbd::buf_addr(hw), buf);
        let sw = hw | rxbd::ADDR_OWN;
        assert!(rxbd::owned_by_sw(sw));
        // Status bit31 alone must NOT be treated as ownership.
        let status_only = 0x8000_0040u32;
        assert!(rxbd::take_frame(hw, status_only, 256).is_err());
    }

    #[test]
    fn rx_wrap_and_return_to_hw() {
        let buf = 0x0010_0004u32;
        let a = rxbd::addr_word_for_hw(buf, true);
        assert!(rxbd::wrap(a));
        assert!(!rxbd::owned_by_sw(a));
        let back = rxbd::return_to_hw(buf | 1, true); // stray low bits masked
        assert_eq!(back & rxbd::ADDR_OWN, 0);
        assert!(rxbd::wrap(back));
        assert_eq!(rxbd::buf_addr(back), buf & rxbd::ADDR_MASK);
    }

    #[test]
    fn rx_sof_eof_length_parse() {
        let st = rxbd::STATUS_SOF | rxbd::STATUS_EOF | 64;
        let p = rxbd::parse_status(st, 256).unwrap();
        assert_eq!(p.len, 64);
        assert!(p.sof && p.eof);
    }

    #[test]
    fn rx_zero_and_oversized_rejected() {
        assert_eq!(
            rxbd::parse_status(rxbd::STATUS_EOF, 256),
            Err(rxbd::RxBdError::ZeroLength)
        );
        assert_eq!(
            rxbd::parse_status(rxbd::STATUS_EOF | 300, 256),
            Err(rxbd::RxBdError::Oversized)
        );
        assert_eq!(
            rxbd::parse_status(rxbd::STATUS_SOF | 64, 256),
            Err(rxbd::RxBdError::MissingEof)
        );
    }

    #[test]
    fn rx_take_frame_requires_own_then_returns() {
        let addr = rxbd::addr_word_for_hw(0x2000, false) | rxbd::ADDR_OWN;
        let st = rxbd::STATUS_SOF | rxbd::STATUS_EOF | 60;
        let p = rxbd::take_frame(addr, st, 256).unwrap();
        assert_eq!(p.len, 60);
        let recycled = rxbd::return_to_hw(rxbd::buf_addr(addr), false);
        assert!(!rxbd::owned_by_sw(recycled));
    }

    #[test]
    fn rx_ring_wrap_flag_on_last() {
        const RING: usize = 4;
        for i in 0..RING {
            let wrap = i + 1 == RING;
            let w = rxbd::addr_word_for_hw(0x1000 + (i as u32) * 0x100, wrap);
            assert_eq!(rxbd::wrap(w), wrap);
        }
    }
}
