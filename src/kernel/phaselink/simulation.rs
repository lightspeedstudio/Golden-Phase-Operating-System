//! Two-node hosted simulation.

use crate::kernel::authority::StateHash;
use crate::kernel::hash::tagged_hash;
use crate::kernel::phaselink::chain::{
    PhaseLinkChain, PhaseLinkReceiverState, PhaseLinkSenderState,
};
use crate::kernel::phaselink::types::{NodeId, PHASELINK_MAX_QUEUE};
use crate::platform::{String, Vec};

#[derive(Debug, Clone)]
pub struct SimulatedPhaseNode {
    pub node_id: NodeId,
    pub boot_session_root: StateHash,
    pub sender: PhaseLinkSenderState,
    pub receiver: PhaseLinkReceiverState,
    pub chain: PhaseLinkChain,
}

#[derive(Debug, Clone, Default)]
pub struct SimulatedWire {
    pub queue: Vec<(String, String, Vec<u8>)>,
    pub last_sent: Option<(String, String, Vec<u8>)>,
}

#[derive(Debug, Clone)]
pub struct TwoNodeSimulation {
    pub node_a: SimulatedPhaseNode,
    pub node_b: SimulatedPhaseNode,
    pub transport: SimulatedWire,
    pub pending_tamper: Option<String>,
}

impl TwoNodeSimulation {
    pub fn new(equation_root: StateHash, boot_a: StateHash, boot_b: StateHash) -> Self {
        Self {
            node_a: SimulatedPhaseNode::new("node-a", equation_root, boot_a),
            node_b: SimulatedPhaseNode::new("node-b", equation_root, boot_b),
            transport: SimulatedWire::default(),
            pending_tamper: None,
        }
    }

    pub fn node(&self, name: &str) -> Option<&SimulatedPhaseNode> {
        match name {
            "node-a" => Some(&self.node_a),
            "node-b" => Some(&self.node_b),
            _ => None,
        }
    }

    pub fn node_mut(&mut self, name: &str) -> Option<&mut SimulatedPhaseNode> {
        match name {
            "node-a" => Some(&mut self.node_a),
            "node-b" => Some(&mut self.node_b),
            _ => None,
        }
    }

    pub fn enqueue(&mut self, from: &str, to: &str, mut bytes: Vec<u8>) {
        if let Some(tamper) = self.pending_tamper.take() {
            apply_tamper(&mut bytes, &tamper);
        }
        if self.transport.queue.len() >= PHASELINK_MAX_QUEUE {
            self.transport.queue.remove(0);
        }
        self.transport.last_sent = Some((from.into(), to.into(), bytes.clone()));
        self.transport.queue.push((from.into(), to.into(), bytes));
    }

    pub fn recv(&mut self, node: &str) -> Option<(String, Vec<u8>)> {
        let index = self
            .transport
            .queue
            .iter()
            .position(|(_, to, _)| to == node)?;
        let (from, _to, bytes) = self.transport.queue.remove(index);
        Some((from, bytes))
    }

    pub fn replay_last(&mut self) -> bool {
        if let Some((from, to, bytes)) = self.transport.last_sent.clone() {
            self.enqueue(&from, &to, bytes);
            true
        } else {
            false
        }
    }

    pub fn reboot(&mut self, node: &str) -> bool {
        if let Some(node_ref) = self.node_mut(node) {
            node_ref.boot_session_root = tagged_hash(
                "KAIROS_PHASELINK_SIM_REBOOT_V1",
                &[&node_ref.boot_session_root],
            );
            node_ref.receiver.remotes.clear();
            node_ref.sender.previous_frame_hash = [0u8; 32];
            node_ref.sender.sequence_number = 0;
            true
        } else {
            false
        }
    }
}

impl SimulatedPhaseNode {
    fn new(name: &str, equation_root: StateHash, boot_session_root: StateHash) -> Self {
        Self {
            node_id: tagged_hash(
                "KAIROS_PHASELINK_SIM_NODE_V1",
                &[name.as_bytes(), &equation_root, &boot_session_root],
            ),
            boot_session_root,
            sender: PhaseLinkSenderState::default(),
            receiver: PhaseLinkReceiverState::default(),
            chain: PhaseLinkChain::default(),
        }
    }
}

fn apply_tamper(bytes: &mut [u8], field: &str) {
    match field {
        "payload" if !bytes.is_empty() => {
            let idx = bytes.len().saturating_sub(40);
            bytes[idx] ^= 0x5A;
        }
        "sequence" if bytes.len() > 89 => bytes[88] ^= 1,
        "previous-hash" if bytes.len() > 128 => bytes[120] ^= 1,
        "equation-root" if bytes.len() > 80 => bytes[45] ^= 1,
        "boot-session" if bytes.len() > 120 => bytes[77] ^= 1,
        "proof" if bytes.len() > 160 => bytes[150] ^= 1,
        "next-phase" if bytes.len() > 120 => bytes[119] ^= 1,
        "truncate" if !bytes.is_empty() => {
            let new_len = bytes.len().saturating_sub(5);
            bytes.iter_mut().skip(new_len).for_each(|b| *b = 0);
        }
        _ => {}
    }
}
