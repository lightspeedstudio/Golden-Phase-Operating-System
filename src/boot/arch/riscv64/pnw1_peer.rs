//! QEMU virtio PN-WIRE-1A peer: reply to board PNW1 REQ with controlled REP.
//!
//! This is a Golden Phase endpoint over virtio-net for development proof
//! (PZ7020 ↔ QEMU). It is **not** a two-board PhaseNet seal (that is PN-WIRE-1B).

use crate::boot::arch::riscv64::virtio_net::{self, PHASELINK_ETHERTYPE};
use crate::kernel::network::NetworkInterfaceStatus;
use crate::protocol::pnw1_wire::{self, FRAME_LEN, KIND_REP};

/// Poll once: if a board PNW1 REQ is waiting, TX controlled REP.
///
/// Returns `Ok(seq)` on reply, `Err` if no frame / not PNW1 REQ / TX fail.
pub fn try_reply_once(status: &mut NetworkInterfaceStatus) -> Result<u8, &'static str> {
    virtio_net::receive_frame(status)?;
    let frame = virtio_net::last_rx_ethernet();
    let et = if frame.len() >= 14 {
        u16::from_be_bytes([frame[12], frame[13]])
    } else {
        return Err("pnw1 peer: short frame");
    };
    if et != PHASELINK_ETHERTYPE && et != pnw1_wire::ETHERTYPE {
        return Err("pnw1 peer: wrong ethertype");
    }
    let req = pnw1_wire::parse_board_req(frame).ok_or("pnw1 peer: not board PNW1 REQ")?;
    let peer_mac = status.mac_address.ok_or("pnw1 peer: virtio MAC missing")?;
    let mut rep = [0u8; FRAME_LEN];
    let n = pnw1_wire::build_rep(&mut rep, req.seq, &peer_mac);
    debug_assert_eq!(rep[18], KIND_REP);
    virtio_net::transmit_ethernet(status, &rep[..n])?;
    Ok(req.seq)
}

/// Busy-poll up to `spins` iterations for one successful REQ→REP.
pub fn serve_one(status: &mut NetworkInterfaceStatus, spins: u32) -> Result<u8, &'static str> {
    let mut last = "pnw1 peer: timeout waiting for REQ";
    for _ in 0..spins {
        match try_reply_once(status) {
            Ok(seq) => return Ok(seq),
            Err("virtio-net: no packet received") => {
                core::hint::spin_loop();
            }
            Err(e) => {
                // Non-PNW1 traffic: keep polling.
                if e.starts_with("pnw1 peer: not board") || e.starts_with("pnw1 peer: wrong") {
                    last = e;
                    core::hint::spin_loop();
                    continue;
                }
                return Err(e);
            }
        }
    }
    Err(last)
}

/// Serve up to `count` REQ→REP exchanges (each waits up to `spins_per`).
pub fn serve_many(
    status: &mut NetworkInterfaceStatus,
    count: u32,
    spins_per: u32,
) -> Result<u32, &'static str> {
    let mut done = 0u32;
    for _ in 0..count {
        match serve_one(status, spins_per) {
            Ok(_) => done = done.saturating_add(1),
            Err(e) if done == 0 => return Err(e),
            Err(_) => break,
        }
    }
    Ok(done)
}
