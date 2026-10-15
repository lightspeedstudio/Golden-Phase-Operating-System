//! smoltcp `phy::Device` over the GEM0 raw wire path.
//!
//! The GEM wire path is promiscuous (NWCFG COPYALLEN) and EtherType-agnostic, so
//! it carries ARP/IPv4 as well as the raw 0x88B5 frames. This is the Rust
//! equivalent of the vendor `xemacpsif` netif shim, but thin because `gem.rs`
//! already owns the DMA rings:
//!   RX -> `gem::wire_rx_nb` (non-blocking single poll)
//!   TX -> `gem::wire_tx_raw`
//! GemDevice is zero-sized because the GEM state lives in the `gem` module statics.

use crate::board::zynq7000::gem::{self, WIRE_BUF_LEN};
use crate::platform::Vec;
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::time::Instant;

pub struct GemDevice;

pub struct GemRxToken(Vec<u8>);
pub struct GemTxToken;

impl Device for GemDevice {
    type RxToken<'a>
        = GemRxToken
    where
        Self: 'a;
    type TxToken<'a>
        = GemTxToken
    where
        Self: 'a;

    fn receive(&mut self, _timestamp: Instant) -> Option<(GemRxToken, GemTxToken)> {
        // Heap, not stack: this runs nested under `net ssh serve` (sunset Runner
        // + Kernel already on the 64 KiB SVC stack). A 1536-byte stack frame
        // here sits above GEM DMA descriptors in .bss.
        let mut frame = alloc::vec![0u8; WIRE_BUF_LEN];
        let n = gem::wire_rx_nb(&mut frame)?;
        frame.truncate(n);
        Some((GemRxToken(frame), GemTxToken))
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<GemTxToken> {
        Some(GemTxToken)
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet;
        caps.max_transmission_unit = WIRE_BUF_LEN;
        caps
    }
}

impl RxToken for GemRxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(&self.0)
    }
}

impl TxToken for GemTxToken {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buf = Vec::new();
        buf.resize(len, 0u8);
        let result = f(&mut buf);
        let _ = gem::wire_tx_raw(&buf);
        result
    }
}
