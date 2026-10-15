//! Local two-node PhaseNet simulator (hosted lab).

use crate::core::gphase_core::PhaseState;
use crate::kernel::authority::StateHash;
use crate::kernel::phasenet::addr::{addr_for_uri, parse_phase_uri};
use crate::kernel::phasenet::firewall::{evaluate_frame, FirewallContext, PhaseNetVerdict};
use crate::kernel::phasenet::link::{PhaseLinkClock, PhaseLinkFrame};
use crate::kernel::phasenet::packet::PhasePacket;
use crate::platform::{HashMap, HashSet, String, Vec};
use crate::stateclock::StateClockSnapshot;

#[derive(Debug, Clone)]
pub struct SimNode {
    pub name: String,
    pub node_root: StateHash,
    pub equation_root: StateHash,
    pub boot_session_root: StateHash,
    pub rx_queue: Vec<ReceivedSimFrame>,
    pub sequence_in: u64,
    pub sequence_out: u64,
    pub last_frame_hash: StateHash,
    pub trusted_peers: HashSet<StateHash>,
}

#[derive(Debug, Clone)]
pub struct ReceivedSimFrame {
    pub frame: PhaseLinkFrame,
    pub verdict: PhaseNetVerdict,
    pub from: String,
}

#[derive(Debug, Default)]
pub struct PhaseNetSimBus {
    pub nodes: HashMap<String, SimNode>,
}

impl PhaseNetSimBus {
    pub fn init_pair(&mut self, equation_root: StateHash, boot_a: StateHash, boot_b: StateHash) {
        self.nodes.clear();
        let root_a = crate::kernel::phasenet::addr::compute_phase_addr(
            &equation_root,
            &boot_a,
            "lab",
            0,
            1,
            "node",
        );
        let root_b = crate::kernel::phasenet::addr::compute_phase_addr(
            &equation_root,
            &boot_b,
            "lab",
            0,
            2,
            "node",
        );
        let mut trusted_a = HashSet::new();
        trusted_a.insert(root_b);
        let mut trusted_b = HashSet::new();
        trusted_b.insert(root_a);
        self.nodes.insert(
            "node-a".into(),
            SimNode {
                name: "node-a".into(),
                node_root: root_a,
                equation_root,
                boot_session_root: boot_a,
                rx_queue: Vec::new(),
                sequence_in: 0,
                sequence_out: 0,
                last_frame_hash: [0u8; 32],
                trusted_peers: trusted_a,
            },
        );
        self.nodes.insert(
            "node-b".into(),
            SimNode {
                name: "node-b".into(),
                node_root: root_b,
                equation_root,
                boot_session_root: boot_b,
                rx_queue: Vec::new(),
                sequence_in: 0,
                sequence_out: 0,
                last_frame_hash: [0u8; 32],
                trusted_peers: trusted_b,
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn send_state_update(
        &mut self,
        from: &str,
        to: &str,
        uri: &str,
        operation: &str,
        old_phase: PhaseState,
        new_phase: PhaseState,
        snapshot: StateClockSnapshot,
    ) -> Result<StateHash, SimError> {
        let src_node = self.nodes.get(from).ok_or(SimError::UnknownNode)?.clone();
        let dst_node = self.nodes.get(to).ok_or(SimError::UnknownNode)?;
        let uri = parse_phase_uri(uri).ok_or(SimError::BadUri)?;
        let dst_addr = addr_for_uri(&dst_node.equation_root, &dst_node.node_root, &uri, 0, 0);
        let packet = PhasePacket::state_update(
            src_node.equation_root,
            from,
            to,
            &uri.domain,
            operation,
            old_phase,
            new_phase,
            snapshot,
            src_node.last_frame_hash,
        );
        let sequence = src_node.sequence_out + 1;
        let frame = PhaseLinkFrame::from_packet(
            src_node.node_root,
            dst_addr,
            src_node.boot_session_root,
            src_node.equation_root,
            1,
            sequence,
            PhaseLinkClock::from(&snapshot),
            src_node.last_frame_hash,
            &packet,
        );
        let frame_hash = frame.frame_hash;
        if let Some(sender) = self.nodes.get_mut(from) {
            sender.sequence_out = sequence;
            sender.last_frame_hash = frame_hash;
        }
        self.deliver(to, from, frame)?;
        Ok(frame_hash)
    }

    fn deliver(&mut self, to: &str, from: &str, frame: PhaseLinkFrame) -> Result<(), SimError> {
        let receiver = self.nodes.get(to).ok_or(SimError::UnknownNode)?;
        let ctx = FirewallContext {
            equation_root: receiver.equation_root,
            boot_session_root: receiver.boot_session_root,
            trusted_peer_addrs: receiver.trusted_peers.clone(),
            anom_protected: false,
        };
        let result = evaluate_frame(
            &frame,
            &ctx,
            receiver.sequence_in,
            &receiver.last_frame_hash,
        );
        let verdict = result.verdict;
        let accepted = verdict == PhaseNetVerdict::Accept;
        if let Some(node) = self.nodes.get_mut(to) {
            if accepted {
                node.sequence_in = frame.sequence;
                node.last_frame_hash = frame.frame_hash;
            }
            node.rx_queue.push(ReceivedSimFrame {
                frame,
                verdict,
                from: from.into(),
            });
        }
        Ok(())
    }

    pub fn recv(&mut self, node: &str) -> Option<ReceivedSimFrame> {
        self.nodes.get_mut(node)?.rx_queue.pop()
    }

    pub fn pending(&self, node: &str) -> usize {
        self.nodes.get(node).map(|n| n.rx_queue.len()).unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimError {
    UnknownNode,
    BadUri,
}

impl SimError {
    pub fn reason(&self) -> &'static str {
        match self {
            SimError::UnknownNode => "unknown sim node",
            SimError::BadUri => "invalid phase uri",
        }
    }
}
