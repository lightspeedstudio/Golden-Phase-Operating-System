//! PhaseLink over VirtIO Ethernet for baremetal cluster nodes.

use crate::boot::arch::riscv64::virtio_net::{self, PHASELINK_ETHERTYPE};
use crate::kernel::network::NetworkDriverState;
use crate::kernel::phaselink::decoder::PhaseLinkDecoder;
use crate::kernel::phaselink::encoder::PhaseLinkEncoder;
use crate::kernel::phaselink::quarantine::QuarantineReason;
use crate::kernel::phaselink::{
    accept_or_quarantine, build_payload_frame, init_baremetal_node, peer_node_id,
    FrameValidationResult,
};
use crate::kernel::Kernel;
use crate::platform::{String, Vec};

pub fn init_node(
    kernel: &mut Kernel,
    local_name: &str,
    peer_name: &str,
) -> Result<(), &'static str> {
    if local_name.is_empty() || peer_name.is_empty() {
        return Err("phaselink: usage init <local-node> <peer-node>");
    }
    init_baremetal_node(kernel, local_name, peer_name);
    Ok(())
}

pub fn send_text(kernel: &mut Kernel, peer_name: &str, text: &str) -> Result<(), &'static str> {
    if kernel.network.driver_state != NetworkDriverState::Initialized {
        return Err("phaselink: network not initialized");
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
    let eth = wrap_ethernet(kernel, &encoded)?;
    virtio_net::transmit_ethernet(&mut kernel.network, &eth)?;
    kernel.phaselink.sender.previous_frame_hash = frame.frame_hash;
    Ok(())
}

pub fn replay_last(kernel: &mut Kernel) -> Result<(), &'static str> {
    if kernel.network.driver_state != NetworkDriverState::Initialized {
        return Err("phaselink: network not initialized");
    }
    virtio_net::replay_last_ethernet(&mut kernel.network)?;
    Ok(())
}

pub fn recv_and_report(kernel: &mut Kernel) -> Result<String, &'static str> {
    if kernel.network.driver_state != NetworkDriverState::Initialized {
        return Err("phaselink: network not initialized");
    }
    if kernel.phaselink.local_node == [0u8; 32] {
        return Err("phaselink: not initialized");
    }

    let mut received = false;
    for _ in 0..64 {
        if virtio_net::receive_frame(&mut kernel.network).is_ok() {
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
    let eth = virtio_net::last_rx_ethernet();
    if eth.len() < 14 {
        return Err("phaselink: ethernet frame too short");
    }
    let ethertype = u16::from_be_bytes([eth[12], eth[13]]);
    if ethertype != PHASELINK_ETHERTYPE {
        return Err("phaselink: unexpected ethertype");
    }
    let payload = &eth[14..];
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

fn wrap_ethernet(kernel: &Kernel, payload: &[u8]) -> Result<Vec<u8>, &'static str> {
    let src = kernel
        .network
        .mac_address
        .ok_or("phaselink: no source mac")?;
    if payload.len() + 14 > 1514 {
        return Err("phaselink: frame too large");
    }
    let mut eth = Vec::with_capacity(14 + payload.len());
    eth.extend_from_slice(&[0xff; 6]);
    eth.extend_from_slice(&src);
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
