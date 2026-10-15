//! PhaseNet — native Kairos State network for Golden Phase Operating System.

pub mod addr;
pub mod firewall;
pub mod link;
pub mod packet;
pub mod sim;
pub mod stream;
pub mod uart_gem;
pub mod visualize;

pub use addr::{
    addr_for_uri, compute_phase_addr, parse_phase_uri, render_addr_short, PhaseUri, PHASE_ADDR_TAG,
};
pub use firewall::{
    evaluate_frame, render_failures, render_verdict, FirewallContext, FirewallFailure,
    FirewallResult, PhaseNetVerdict,
};
pub use link::{
    compute_frame_hash, decode_frame, encode_frame, FrameDecodeError, PayloadKind, PhaseLinkClock,
    PhaseLinkFrame, PHASE_LINK_FRAME_TAG, PHASE_LINK_MAGIC, PHASE_LINK_VERSION,
};
pub use packet::{decode_packet, encode_packet, PacketDecodeError};
pub use packet::{PhasePacket, PhasePacketKind, PHASE_PACKET_PROOF_TAG, PHASE_PACKET_TAG};
pub use sim::{PhaseNetSimBus, ReceivedSimFrame, SimError, SimNode};
pub use stream::{
    decode_bytes, encode_bytes, hex_decode, hex_encode, render_stream, render_stream_compact,
    HexDecodeError, PhaseStream,
};
pub use visualize::{
    render_frame_summary, render_packet_view, render_recv_line, render_status, render_uri_addr,
};

use crate::core::gphase_core::PhaseState;
use crate::kernel::authority::StateHash;
use crate::platform::Vec;
use crate::platform::{HashSet, String};
use crate::stateclock::StateClockSnapshot;

use crate::kernel::state_proof::ObjectKind;
use crate::kernel::state_root::SubsystemKind;

pub const PHASENET_STATECHAIN_OBJECT_ID: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseNetMode {
    Off,
    Loopback,
    Sim,
}

#[derive(Debug, Clone)]
pub struct ReceivedFrame {
    pub frame: link::PhaseLinkFrame,
    pub verdict: PhaseNetVerdict,
    pub failures: Vec<firewall::FirewallFailure>,
}

#[derive(Debug)]
pub struct PhaseNetController {
    pub mode: PhaseNetMode,
    pub node_name: String,
    pub node_root: StateHash,
    pub equation_root: StateHash,
    pub boot_session_root: StateHash,
    pub sequence_out: u64,
    pub sequence_in: u64,
    pub last_frame_hash: StateHash,
    pub trusted_peers: HashSet<StateHash>,
    pub rx_queue: Vec<ReceivedFrame>,
    pub sim_bus: Option<PhaseNetSimBus>,
}

impl Default for PhaseNetController {
    fn default() -> Self {
        Self::new()
    }
}

impl PhaseNetController {
    pub fn new() -> Self {
        Self {
            mode: PhaseNetMode::Off,
            node_name: "local".into(),
            node_root: [0u8; 32],
            equation_root: [0u8; 32],
            boot_session_root: [0u8; 32],
            sequence_out: 0,
            sequence_in: 0,
            last_frame_hash: [0u8; 32],
            trusted_peers: HashSet::new(),
            rx_queue: Vec::new(),
            sim_bus: None,
        }
    }

    pub fn init(
        &mut self,
        equation_root: StateHash,
        boot_session_root: StateHash,
        node_name: &str,
    ) {
        self.equation_root = equation_root;
        self.boot_session_root = boot_session_root;
        self.node_name = node_name.into();
        self.node_root =
            compute_phase_addr(&equation_root, &boot_session_root, "kairos", 0, 0, "node");
        self.trusted_peers.clear();
        self.trusted_peers.insert(self.node_root);
        self.mode = PhaseNetMode::Loopback;
        self.sequence_out = 0;
        self.sequence_in = 0;
        self.last_frame_hash = [0u8; 32];
        self.rx_queue.clear();
    }

    pub fn set_mode(&mut self, mode: PhaseNetMode) {
        self.mode = mode;
    }

    pub fn loopback_send_state_update(
        &mut self,
        dst_uri: &str,
        operation: &str,
        old_phase: PhaseState,
        new_phase: PhaseState,
        snapshot: StateClockSnapshot,
        statechain: &mut crate::kernel::statechain::StatechainLedger,
    ) -> Result<StateHash, PhaseNetError> {
        let uri = parse_phase_uri(dst_uri).ok_or(PhaseNetError::BadUri)?;
        let dst_addr = addr_for_uri(&self.equation_root, &self.node_root, &uri, 0, 0);
        let packet = packet::PhasePacket::state_update(
            self.equation_root,
            &self.node_name,
            &uri.domain,
            &uri.domain,
            operation,
            old_phase,
            new_phase,
            snapshot,
            self.last_frame_hash,
        );
        self.sequence_out += 1;
        let frame = link::PhaseLinkFrame::from_packet(
            self.node_root,
            dst_addr,
            self.boot_session_root,
            self.equation_root,
            0,
            self.sequence_out,
            link::PhaseLinkClock::from(&snapshot),
            self.last_frame_hash,
            &packet,
        );
        let frame_hash = frame.frame_hash;
        self.last_frame_hash = frame_hash;

        let ctx = firewall::FirewallContext {
            equation_root: self.equation_root,
            boot_session_root: self.boot_session_root,
            trusted_peer_addrs: self.trusted_peers.clone(),
            anom_protected: false,
        };
        let eval = firewall::evaluate_frame(&frame, &ctx, self.sequence_in, &[0u8; 32]);
        if eval.verdict == PhaseNetVerdict::Accept {
            self.sequence_in = frame.sequence;
        }
        self.rx_queue.push(ReceivedFrame {
            frame,
            verdict: eval.verdict,
            failures: eval.failures,
        });

        let payload = format!(
            "phasenet_loopback|{}|{}->{}|frame={}",
            uri.domain,
            phase_label(old_phase),
            phase_label(new_phase),
            render_addr_short(&frame_hash),
        );
        let _ = statechain.record_transition(
            ObjectKind::Driver,
            PHASENET_STATECHAIN_OBJECT_ID,
            SubsystemKind::Driver,
            new_phase,
            new_phase,
            "phasenet_loopback",
            None,
            payload.as_bytes(),
        );

        Ok(frame_hash)
    }

    pub fn loopback_recv(&mut self) -> Option<ReceivedFrame> {
        self.rx_queue.pop()
    }

    pub fn sim_init(&mut self, boot_b: StateHash) {
        let mut bus = sim::PhaseNetSimBus::default();
        bus.init_pair(self.equation_root, self.boot_session_root, boot_b);
        self.sim_bus = Some(bus);
        self.mode = PhaseNetMode::Sim;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sim_send(
        &mut self,
        from: &str,
        to: &str,
        uri: &str,
        operation: &str,
        old_phase: PhaseState,
        new_phase: PhaseState,
        snapshot: StateClockSnapshot,
    ) -> Result<StateHash, PhaseNetError> {
        let bus = self.sim_bus.as_mut().ok_or(PhaseNetError::SimInactive)?;
        bus.send_state_update(from, to, uri, operation, old_phase, new_phase, snapshot)
            .map_err(PhaseNetError::Sim)
    }

    pub fn sim_recv(&mut self, node: &str) -> Option<sim::ReceivedSimFrame> {
        self.sim_bus.as_mut()?.recv(node)
    }
}

fn phase_label(p: PhaseState) -> &'static str {
    p.kairos_label()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseNetError {
    BadUri,
    SimInactive,
    Sim(sim::SimError),
    HostedOnly,
}

impl PhaseNetError {
    pub fn reason(&self) -> &'static str {
        match self {
            PhaseNetError::BadUri => "invalid phase uri",
            PhaseNetError::SimInactive => "sim bus not initialized",
            PhaseNetError::Sim(e) => e.reason(),
            PhaseNetError::HostedOnly => "hosted-only",
        }
    }
}
