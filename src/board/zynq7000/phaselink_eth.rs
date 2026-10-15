//! PhaseLink over GEM0 raw Ethernet (Zynq PS / U15 RTL8211F).
//!
//! Transport counterpart to `boot::arch::riscv64::phaselink_eth`, but driving the
//! proven GEM0 wire path (`gem::wire_tx_raw`/`wire_rx_raw`, EtherType `0x88B5`)
//! instead of RISC-V VirtIO. All PhaseLink protocol logic is reused unchanged
//! from `kernel::phaselink`; only the Ethernet transport differs.

use crate::board::zynq7000::gem;
use crate::kernel::phaselink::decoder::PhaseLinkDecoder;
use crate::kernel::phaselink::encoder::PhaseLinkEncoder;
use crate::kernel::phaselink::quarantine::QuarantineReason;
use crate::kernel::phaselink::{
    accept_or_quarantine, build_payload_frame, init_baremetal_node, peer_node_id,
    FrameValidationResult,
};
use crate::kernel::phasenet::uart_gem::{self, UartGemBridge};
use crate::kernel::Kernel;
use crate::platform::{String, Vec};
use crate::protocol::pnw1_wire::BOARD_MAC;

/// Golden Phase raw EtherType — identical to the RISC-V PhaseLink transport and
/// the GEM0 wire path, so frames are interoperable on the wire.
const PHASELINK_ETHERTYPE: u16 = 0x88B5;

/// RX scratch matches the GEM wire buffer (full MTU), so it stays in sync.
const RX_BUF_LEN: usize = gem::WIRE_BUF_LEN;

/// Last transmitted Ethernet frame, for `replay`.
static mut LAST_TX: Option<Vec<u8>> = None;
static mut UART_GEM: UartGemBridge = UartGemBridge {
    active: false,
    seq_out: 0,
    seq_in: 0,
    last_out_hash: [0u8; 32],
    last_in_hash: [0u8; 32],
    src: [0u8; 32],
    dst: [0u8; 32],
    equation_root: [0u8; 32],
    boot_session_root: [0u8; 32],
};

pub fn init_node(
    kernel: &mut Kernel,
    local_name: &str,
    peer_name: &str,
) -> Result<(), &'static str> {
    if local_name.is_empty() || peer_name.is_empty() {
        return Err("phaselink: usage init <local-node> <peer-node>");
    }
    // Bring up the GEM0 wire path (requires PHY link UP).
    gem::wire_init().map_err(|_| "phaselink: GEM0 wire init failed (PHY link down?)")?;
    init_baremetal_node(kernel, local_name, peer_name);
    unsafe {
        UART_GEM.activate(
            kernel.phaselink.equation_root,
            kernel.phaselink.boot_session_root,
        );
    }
    kernel.phaselink.uart_gem_active = true;
    Ok(())
}

pub fn send_text(kernel: &mut Kernel, peer_name: &str, text: &str) -> Result<(), &'static str> {
    if !gem::wire_status().live {
        return Err("phaselink: network not initialized (run phaselink init)");
    }
    if kernel.phaselink.local_node == [0u8; 32] {
        return Err("phaselink: not initialized");
    }
    if peer_name.is_empty() || text.is_empty() {
        return Err("phaselink: usage send <peer-node> <text>");
    }

    let destination = peer_node_id(peer_name, &kernel.phaselink.equation_root);
    let frame = build_payload_frame(kernel, destination, "HELLO", text.as_bytes())
        .map_err(|_| "phaselink: frame build failed")?;
    let encoded = PhaseLinkEncoder::encode_frame(&frame).map_err(|_| "phaselink: encode failed")?;
    let eth = wrap_ethernet(&encoded)?;
    gem::wire_tx_raw(&eth).map_err(|_| "phaselink: GEM0 TX failed (frame too large / link)")?;
    unsafe {
        LAST_TX = Some(eth);
    }
    kernel.phaselink.sender.previous_frame_hash = frame.frame_hash;
    Ok(())
}

pub fn replay_last(_kernel: &mut Kernel) -> Result<(), &'static str> {
    if !gem::wire_status().live {
        return Err("phaselink: network not initialized");
    }
    let eth = unsafe { LAST_TX.as_ref() };
    match eth {
        Some(frame) => {
            gem::wire_tx_raw(frame).map_err(|_| "phaselink: replay TX failed")?;
            Ok(())
        }
        None => Err("phaselink: nothing to replay"),
    }
}

/// One GEM RX attempt. Ok(payload) is EtherType 0x88B5 body; Err if none / wrong type.
pub fn recv_payload() -> Result<Vec<u8>, &'static str> {
    if !gem::wire_status().live {
        return Err("phaselink: network not initialized");
    }
    let mut buf = [0u8; RX_BUF_LEN];
    let n = gem::wire_rx_raw(&mut buf).map_err(|_| "phaselink: no packet received")?;
    if n < 14 {
        return Err("phaselink: ethernet frame too short");
    }
    let ethertype = u16::from_be_bytes([buf[12], buf[13]]);
    if ethertype != PHASELINK_ETHERTYPE {
        return Err("phaselink: unexpected ethertype");
    }
    Ok(buf[14..n].to_vec())
}

pub fn recv_and_report(kernel: &mut Kernel) -> Result<String, &'static str> {
    if !gem::wire_status().live {
        return Err("phaselink: network not initialized");
    }
    if kernel.phaselink.local_node == [0u8; 32] {
        return Err("phaselink: not initialized");
    }

    let mut buf = [0u8; RX_BUF_LEN];
    let mut got = 0usize;
    let mut received = false;
    for _ in 0..64 {
        if let Ok(n) = gem::wire_rx_raw(&mut buf) {
            got = n;
            received = true;
            break;
        }
        for _ in 0..50_000 {
            core::hint::spin_loop();
        }
    }
    if !received {
        return Err("phaselink: no packet received");
    }
    if got < 14 {
        return Err("phaselink: ethernet frame too short");
    }
    let ethertype = u16::from_be_bytes([buf[12], buf[13]]);
    if ethertype != PHASELINK_ETHERTYPE {
        return Err("phaselink: unexpected ethertype");
    }
    let payload = &buf[14..got];
    let frame = PhaseLinkDecoder::decode_frame(payload).map_err(|_| "phaselink: decode failed")?;
    let accepted_before = kernel.phaselink.accepted_frames;
    let quarantined_before = kernel.phaselink.quarantined_frames;
    let result = accept_or_quarantine(kernel, &frame);
    Ok(render_receive_report(
        ethertype,
        &result,
        &frame,
        accepted_before,
        quarantined_before,
    ))
}

fn wrap_ethernet(payload: &[u8]) -> Result<Vec<u8>, &'static str> {
    // Keep the Ethernet frame within the GEM wire buffer (wire_tx_raw also enforces).
    if payload.len() + 14 > RX_BUF_LEN {
        return Err("phaselink: frame too large for GEM wire buffer");
    }
    let mut eth = Vec::with_capacity(14 + payload.len());
    eth.extend_from_slice(&[0xff; 6]); // broadcast dst
    eth.extend_from_slice(&BOARD_MAC); // src = 02:00:00:47:01:50
    eth.extend_from_slice(&PHASELINK_ETHERTYPE.to_be_bytes());
    eth.extend_from_slice(payload);
    Ok(eth)
}

fn render_receive_report(
    ethertype: u16,
    result: &FrameValidationResult,
    frame: &crate::kernel::phaselink::PhaseLinkFrame,
    accepted_before: u64,
    quarantined_before: u64,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("[ RECEIVE ] EtherType:       0x{ethertype:04X}\n"));
    out.push_str("[ PHASE   ] frame decoded:   OK\n");
    match result {
        FrameValidationResult::Accepted => {
            out.push_str("[ ROOT    ] EquationRoot:    MATCH\n");
            out.push_str("[ SEQ     ] sequence:        OK\n");
            out.push_str("[ PROOF   ] transition:      OK\n");
            out.push_str("[ CHAIN   ] previous hash:   OK\n");
            out.push_str(&format!(
                "[ PAYLOAD ] {}\n",
                String::from_utf8_lossy(&frame.payload)
            ));
            out.push_str("[ RESULT  ] ACCEPTED\n");
        }
        FrameValidationResult::Quarantined(QuarantineReason::ReplayDetected) => {
            out.push_str("[ QUARANTINE] replay detected\n");
            if accepted_before == quarantined_before {
                out.push_str("[ STATE   ] unchanged\n");
            } else {
                out.push_str("[ STATE   ] quarantined\n");
            }
        }
        FrameValidationResult::Quarantined(reason) => {
            out.push_str(&format!("[ QUARANTINE] {}\n", reason.label()));
            out.push_str("[ STATE   ] unchanged\n");
        }
        FrameValidationResult::Rejected(err) => {
            out.push_str(&format!("[ REJECT  ] {}\n", err.reason()));
            out.push_str("[ STATE   ] unchanged\n");
        }
    }
    out
}

/// UART text → bounded 0x88B5 PhaseNet frame → GEM TX → wait for peer → UART.
pub fn bridge_exchange(kernel: &mut Kernel, text: &str) -> Result<String, &'static str> {
    if !gem::wire_status().live {
        return Err("bridge: network not initialized (run phaselink init)");
    }
    unsafe {
        if !UART_GEM.active {
            UART_GEM.activate(
                kernel.phaselink.equation_root,
                kernel.phaselink.boot_session_root,
            );
            kernel.phaselink.uart_gem_active = true;
        }
    }
    let body = unsafe { UART_GEM.encode_uart(text.as_bytes()) }.map_err(|e| e.as_str())?;
    let eth = UartGemBridge::wrap_ethernet(&body, &BOARD_MAC).map_err(|e| e.as_str())?;
    gem::wire_tx_raw(&eth).map_err(|_| "bridge: GEM0 TX failed")?;
    let mut buf = [0u8; RX_BUF_LEN];
    for _ in 0..400 {
        if let Ok(n) = gem::wire_rx_raw(&mut buf) {
            match unsafe { UART_GEM.accept_ethernet(&buf[..n]) } {
                Ok(payload) => {
                    let text = core::str::from_utf8(&payload).unwrap_or("<bin>");
                    return Ok(alloc::format!(
                        "bridge tx: {} bytes ethertype=0x88B5\nbridge rx: {}\nbridge: uart-gem",
                        eth.len(),
                        text
                    ));
                }
                Err(uart_gem::BridgeError::BadEtherType) => {}
                Err(e) => return Err(e.as_str()),
            }
        }
        for _ in 0..20_000 {
            core::hint::spin_loop();
        }
    }
    Err(uart_gem::BridgeError::PeerLoss.as_str())
}

pub fn bridge_refuse_malformed() -> &'static str {
    uart_gem::BridgeError::Decode.as_str()
}
