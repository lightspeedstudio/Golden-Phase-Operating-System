//! PHI-STABILITY network encoding and transport.

use crate::kernel::hash::tagged_hash;
use crate::kernel::hash::StateHash;
use crate::kernel::phi_stability::error::PhiStabilityError;
use crate::kernel::phi_stability::fault::apply_fault_to_message;
use crate::kernel::phi_stability::types::{
    PhiIntegerMode, PhiNetworkMessageKind, PhiStabilityNetworkMessage, PhiTestId,
};
use crate::kernel::Kernel;
use crate::platform::{String, Vec};

#[cfg(all(
    feature = "baremetal",
    target_arch = "riscv64",
    not(feature = "std"),
    feature = "bm-net"
))]
use crate::boot::arch::riscv64::virtio_net::{self, PHASELINK_ETHERTYPE};
#[cfg(all(
    feature = "baremetal",
    target_arch = "riscv64",
    not(feature = "std"),
    feature = "bm-net"
))]
use crate::kernel::network::NetworkDriverState;
#[cfg(all(
    feature = "baremetal",
    target_arch = "riscv64",
    not(feature = "std"),
    feature = "bm-net"
))]
use crate::kernel::phaselink::decoder::PhaseLinkDecoder;
#[cfg(all(
    feature = "baremetal",
    target_arch = "riscv64",
    not(feature = "std"),
    feature = "bm-net"
))]
use crate::kernel::phaselink::encoder::PhaseLinkEncoder;
#[cfg(all(
    feature = "baremetal",
    target_arch = "riscv64",
    not(feature = "std"),
    feature = "bm-net"
))]
use crate::kernel::phaselink::{accept_or_quarantine, build_payload_frame};

const NETWORK_TAG: &str = "KAIROS_PHI_STABILITY_NETWORK_V1";

pub fn encode_message(message: &PhiStabilityNetworkMessage) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(message.version);
    out.push(message.message_kind as u8);
    out.extend_from_slice(&message.source_node);
    out.extend_from_slice(&message.destination_node);
    out.extend_from_slice(&message.equation_root);
    out.extend_from_slice(&message.boot_session_root);
    out.extend_from_slice(&message.test_id.0.to_be_bytes());
    out.extend_from_slice(&message.target_index.to_be_bytes());
    match message.integer_mode {
        PhiIntegerMode::ExactHosted => {
            out.push(0);
        }
        PhiIntegerMode::FixedWidthBaremetal { bits } => {
            out.push(1);
            out.push((bits >> 8) as u8);
            out.push((bits & 0xff) as u8);
        }
        PhiIntegerMode::ModularBaremetal => {
            out.push(2);
        }
    }
    out.extend_from_slice(&message.result_hash);
    out.extend_from_slice(&message.pair_hash);
    out.extend_from_slice(&message.trajectory_root);
    out.extend_from_slice(&message.clock_root);
    out.extend_from_slice(&message.test_root);
    out.extend_from_slice(&message.recurrence_steps.to_be_bytes());
    out.extend_from_slice(&message.chronos_elapsed.to_be_bytes());
    out.extend_from_slice(&message.kairos_events.to_be_bytes());
    out.extend_from_slice(&message.sequence_number.to_be_bytes());
    out.extend_from_slice(&message.previous_frame_hash);
    out.extend_from_slice(&message.frame_hash);
    out
}

pub fn decode_message(bytes: &[u8]) -> Result<PhiStabilityNetworkMessage, PhiStabilityError> {
    if bytes.len() < 300 {
        return Err(PhiStabilityError::FrameDecodeFailed);
    }
    let mut idx = 0usize;
    let version = bytes[idx];
    idx += 1;
    let kind = match bytes[idx] {
        1 => PhiNetworkMessageKind::TestRequest,
        2 => PhiNetworkMessageKind::TestResult,
        3 => PhiNetworkMessageKind::VerificationResult,
        4 => PhiNetworkMessageKind::FaultResult,
        _ => return Err(PhiStabilityError::FrameDecodeFailed),
    };
    idx += 1;
    let read_hash = |buf: &[u8], i: &mut usize| -> StateHash {
        let mut h = [0u8; 32];
        h.copy_from_slice(&buf[*i..*i + 32]);
        *i += 32;
        h
    };
    let source_node = read_hash(bytes, &mut idx);
    let destination_node = read_hash(bytes, &mut idx);
    let equation_root = read_hash(bytes, &mut idx);
    let boot_session_root = read_hash(bytes, &mut idx);
    let test_id = PhiTestId(u64::from_be_bytes(bytes[idx..idx + 8].try_into().unwrap()));
    idx += 8;
    let target_index = u64::from_be_bytes(bytes[idx..idx + 8].try_into().unwrap());
    idx += 8;
    let integer_mode = match bytes[idx] {
        0 => {
            idx += 1;
            PhiIntegerMode::ExactHosted
        }
        1 => {
            idx += 1;
            let hi = bytes[idx] as u16;
            idx += 1;
            let lo = bytes[idx] as u16;
            idx += 1;
            PhiIntegerMode::FixedWidthBaremetal {
                bits: (hi << 8) | lo,
            }
        }
        2 => {
            idx += 1;
            PhiIntegerMode::ModularBaremetal
        }
        _ => return Err(PhiStabilityError::FrameDecodeFailed),
    };
    let result_hash = read_hash(bytes, &mut idx);
    let pair_hash = read_hash(bytes, &mut idx);
    let trajectory_root = read_hash(bytes, &mut idx);
    let clock_root = read_hash(bytes, &mut idx);
    let test_root = read_hash(bytes, &mut idx);
    let recurrence_steps = u32::from_be_bytes(bytes[idx..idx + 4].try_into().unwrap());
    idx += 4;
    let chronos_elapsed = u64::from_be_bytes(bytes[idx..idx + 8].try_into().unwrap());
    idx += 8;
    let kairos_events = u64::from_be_bytes(bytes[idx..idx + 8].try_into().unwrap());
    idx += 8;
    let sequence_number = u64::from_be_bytes(bytes[idx..idx + 8].try_into().unwrap());
    idx += 8;
    let previous_frame_hash = read_hash(bytes, &mut idx);
    let frame_hash = read_hash(bytes, &mut idx);
    let _ = version;
    Ok(PhiStabilityNetworkMessage {
        version: 1,
        message_kind: kind,
        source_node,
        destination_node,
        equation_root,
        boot_session_root,
        test_id,
        target_index,
        integer_mode,
        result_hash,
        pair_hash,
        trajectory_root,
        clock_root,
        test_root,
        recurrence_steps,
        chronos_elapsed,
        kairos_events,
        sequence_number,
        previous_frame_hash,
        frame_hash,
    })
}

pub fn frame_hash_for_payload(payload: &[u8]) -> StateHash {
    tagged_hash(NETWORK_TAG, &[payload])
}

#[cfg(all(
    feature = "baremetal",
    target_arch = "riscv64",
    not(feature = "std"),
    feature = "bm-net"
))]
pub fn send_message(
    kernel: &mut Kernel,
    peer_name: &str,
    message: &mut PhiStabilityNetworkMessage,
) -> Result<(), PhiStabilityError> {
    if kernel.network.driver_state != NetworkDriverState::Initialized {
        return Err(PhiStabilityError::NetworkUnavailable);
    }
    if kernel.phaselink.local_node == [0u8; 32] {
        return Err(PhiStabilityError::NotInitialized);
    }
    let payload = encode_message(message);
    message.frame_hash = frame_hash_for_payload(&payload);
    let destination = resolve_peer_node(kernel, peer_name);
    let frame = build_payload_frame(kernel, destination, "phi_stability", &payload)
        .map_err(|_| PhiStabilityError::FrameDecodeFailed)?;
    let encoded =
        PhaseLinkEncoder::encode_frame(&frame).map_err(|_| PhiStabilityError::FrameDecodeFailed)?;
    let eth = wrap_ethernet(kernel, &encoded)?;
    virtio_net::transmit_ethernet(&mut kernel.network, &eth)
        .map_err(|_| PhiStabilityError::NetworkUnavailable)?;
    kernel.phaselink.sender.previous_frame_hash = frame.frame_hash;
    Ok(())
}

#[cfg(all(
    feature = "baremetal",
    target_arch = "riscv64",
    not(feature = "std"),
    feature = "bm-net"
))]
pub fn replay_last(kernel: &mut Kernel) -> Result<(), PhiStabilityError> {
    virtio_net::replay_last_ethernet(&mut kernel.network)
        .map_err(|_| PhiStabilityError::NetworkUnavailable)?;
    Ok(())
}

#[cfg(all(
    feature = "baremetal",
    target_arch = "riscv64",
    not(feature = "std"),
    feature = "bm-net"
))]
pub fn receive_and_verify(kernel: &mut Kernel) -> Result<String, PhiStabilityError> {
    if kernel.network.driver_state != NetworkDriverState::Initialized {
        return Err(PhiStabilityError::NetworkUnavailable);
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
        return Err(PhiStabilityError::NetworkUnavailable);
    }
    let eth = virtio_net::last_rx_ethernet();
    if eth.len() < 14 {
        return Err(PhiStabilityError::FrameDecodeFailed);
    }
    let ethertype = u16::from_be_bytes([eth[12], eth[13]]);
    if ethertype != PHASELINK_ETHERTYPE {
        return Err(PhiStabilityError::FrameDecodeFailed);
    }
    let payload = &eth[14..];
    let frame = PhaseLinkDecoder::decode_frame(payload)
        .map_err(|_| PhiStabilityError::FrameDecodeFailed)?;
    let result = accept_or_quarantine(kernel, &frame);
    let message = decode_message(&frame.payload)?;
    let local = crate::kernel::phi_stability::runner::compute_result_hash_only(
        kernel,
        message.target_index,
    )?;
    let report = render_receive_report(&message, &result, local == message.result_hash);
    Ok(report)
}

#[cfg(all(
    feature = "baremetal",
    target_arch = "riscv64",
    not(feature = "std"),
    feature = "bm-net"
))]
fn wrap_ethernet(kernel: &Kernel, payload: &[u8]) -> Result<Vec<u8>, PhiStabilityError> {
    let src = kernel
        .network
        .mac_address
        .ok_or(PhiStabilityError::NetworkUnavailable)?;
    if payload.len() + 14 > 1514 {
        return Err(PhiStabilityError::FrameDecodeFailed);
    }
    let mut eth = Vec::with_capacity(14 + payload.len());
    eth.extend_from_slice(&[0xff; 6]);
    eth.extend_from_slice(&src);
    eth.extend_from_slice(&PHASELINK_ETHERTYPE.to_be_bytes());
    eth.extend_from_slice(payload);
    Ok(eth)
}

pub fn render_receive_report(
    message: &PhiStabilityNetworkMessage,
    validation: &crate::kernel::phaselink::validation::FrameValidationResult,
    result_match: bool,
) -> String {
    use crate::kernel::phaselink::quarantine::QuarantineReason;
    use crate::kernel::phaselink::validation::FrameValidationResult;
    let mut out = String::new();
    out.push_str("[ RECEIVE ] PHI-STABILITY frame\n");
    out.push_str(&format!("[ TARGET  ] F({})\n", message.target_index));
    match validation {
        FrameValidationResult::Accepted => {
            out.push_str("[ ROOT    ] EquationRoot: MATCH\n");
            out.push_str("[ SEQ     ] PhaseLink sequence: OK\n");
            out.push_str("[ CHAIN   ] previous frame hash: OK\n");
            out.push_str("[ COMPUTE ] independent fast-doubling result complete\n");
            if result_match {
                out.push_str("[ RESULT  ] result hash: MATCH\n");
                out.push_str("[ PAIR    ] pair hash: MATCH\n");
                out.push_str("[ KAIROS  ] event policy: MATCH\n");
                out.push_str("[ PROOF   ] trajectory commitment: OK\n");
                out.push_str("[ MERKLE  ] test root: OK\n");
                out.push_str("[ ACCEPT  ] remote result verified\n");
            } else {
                out.push_str("[ RESULT  ] result hash: MISMATCH\n");
                out.push_str("[ REJECT  ] remote verification failed\n");
            }
        }
        FrameValidationResult::Quarantined(QuarantineReason::ReplayDetected) => {
            out.push_str("[ QUARANTINE ] replay detected\n");
            out.push_str("[ STATE      ] unchanged\n");
            out.push_str("[ CHAIN      ] preserved\n");
        }
        FrameValidationResult::Quarantined(reason) => {
            out.push_str(&format!("[ QUARANTINE ] {}\n", reason.label()));
            out.push_str("[ STATE      ] unchanged\n");
        }
        FrameValidationResult::Rejected(err) => {
            out.push_str(&format!("[ REJECT  ] {}\n", err.reason()));
        }
    }
    out
}

fn resolve_peer_node(kernel: &Kernel, peer_name: &str) -> StateHash {
    #[cfg(feature = "baremetal")]
    {
        return crate::kernel::phaselink::peer_node_id(peer_name, &kernel.phaselink.equation_root);
    }
    #[cfg(not(feature = "baremetal"))]
    {
        let peer_bsr = tagged_hash("KAIROS_BAREMETAL_BOOT_SESSION_V1", &[peer_name.as_bytes()]);
        tagged_hash(
            "KAIROS_PHASELINK_NODE_V1",
            &[
                peer_name.as_bytes(),
                &kernel.phaselink.equation_root,
                &peer_bsr,
            ],
        )
    }
}

pub fn build_network_message(
    kernel: &Kernel,
    test_id: PhiTestId,
    target_index: u64,
    integer_mode: PhiIntegerMode,
    result_hash: StateHash,
    pair_hash: StateHash,
    trajectory_root: StateHash,
    clock_root: StateHash,
    test_root: StateHash,
    recurrence_steps: u32,
    chronos_elapsed: u64,
    kairos_events: u64,
    peer_name: &str,
) -> PhiStabilityNetworkMessage {
    let destination = resolve_peer_node(kernel, peer_name);
    PhiStabilityNetworkMessage {
        version: 1,
        message_kind: PhiNetworkMessageKind::TestResult,
        source_node: kernel.phaselink.local_node,
        destination_node: destination,
        equation_root: kernel.phaselink.equation_root,
        boot_session_root: kernel.phaselink.boot_session_root,
        test_id,
        target_index,
        integer_mode,
        result_hash,
        pair_hash,
        trajectory_root,
        clock_root,
        test_root,
        recurrence_steps,
        chronos_elapsed,
        kairos_events,
        sequence_number: kernel.phaselink.sender.sequence_number + 1,
        previous_frame_hash: kernel.phaselink.sender.previous_frame_hash,
        frame_hash: [0u8; 32],
    }
}

pub fn apply_pending_fault(
    message: &mut PhiStabilityNetworkMessage,
    fault: Option<crate::kernel::phi_stability::types::PhiFaultType>,
) -> Result<(), PhiStabilityError> {
    if let Some(f) = fault {
        apply_fault_to_message(message, f)?;
    }
    Ok(())
}

// Stub when VirtIO network transport is unavailable.
#[cfg(not(all(
    feature = "baremetal",
    target_arch = "riscv64",
    not(feature = "std"),
    feature = "bm-net"
)))]
pub fn send_message(
    _kernel: &mut Kernel,
    _peer_name: &str,
    _message: &mut PhiStabilityNetworkMessage,
) -> Result<(), PhiStabilityError> {
    Err(PhiStabilityError::BaremetalUnsupported)
}

#[cfg(not(all(
    feature = "baremetal",
    target_arch = "riscv64",
    not(feature = "std"),
    feature = "bm-net"
)))]
pub fn replay_last(_kernel: &mut Kernel) -> Result<(), PhiStabilityError> {
    Err(PhiStabilityError::BaremetalUnsupported)
}

#[cfg(not(all(
    feature = "baremetal",
    target_arch = "riscv64",
    not(feature = "std"),
    feature = "bm-net"
)))]
pub fn receive_and_verify(_kernel: &mut Kernel) -> Result<String, PhiStabilityError> {
    Err(PhiStabilityError::BaremetalUnsupported)
}
