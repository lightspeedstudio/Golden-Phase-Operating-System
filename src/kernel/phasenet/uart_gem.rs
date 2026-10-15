//! Bounded UART→GEM PhaseNet frame (EtherType 0x88B5).
//!
//! UART bytes become a length-bounded KPHN RawStream frame. Sequence,
//! previous-hash, and SHA-256 frame hash are checked on RX. This is not
//! unrestricted MMIO: the caller must TX only through GEM wire_tx_raw.

use crate::kernel::hash::tagged_hash;
use crate::kernel::phasenet::link::{
    compute_frame_hash, decode_frame, encode_frame, FrameDecodeError, PayloadKind, PhaseLinkClock,
    PhaseLinkFrame, PHASE_LINK_MAGIC,
};
use crate::platform::{String, Vec};

pub const ETHERTYPE: u16 = 0x88B5;
pub const MAX_UART_PAYLOAD: usize = 64;
pub const BRIDGE_CHANNEL: u32 = 0x5547_454D; // "UGEM"

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeError {
    Empty,
    TooLong,
    BadEtherType,
    Short,
    Decode,
    Replay,
    Checksum,
    Timeout,
    PeerLoss,
}

impl BridgeError {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "bridge: empty uart payload",
            Self::TooLong => "bridge: uart payload exceeds bound",
            Self::BadEtherType => "bridge: ethertype is not 0x88B5",
            Self::Short => "bridge: ethernet frame too short",
            Self::Decode => "bridge: malformed PhaseNet frame refused",
            Self::Replay => "bridge: sequence replay refused",
            Self::Checksum => "bridge: frame hash mismatch",
            Self::Timeout => "bridge: peer timeout",
            Self::PeerLoss => "bridge: peer-loss",
        }
    }
}

#[derive(Debug, Clone)]
pub struct UartGemBridge {
    pub active: bool,
    pub seq_out: u64,
    pub seq_in: u64,
    pub last_out_hash: [u8; 32],
    pub last_in_hash: [u8; 32],
    pub src: [u8; 32],
    pub dst: [u8; 32],
    pub equation_root: [u8; 32],
    pub boot_session_root: [u8; 32],
}

impl Default for UartGemBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl UartGemBridge {
    pub fn new() -> Self {
        Self {
            active: false,
            seq_out: 0,
            seq_in: 0,
            last_out_hash: [0u8; 32],
            last_in_hash: [0u8; 32],
            src: tagged_hash("GPOS/UART-GEM/src/v1", &[b"board"]),
            dst: tagged_hash("GPOS/UART-GEM/dst/v1", &[b"peer"]),
            equation_root: [0u8; 32],
            boot_session_root: [0u8; 32],
        }
    }

    pub fn activate(&mut self, equation_root: [u8; 32], boot_session_root: [u8; 32]) {
        self.equation_root = equation_root;
        self.boot_session_root = boot_session_root;
        self.src = tagged_hash("GPOS/UART-GEM/src/v1", &[b"board"]);
        self.dst = tagged_hash("GPOS/UART-GEM/dst/v1", &[b"peer"]);
        self.active = true;
        self.seq_out = 0;
        self.seq_in = 0;
        self.last_out_hash = [0u8; 32];
        self.last_in_hash = [0u8; 32];
    }

    pub fn encode_uart(&mut self, text: &[u8]) -> Result<Vec<u8>, BridgeError> {
        if text.is_empty() {
            return Err(BridgeError::Empty);
        }
        if text.len() > MAX_UART_PAYLOAD {
            return Err(BridgeError::TooLong);
        }
        self.seq_out = self.seq_out.saturating_add(1);
        let mut frame = PhaseLinkFrame {
            version: 1,
            src_addr: self.src,
            dst_addr: self.dst,
            boot_session_root: self.boot_session_root,
            equation_root: self.equation_root,
            channel: BRIDGE_CHANNEL,
            sequence: self.seq_out,
            clock: PhaseLinkClock {
                proper_tick: self.seq_out,
                kairos_tick: 0,
                sector_id: 0,
                x: 0.0,
            },
            payload_kind: PayloadKind::RawStream,
            payload: text.to_vec(),
            proof_hash: [0u8; 32],
            previous_frame_hash: self.last_out_hash,
            frame_hash: [0u8; 32],
        };
        frame.frame_hash = compute_frame_hash(&frame);
        self.last_out_hash = frame.frame_hash;
        Ok(encode_frame(&frame))
    }

    pub fn wrap_ethernet(payload: &[u8], src_mac: &[u8; 6]) -> Result<Vec<u8>, BridgeError> {
        if 14 + payload.len() > 1536 {
            return Err(BridgeError::TooLong);
        }
        let mut eth = Vec::with_capacity(14 + payload.len());
        eth.extend_from_slice(&[0xff; 6]);
        eth.extend_from_slice(src_mac);
        eth.extend_from_slice(&ETHERTYPE.to_be_bytes());
        eth.extend_from_slice(payload);
        Ok(eth)
    }

    pub fn accept_ethernet(&mut self, eth: &[u8]) -> Result<Vec<u8>, BridgeError> {
        if eth.len() < 14 {
            return Err(BridgeError::Short);
        }
        let et = u16::from_be_bytes([eth[12], eth[13]]);
        if et != ETHERTYPE {
            return Err(BridgeError::BadEtherType);
        }
        let body = &eth[14..];
        let frame = match decode_frame(body) {
            Ok(f) => f,
            Err(FrameDecodeError::BadHash) => return Err(BridgeError::Checksum),
            Err(_) => return Err(BridgeError::Decode),
        };
        if frame.sequence <= self.seq_in {
            return Err(BridgeError::Replay);
        }
        if self.seq_in != 0 && frame.previous_frame_hash != self.last_in_hash {
            return Err(BridgeError::Decode);
        }
        self.seq_in = frame.sequence;
        self.last_in_hash = frame.frame_hash;
        Ok(frame.payload)
    }
}

pub fn magic_ok(payload: &[u8]) -> bool {
    payload.len() >= 4 && &payload[..4] == PHASE_LINK_MAGIC
}

pub fn status_label(active: bool) -> &'static str {
    if active {
        "uart-gem"
    } else {
        "none"
    }
}

pub fn render_rx(text: &str) -> String {
    format!("bridge rx: {text}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::pnw1_wire::BOARD_MAC;

    #[test]
    fn uart_round_trip_and_replay_refused() {
        let mut tx = UartGemBridge::new();
        tx.activate([1u8; 32], [2u8; 32]);
        let mut rx = UartGemBridge::new();
        rx.activate([1u8; 32], [2u8; 32]);
        let body = tx.encode_uart(b"hello-uart").unwrap();
        let eth = UartGemBridge::wrap_ethernet(&body, &BOARD_MAC).unwrap();
        assert_eq!(u16::from_be_bytes([eth[12], eth[13]]), 0x88B5);
        let got = rx.accept_ethernet(&eth).unwrap();
        assert_eq!(got, b"hello-uart");
        assert_eq!(rx.accept_ethernet(&eth), Err(BridgeError::Replay));
    }

    #[test]
    fn malformed_and_oversize_refused() {
        let mut b = UartGemBridge::new();
        b.activate([0u8; 32], [0u8; 32]);
        assert_eq!(b.encode_uart(b""), Err(BridgeError::Empty));
        assert_eq!(
            b.encode_uart(&[0u8; 65]),
            Err(BridgeError::TooLong)
        );
        let mut bad = vec![0u8; 20];
        bad[12] = 0x08;
        bad[13] = 0x00;
        assert_eq!(b.accept_ethernet(&bad), Err(BridgeError::BadEtherType));
        let mut junk = vec![0xffu8; 14 + 32];
        junk[12] = 0x88;
        junk[13] = 0xB5;
        assert_eq!(b.accept_ethernet(&junk), Err(BridgeError::Decode));
    }
}
