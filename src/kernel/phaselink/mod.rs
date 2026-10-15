//! PhaseLink v1 and controlled binary bridge.

pub mod chain;
pub mod decoder;
pub mod encoder;
pub mod error;
pub mod frame;
pub mod loopback;
pub mod merkle;
pub mod phase_stream;
pub mod proof;
pub mod quarantine;
pub mod simulation;
pub mod transport;
pub mod types;
pub mod validation;

use crate::core::gphase_core::PhaseState;
use crate::kernel::authority::AuthorityMode;
use crate::kernel::boot_session::compute_boot_session_root;
use crate::kernel::hash::tagged_hash;
use crate::kernel::phaselink::chain::{
    PhaseLinkChain, PhaseLinkEvent, PhaseLinkEventKind, PhaseLinkReceiverState,
    PhaseLinkSenderState,
};
use crate::kernel::phaselink::decoder::PhaseLinkDecoder;
use crate::kernel::phaselink::encoder::PhaseLinkEncoder;
use crate::kernel::phaselink::error::PhaseLinkError;
use crate::kernel::phaselink::loopback::LoopbackPhaseLink;
use crate::kernel::phaselink::merkle::{leaf_hash, recompute_from_events, short_root};
use crate::kernel::phaselink::proof::build_output_proof;
use crate::kernel::phaselink::quarantine::{QuarantineLog, QuarantineReason};
use crate::kernel::phaselink::simulation::TwoNodeSimulation;
use crate::kernel::phaselink::transport::{BinaryDeviceBridge, HostedMemoryBridge};
use crate::kernel::phaselink::validation::{PhaseLinkValidator, ValidatorContext};
use crate::kernel::state_proof::{hash_payload, ObjectKind};
#[cfg(feature = "std")]
use crate::kernel::state_root::SubsystemKind;
#[cfg(feature = "std")]
use crate::kernel::statechain::StateEvent;
use crate::kernel::Kernel;
use crate::platform::{HashSet, String, Vec};

pub use error::PhaseLinkError as Error;
pub use phase_stream::{
    bytes_to_phase_stream, parse_phase_symbols, phase_stream_to_bytes, render_phase_stream,
};
pub use simulation::{SimulatedPhaseNode, SimulatedWire};
pub use types::*;
pub use validation::FrameValidationResult;

/// Where PhaseLink frames move. Not the operator console.
#[cfg(feature = "std")]
pub fn transport_label() -> &'static str {
    "hosted loopback"
}

#[cfg(all(not(feature = "std"), target_arch = "arm", feature = "bm-net"))]
pub fn transport_label() -> &'static str {
    "GEM0 EtherType 0x88B5"
}

#[cfg(all(not(feature = "std"), target_arch = "riscv64", feature = "bm-net"))]
pub fn transport_label() -> &'static str {
    "VirtIO EtherType 0x88B5"
}

#[cfg(all(
    not(feature = "std"),
    not(all(
        feature = "bm-net",
        any(target_arch = "arm", target_arch = "riscv64")
    ))
))]
pub fn transport_label() -> &'static str {
    "unavailable"
}

#[derive(Debug, Default)]
pub struct PhaseLinkController {
    pub node_name: String,
    pub local_node: NodeId,
    pub equation_root: Hash,
    pub boot_session_root: Hash,
    pub sender: PhaseLinkSenderState,
    pub receiver: PhaseLinkReceiverState,
    pub accepted_chain: PhaseLinkChain,
    pub quarantine: QuarantineLog,
    pub root_leaves: Vec<Hash>,
    pub root: Hash,
    pub gateway: PhaseGatewayPolicy,
    pub trusted_nodes: HashSet<NodeId>,
    pub loopback: LoopbackPhaseLink,
    pub simulation: Option<TwoNodeSimulation>,
    pub accepted_frames: u64,
    pub quarantined_frames: u64,
    pub uart_gem_active: bool,
}

impl PhaseLinkController {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(any(feature = "std", feature = "baremetal"))]
    pub fn init(&mut self, equation_root: Hash, boot_session_root: Hash, node_name: &str) {
        self.node_name = node_name.into();
        self.local_node = tagged_hash(
            "KAIROS_PHASELINK_NODE_V1",
            &[node_name.as_bytes(), &equation_root, &boot_session_root],
        );
        self.equation_root = equation_root;
        self.boot_session_root = boot_session_root;
        self.sender = PhaseLinkSenderState::default();
        self.receiver = PhaseLinkReceiverState::default();
        self.accepted_chain = PhaseLinkChain::default();
        self.quarantine = QuarantineLog::default();
        self.root_leaves.clear();
        self.root = recompute_from_events(&self.root_leaves);
        self.gateway = PhaseGatewayPolicy::default();
        self.trusted_nodes.clear();
        self.trusted_nodes.insert(self.local_node);
        self.loopback = LoopbackPhaseLink::default();
        self.simulation = None;
        self.accepted_frames = 0;
        self.quarantined_frames = 0;
        self.uart_gem_active = false;
    }

    pub fn status_lines(&self) -> String {
        let last_hash = short_root(&self.sender.previous_frame_hash);
        let mut out = format!(
            "version: PhaseLink v1\nnode: {}\ntransport: {}\n",
            self.node_name,
            transport_label(),
        );
        #[cfg(all(not(feature = "std"), target_arch = "arm", feature = "bm-net"))]
        {
            out.push_str("console: PS UART0\n");
            out.push_str(&format!(
                "bridge: {}\n",
                crate::kernel::phasenet::uart_gem::status_label(self.uart_gem_active)
            ));
        }
        out.push_str(&format!(
            "gateway: {}\nsend sequence: {}\nreceive sequence: {}\naccepted frames: {}\nquarantined frames: {}\nlast frame hash: {}\nchain: {}\nPhaseLinkRoot: {}\nANOM decryptor access: {}\n",
            match self.gateway.state {
                GatewayState::Disabled => "disabled",
                GatewayState::RootAuthorized => "root-authorized",
                GatewayState::Active => "active",
                GatewayState::Quarantined => "quarantined",
            },
            self.sender.sequence_number,
            self.receiver
                .remotes
                .values()
                .map(|r| r.last_sequence_number)
                .max()
                .unwrap_or(0),
            self.accepted_frames,
            self.quarantined_frames,
            last_hash,
            if self.verify_chain() { "OK" } else { "FAIL" },
            short_root(&self.root),
            if self.gateway.allow_anom_decryption { "ALLOWED" } else { "DENIED" },
        ));
        out
    }

    pub fn verify_chain(&self) -> bool {
        let mut prev = [0u8; 32];
        for event in &self.accepted_chain.events {
            if event.previous_frame_hash != prev {
                return false;
            }
            prev = event.frame_hash;
        }
        true
    }

    fn push_leaf(&mut self, event: &PhaseLinkEvent, status: &str) {
        self.root_leaves.push(leaf_hash(
            event.source_node,
            event.destination_node,
            event.sequence_number,
            event.frame_hash,
            event.link_chain_head,
            status,
        ));
        self.root = recompute_from_events(&self.root_leaves);
    }
}

#[cfg(feature = "std")]
pub fn init_controller(kernel: &mut Kernel) {
    let equation_root = kernel.statechain.equation_root();
    let boot_session_root = kernel.authority.boot_session_root;
    kernel
        .phaselink
        .init(equation_root, boot_session_root, "local");
}

#[cfg(feature = "std")]
pub fn build_text_frame(
    kernel: &mut Kernel,
    destination_node: NodeId,
    payload: &[u8],
    flags: PhaseLinkFlags,
) -> Result<PhaseLinkFrame, PhaseLinkError> {
    kernel.phaselink.sender.sequence_number += 1;
    let sequence = kernel.phaselink.sender.sequence_number;
    let snapshot = kernel.clock.snapshot();
    let payload_hash = hash_payload(payload);
    let proof = build_output_proof(
        kernel.phaselink.equation_root,
        kernel.phaselink.boot_session_root,
        kernel.phaselink.local_node,
        destination_node,
        ObjectKind::Driver,
        PHASELINK_STATECHAIN_OBJECT_ID,
        "loopback_text",
        PhaseState::Phi1,
        PhaseState::Phi2,
        sequence,
        kernel.phaselink.sender.previous_frame_hash,
        payload,
        snapshot.proper_tick,
        snapshot.kairos_tick,
        snapshot.sector_id as i8,
    );
    let event = StateEvent {
        proof: proof.transition.clone(),
        event_hash: kernel.phaselink.sender.previous_frame_hash,
        tick: kernel.statechain.tick(),
        subsystem: SubsystemKind::Driver,
        assoc_parity: kernel.statechain.assoc_parity(),
        accepted: true,
        reason: String::new(),
    };
    let ctx = PhaseLinkContext {
        source_node: kernel.phaselink.local_node,
        destination_node,
        equation_root: kernel.phaselink.equation_root,
        boot_session_root: kernel.phaselink.boot_session_root,
        sequence_number: sequence,
        chronos_tick: snapshot.proper_tick,
        kairos_tick: snapshot.kairos_tick,
        stateclock_sector: snapshot.sector_id as i8,
        previous_frame_hash: kernel.phaselink.sender.previous_frame_hash,
        payload_encoding: PayloadEncoding::RawBinary,
        flags,
    };
    PhaseLinkEncoder::build_state_frame(&ctx, &event, payload).map(|mut frame| {
        frame.payload_hash = payload_hash;
        frame
    })
}

#[cfg(feature = "std")]
pub fn build_process_commitment_frame(
    kernel: &mut Kernel,
    pid: u32,
    destination_node: NodeId,
    operation: &str,
    previous_phase: PhaseState,
    next_phase: PhaseState,
) -> Result<PhaseLinkFrame, PhaseLinkError> {
    kernel.phaselink.sender.sequence_number += 1;
    let sequence = kernel.phaselink.sender.sequence_number;
    let snapshot = kernel.clock.snapshot();
    let ctx = PhaseLinkContext {
        source_node: kernel.phaselink.local_node,
        destination_node,
        equation_root: kernel.phaselink.equation_root,
        boot_session_root: kernel.phaselink.boot_session_root,
        sequence_number: sequence,
        chronos_tick: snapshot.proper_tick,
        kairos_tick: snapshot.kairos_tick,
        stateclock_sector: snapshot.sector_id as i8,
        previous_frame_hash: kernel.phaselink.sender.previous_frame_hash,
        payload_encoding: PayloadEncoding::PublicCommitment,
        flags: PhaseLinkFlags {
            anom_public: true,
            proof_required: true,
            ..PhaseLinkFlags::default()
        },
    };
    let public = kernel.public_process_view(pid);
    PhaseLinkEncoder::public_commitment_frame(
        &ctx,
        ObjectKind::Process,
        &pid.to_string(),
        operation,
        previous_phase,
        next_phase,
        public,
    )
}

#[cfg(feature = "std")]
pub fn send_loopback_text(
    kernel: &mut Kernel,
    text: &str,
) -> Result<(PhaseLinkFrame, Vec<u8>), PhaseLinkError> {
    let mut bridge = HostedMemoryBridge;
    let phase_stream = bridge.receive_binary(text.as_bytes())?;
    kernel
        .phase_math
        .enqueue_network(PHASELINK_STATECHAIN_OBJECT_ID);
    let frame = build_text_frame(
        kernel,
        kernel.phaselink.local_node,
        text.as_bytes(),
        PhaseLinkFlags {
            proof_required: true,
            loopback: true,
            ..PhaseLinkFlags::default()
        },
    )?;
    let bytes = PhaseLinkEncoder::encode_frame(&frame)?;
    kernel.phaselink.loopback.send(bytes.clone());
    kernel.phaselink.sender.previous_frame_hash = frame.frame_hash;
    let payload = format!(
        "frame_sent|seq={}|hash={}",
        frame.sequence_number,
        short_root(&frame.frame_hash)
    );
    let _ = kernel.statechain.record_transition(
        ObjectKind::Driver,
        PHASELINK_STATECHAIN_OBJECT_ID,
        SubsystemKind::Driver,
        PhaseState::Phi1,
        PhaseState::Phi2,
        "phaselink_frame_sent",
        None,
        payload.as_bytes(),
    );
    let _ = phase_stream;
    Ok((frame, bytes))
}

#[cfg(feature = "std")]
pub fn recv_loopback(
    kernel: &mut Kernel,
) -> Result<(PhaseLinkFrame, Vec<u8>, FrameValidationResult), PhaseLinkError> {
    let bytes = kernel
        .phaselink
        .loopback
        .recv()
        .ok_or(PhaseLinkError::TransportUnavailable)?;
    let frame = PhaseLinkDecoder::decode_frame(&bytes)?;
    let result = accept_or_quarantine(kernel, &frame);
    Ok((frame, bytes, result))
}

#[cfg(feature = "std")]
pub fn accept_or_quarantine(kernel: &mut Kernel, frame: &PhaseLinkFrame) -> FrameValidationResult {
    let ctx = ValidatorContext {
        equation_root: kernel.phaselink.equation_root,
        boot_session_root: kernel.phaselink.boot_session_root,
        trusted_nodes: kernel.phaselink.trusted_nodes.clone(),
        equation_policy: EquationRootPolicy::ExactMatch,
        gap_policy: SequenceGapPolicy::Quarantine,
        gateway: kernel.phaselink.gateway.clone(),
        remotes: kernel.phaselink.receiver.remotes.clone(),
    };
    let previous_boot = kernel
        .phaselink
        .receiver
        .remotes
        .get(&frame.source_node)
        .map(|r| r.boot_session_root);
    let result = PhaseLinkValidator::validate(frame, &ctx);
    match &result {
        FrameValidationResult::Accepted => {
            kernel
                .phase_math
                .enqueue_proof(PHASELINK_STATECHAIN_OBJECT_ID, frame.sequence_number);
            kernel
                .phase_math
                .enqueue_merkle_leaf(PHASELINK_STATECHAIN_OBJECT_ID);
            if previous_boot.is_some() && previous_boot != Some(frame.boot_session_root) {
                record_control_event(
                    kernel,
                    "remote_boot_session_changed",
                    frame,
                    Some(QuarantineReason::BootSessionChanged),
                );
            }
            kernel.phaselink.receiver.remotes.insert(
                frame.source_node,
                RemoteSequenceState {
                    source_node: frame.source_node,
                    boot_session_root: frame.boot_session_root,
                    last_sequence_number: frame.sequence_number,
                    last_frame_hash: frame.frame_hash,
                },
            );
            kernel.phaselink.accepted_frames += 1;
            let event = PhaseLinkEvent {
                kind: PhaseLinkEventKind::FrameAccepted,
                source_node: frame.source_node,
                destination_node: frame.destination_node,
                sequence_number: frame.sequence_number,
                frame_hash: frame.frame_hash,
                previous_frame_hash: frame.previous_frame_hash,
                payload_hash: frame.payload_hash,
                public_commitment: frame.public_commitment,
                operation: frame.operation.clone(),
                previous_phase: frame.previous_phase,
                next_phase: frame.next_phase,
                link_chain_head: frame.frame_hash,
                quarantine_reason: None,
            };
            kernel.phaselink.accepted_chain.push(event.clone());
            kernel.phaselink.push_leaf(&event, "accepted");
            record_control_event(kernel, "frame_accepted", frame, None);
        }
        FrameValidationResult::Quarantined(reason) => {
            kernel.phaselink.quarantined_frames += 1;
            kernel
                .phaselink
                .quarantine
                .push(PhaseLinkValidator::quarantined_meta(frame, reason.clone()));
            record_control_event(kernel, "frame_quarantined", frame, Some(reason.clone()));
        }
        FrameValidationResult::Rejected(_) => {
            record_control_event(kernel, "frame_rejected", frame, None);
        }
    }
    result
}

#[cfg(feature = "std")]
fn record_control_event(
    kernel: &mut Kernel,
    operation: &str,
    frame: &PhaseLinkFrame,
    quarantine: Option<QuarantineReason>,
) {
    let payload = if let Some(reason) = quarantine {
        format!(
            "{}|{}|seq={}|hash={}",
            operation,
            reason.label(),
            frame.sequence_number,
            short_root(&frame.frame_hash)
        )
    } else {
        format!(
            "{}|seq={}|hash={}",
            operation,
            frame.sequence_number,
            short_root(&frame.frame_hash)
        )
    };
    let _ = kernel.statechain.record_transition(
        ObjectKind::Driver,
        PHASELINK_STATECHAIN_OBJECT_ID,
        SubsystemKind::Driver,
        frame.previous_phase,
        frame.next_phase,
        operation,
        None,
        payload.as_bytes(),
    );
}

#[cfg(feature = "std")]
pub fn sim_init(kernel: &mut Kernel) {
    let boot_b = compute_boot_session_root(
        &kernel.phaselink.equation_root,
        1,
        &kernel.statechain.kernel_root(),
        kernel.clock.proper_tick + 1,
    );
    kernel.phaselink.simulation = Some(TwoNodeSimulation::new(
        kernel.phaselink.equation_root,
        kernel.phaselink.boot_session_root,
        boot_b,
    ));
    if let Some(sim) = &kernel.phaselink.simulation {
        kernel.phaselink.trusted_nodes.insert(sim.node_a.node_id);
        kernel.phaselink.trusted_nodes.insert(sim.node_b.node_id);
    }
}

#[cfg(feature = "std")]
pub fn sim_send_text(
    kernel: &mut Kernel,
    from: &str,
    to: &str,
    text: &str,
) -> Result<(PhaseLinkFrame, FrameValidationResult), PhaseLinkError> {
    let (src, dst) = {
        let sim = kernel
            .phaselink
            .simulation
            .as_ref()
            .ok_or(PhaseLinkError::TransportUnavailable)?;
        (
            sim.node(from).ok_or(PhaseLinkError::UnknownNode)?.clone(),
            sim.node(to).ok_or(PhaseLinkError::UnknownNode)?.clone(),
        )
    };
    let snapshot = kernel.clock.snapshot();
    let payload_hash = hash_payload(text.as_bytes());
    let proof = build_output_proof(
        kernel.phaselink.equation_root,
        src.boot_session_root,
        src.node_id,
        dst.node_id,
        ObjectKind::Driver,
        PHASELINK_STATECHAIN_OBJECT_ID,
        "sim_text",
        PhaseState::Phi1,
        PhaseState::Phi2,
        src.sender.sequence_number + 1,
        src.sender.previous_frame_hash,
        text.as_bytes(),
        snapshot.proper_tick,
        snapshot.kairos_tick,
        snapshot.sector_id as i8,
    );
    let frame = PhaseLinkFrame {
        version: PHASELINK_VERSION,
        flags: PhaseLinkFlags {
            proof_required: true,
            simulation: true,
            ..PhaseLinkFlags::default()
        },
        source_node: src.node_id,
        destination_node: dst.node_id,
        equation_root: kernel.phaselink.equation_root,
        boot_session_root: src.boot_session_root,
        sequence_number: src.sender.sequence_number + 1,
        chronos_tick: snapshot.proper_tick,
        kairos_tick: snapshot.kairos_tick,
        stateclock_sector: snapshot.sector_id as i8,
        object_kind: PhaseLinkObjectKind::Driver,
        object_id: "sim".into(),
        operation: "sim_text".into(),
        previous_phase: PhaseState::Phi1,
        next_phase: PhaseState::Phi2,
        previous_frame_hash: src.sender.previous_frame_hash,
        transition_proof: proof.transition,
        payload_encoding: PayloadEncoding::RawBinary,
        payload: text.as_bytes().to_vec(),
        payload_hash,
        public_commitment: tagged_hash("KAIROS_PHASELINK_SIM_COMMITMENT_V1", &[&payload_hash]),
        frame_hash: [0u8; 32],
    };
    let mut frame = frame;
    let bytes_wo = frame::serialize_frame_without_hash(&frame)?;
    frame.frame_hash = frame::compute_frame_hash(&bytes_wo);
    {
        let sim = kernel
            .phaselink
            .simulation
            .as_mut()
            .ok_or(PhaseLinkError::TransportUnavailable)?;
        sim.enqueue(from, to, PhaseLinkEncoder::encode_frame(&frame)?);
        if let Some(sender) = sim.node_mut(from) {
            sender.sender.sequence_number += 1;
            sender.sender.previous_frame_hash = frame.frame_hash;
        }
    }
    let (_source_name, bytes) = {
        let sim = kernel
            .phaselink
            .simulation
            .as_mut()
            .ok_or(PhaseLinkError::TransportUnavailable)?;
        sim.recv(to).ok_or(PhaseLinkError::TransportUnavailable)?
    };
    let received = PhaseLinkDecoder::decode_frame(&bytes)?;
    let result = accept_or_quarantine(kernel, &received);
    if matches!(result, FrameValidationResult::Accepted) {
        let sim = kernel
            .phaselink
            .simulation
            .as_mut()
            .ok_or(PhaseLinkError::TransportUnavailable)?;
        if let Some(dst_node) = sim.node_mut(to) {
            dst_node.chain.push(
                kernel
                    .phaselink
                    .accepted_chain
                    .events
                    .last()
                    .cloned()
                    .expect("accepted"),
            );
        }
    }
    Ok((received, result))
}

#[cfg(feature = "std")]
pub fn sim_replay(kernel: &mut Kernel) -> bool {
    let Some(sim) = kernel.phaselink.simulation.as_mut() else {
        return false;
    };
    sim.replay_last()
}

#[cfg(feature = "std")]
pub fn sim_set_tamper(kernel: &mut Kernel, field: &str) -> bool {
    let Some(sim) = kernel.phaselink.simulation.as_mut() else {
        return false;
    };
    sim.pending_tamper = Some(field.into());
    true
}

#[cfg(feature = "std")]
pub fn sim_reboot(kernel: &mut Kernel, node: &str) -> bool {
    let Some(sim) = kernel.phaselink.simulation.as_mut() else {
        return false;
    };
    sim.reboot(node)
}

#[cfg(feature = "std")]
pub fn gateway_enable(kernel: &mut Kernel, passphrase: &[u8]) -> Result<(), PhaseLinkError> {
    if kernel.authority.authority != AuthorityMode::Anom {
        return Err(PhaseLinkError::UnauthorizedOperation);
    }
    let session = kernel
        .anom
        .session()
        .ok_or(PhaseLinkError::UnauthorizedOperation)?;
    let expected = crate::kernel::anom::decryptor::session_seal_from_passphrase(
        passphrase,
        &session.equation_root,
        &session.boot_session_root,
    );
    if expected != kernel.anom.session_seal() {
        return Err(PhaseLinkError::UnauthorizedOperation);
    }
    kernel.phaselink.gateway.state = GatewayState::Active;
    kernel.phaselink.gateway.allow_binary_input = true;
    kernel.phaselink.gateway.allow_binary_output = true;
    let payload = format!(
        "gateway_enabled|bsr={}",
        short_root(&kernel.authority.boot_session_root)
    );
    let _ = kernel.statechain.record_transition(
        ObjectKind::Driver,
        PHASELINK_STATECHAIN_OBJECT_ID,
        SubsystemKind::Driver,
        PhaseState::Phi1,
        PhaseState::Phi2,
        "gateway_enabled",
        None,
        payload.as_bytes(),
    );
    Ok(())
}

#[cfg(feature = "std")]
pub fn gateway_disable(kernel: &mut Kernel) {
    kernel.phaselink.gateway = PhaseGatewayPolicy::default();
    let _ = kernel.statechain.record_transition(
        ObjectKind::Driver,
        PHASELINK_STATECHAIN_OBJECT_ID,
        SubsystemKind::Driver,
        PhaseState::Phi2,
        PhaseState::Phi1,
        "gateway_disabled",
        None,
        b"gateway_disabled",
    );
}

#[cfg(feature = "baremetal")]
pub fn init_baremetal_node(kernel: &mut Kernel, local_name: &str, peer_name: &str) {
    use crate::kernel::hash::tagged_hash;
    let equation_root = tagged_hash("KAIROS_BAREMETAL_EQUATION_V1", &[b"cluster"]);
    let local_bsr = tagged_hash("KAIROS_BAREMETAL_BOOT_SESSION_V1", &[local_name.as_bytes()]);
    kernel.phaselink.init(equation_root, local_bsr, local_name);
    kernel
        .phaselink
        .trusted_nodes
        .insert(peer_node_id(peer_name, &equation_root));
}

#[cfg(feature = "baremetal")]
pub fn peer_node_id(peer_name: &str, equation_root: &Hash) -> NodeId {
    use crate::kernel::hash::tagged_hash;
    let peer_bsr = tagged_hash("KAIROS_BAREMETAL_BOOT_SESSION_V1", &[peer_name.as_bytes()]);
    tagged_hash(
        "KAIROS_PHASELINK_NODE_V1",
        &[peer_name.as_bytes(), equation_root, &peer_bsr],
    )
}

#[cfg(feature = "baremetal")]
pub fn build_payload_frame(
    kernel: &mut Kernel,
    destination_node: NodeId,
    operation: &str,
    payload: &[u8],
) -> Result<PhaseLinkFrame, PhaseLinkError> {
    use crate::kernel::hash::tagged_hash;
    use crate::kernel::phaselink::frame::{compute_frame_hash, serialize_frame_without_hash};
    use crate::kernel::phaselink::types::{
        PayloadEncoding, PhaseLinkFlags, PhaseLinkObjectKind, PHASELINK_VERSION,
    };

    kernel.phaselink.sender.sequence_number += 1;
    let sequence = kernel.phaselink.sender.sequence_number;
    let snapshot = kernel.clock.snapshot();
    let payload_hash = hash_payload(payload);
    let proof = build_output_proof(
        kernel.phaselink.equation_root,
        kernel.phaselink.boot_session_root,
        kernel.phaselink.local_node,
        destination_node,
        ObjectKind::Driver,
        PHASELINK_STATECHAIN_OBJECT_ID,
        operation,
        PhaseState::Phi1,
        PhaseState::Phi2,
        sequence,
        kernel.phaselink.sender.previous_frame_hash,
        payload,
        snapshot.proper_tick,
        snapshot.kairos_tick,
        snapshot.sector_id as i8,
    );
    let public_commitment = tagged_hash(
        "KAIROS_PHASELINK_PUBLIC_COMMITMENT_V1",
        &[
            &payload_hash,
            &kernel.phaselink.sender.previous_frame_hash,
            &sequence.to_be_bytes(),
        ],
    );
    let mut frame = PhaseLinkFrame {
        version: PHASELINK_VERSION,
        flags: PhaseLinkFlags {
            proof_required: true,
            ..PhaseLinkFlags::default()
        },
        source_node: kernel.phaselink.local_node,
        destination_node,
        equation_root: kernel.phaselink.equation_root,
        boot_session_root: kernel.phaselink.boot_session_root,
        sequence_number: sequence,
        chronos_tick: snapshot.proper_tick,
        kairos_tick: snapshot.kairos_tick,
        stateclock_sector: snapshot.sector_id as i8,
        object_kind: PhaseLinkObjectKind::Driver,
        object_id: "phaselink".into(),
        operation: operation.into(),
        previous_phase: PhaseState::Phi1,
        next_phase: PhaseState::Phi2,
        previous_frame_hash: kernel.phaselink.sender.previous_frame_hash,
        transition_proof: proof.transition,
        payload_encoding: PayloadEncoding::RawBinary,
        payload: payload.to_vec(),
        payload_hash,
        public_commitment,
        frame_hash: [0u8; 32],
    };
    let encoded = serialize_frame_without_hash(&frame)?;
    frame.frame_hash = compute_frame_hash(&encoded);
    Ok(frame)
}

#[cfg(feature = "baremetal")]
pub fn accept_or_quarantine(kernel: &mut Kernel, frame: &PhaseLinkFrame) -> FrameValidationResult {
    let ctx = ValidatorContext {
        equation_root: kernel.phaselink.equation_root,
        boot_session_root: kernel.phaselink.boot_session_root,
        trusted_nodes: kernel.phaselink.trusted_nodes.clone(),
        equation_policy: EquationRootPolicy::ExactMatch,
        gap_policy: SequenceGapPolicy::Quarantine,
        gateway: kernel.phaselink.gateway.clone(),
        remotes: kernel.phaselink.receiver.remotes.clone(),
    };
    let result = PhaseLinkValidator::validate(frame, &ctx);
    match &result {
        FrameValidationResult::Accepted => {
            kernel
                .phase_math
                .enqueue_proof(PHASELINK_STATECHAIN_OBJECT_ID, frame.sequence_number);
            kernel
                .phase_math
                .enqueue_merkle_leaf(PHASELINK_STATECHAIN_OBJECT_ID);
            kernel.phaselink.receiver.remotes.insert(
                frame.source_node,
                RemoteSequenceState {
                    source_node: frame.source_node,
                    boot_session_root: frame.boot_session_root,
                    last_sequence_number: frame.sequence_number,
                    last_frame_hash: frame.frame_hash,
                },
            );
            kernel.phaselink.accepted_frames += 1;
            let event = PhaseLinkEvent {
                kind: PhaseLinkEventKind::FrameAccepted,
                source_node: frame.source_node,
                destination_node: frame.destination_node,
                sequence_number: frame.sequence_number,
                frame_hash: frame.frame_hash,
                previous_frame_hash: frame.previous_frame_hash,
                payload_hash: frame.payload_hash,
                public_commitment: frame.public_commitment,
                operation: frame.operation.clone(),
                previous_phase: frame.previous_phase,
                next_phase: frame.next_phase,
                link_chain_head: frame.frame_hash,
                quarantine_reason: None,
            };
            kernel.phaselink.accepted_chain.push(event.clone());
            kernel.phaselink.push_leaf(&event, "accepted");
        }
        FrameValidationResult::Quarantined(reason) => {
            kernel.phaselink.quarantined_frames += 1;
            kernel
                .phaselink
                .quarantine
                .push(PhaseLinkValidator::quarantined_meta(frame, reason.clone()));
        }
        FrameValidationResult::Rejected(_) => {}
    }
    result
}

#[cfg(test)]
mod tests {
    use super::transport_label;
    use crate::kernel::phaselink::PhaseLinkController;

    #[test]
    fn transport_is_not_the_uart_console() {
        let label = transport_label();
        assert!(!label.contains("UART"));
        assert!(!label.contains("uart"));
        assert_eq!(label, "hosted loopback");
        let status = PhaseLinkController::new().status_lines();
        assert!(status.contains("transport: hosted loopback"));
        assert!(!status.contains("loopback/UART"));
        assert!(!status.contains("transport: PS UART"));
    }
}
