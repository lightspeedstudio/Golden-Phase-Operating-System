//! Accepted-event chain and receiver state.

use crate::core::gphase_core::PhaseState;
use crate::kernel::authority::StateHash;
use crate::kernel::phaselink::quarantine::QuarantineReason;
use crate::kernel::phaselink::types::{Hash, NodeId, RemoteSequenceState};
use crate::platform::{HashMap, String, Vec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseLinkEventKind {
    FrameCreated,
    FrameSent,
    FrameReceived,
    FrameAccepted,
    FrameQuarantined,
    FrameRejected,
    SequenceGap,
    ReplayDetected,
    RemoteBootSessionChanged,
    LoopbackCompleted,
    SimulationLinkCreated,
    SimulationLinkClosed,
    GatewayEnabled,
    GatewayDisabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseLinkEvent {
    pub kind: PhaseLinkEventKind,
    pub source_node: NodeId,
    pub destination_node: NodeId,
    pub sequence_number: u64,
    pub frame_hash: Hash,
    pub previous_frame_hash: Hash,
    pub payload_hash: Hash,
    pub public_commitment: Hash,
    pub operation: String,
    pub previous_phase: PhaseState,
    pub next_phase: PhaseState,
    pub link_chain_head: Hash,
    pub quarantine_reason: Option<QuarantineReason>,
}

#[derive(Debug, Default, Clone)]
pub struct PhaseLinkChain {
    pub events: Vec<PhaseLinkEvent>,
}

impl PhaseLinkChain {
    pub fn push(&mut self, event: PhaseLinkEvent) {
        self.events.push(event);
    }

    pub fn head(&self) -> StateHash {
        self.events
            .last()
            .map(|e| e.frame_hash)
            .unwrap_or([0u8; 32])
    }
}

#[derive(Debug, Default, Clone)]
pub struct PhaseLinkSenderState {
    pub sequence_number: u64,
    pub previous_frame_hash: StateHash,
}

#[derive(Debug, Default, Clone)]
pub struct PhaseLinkReceiverState {
    pub remotes: HashMap<NodeId, RemoteSequenceState>,
}
