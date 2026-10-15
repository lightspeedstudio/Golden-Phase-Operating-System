//! Logical packet envelope + GPL transport phase.

use crate::core::gphase_core::{compose, PhaseState};
use crate::kernel::hash::{tagged_hash, StateHash};
use crate::kernel::phase_transport::error::TransportError;
use crate::platform::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketOpcode {
    Data,
    Ack,
    Nack,
    Route,
    Heartbeat,
}

impl PacketOpcode {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Data => 1,
            Self::Ack => 2,
            Self::Nack => 3,
            Self::Route => 4,
            Self::Heartbeat => 5,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "data" | "DATA" => Some(Self::Data),
            "ack" | "ACK" => Some(Self::Ack),
            "nack" | "NACK" => Some(Self::Nack),
            "route" | "ROUTE" => Some(Self::Route),
            "heartbeat" | "HEARTBEAT" => Some(Self::Heartbeat),
            _ => None,
        }
    }
}

/// Transport lifecycle phases (orthogonal to fault flags).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportPhase {
    Empty = 0,
    Queued = 1,
    InFlight = 2,
    Received = 3,
}

impl TransportPhase {
    pub fn as_phase(self) -> PhaseState {
        PhaseState::from_u8(self as u8).unwrap()
    }

    pub fn from_phase(p: PhaseState) -> Option<Self> {
        match p {
            PhaseState::Phi0 => Some(Self::Empty),
            PhaseState::Phi1 => Some(Self::Queued),
            PhaseState::Phi2 => Some(Self::InFlight),
            PhaseState::Phi3 => Some(Self::Received),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransportStatus {
    pub phase: TransportPhase,
    pub quarantined: bool,
    pub error: Option<TransportError>,
}

#[derive(Debug, Clone)]
pub struct PhasePacket {
    pub packet_id: u64,
    pub source: (usize, usize),
    pub destination: (usize, usize),
    pub opcode: PacketOpcode,
    pub payload: Vec<u8>,
    pub sequence: u64,
    pub ttl: u64,
    pub parent_packet_id: Option<u64>,
    pub episode_id: u64,
    pub checksum: StateHash,
    pub created_tick: u64,
    pub status: TransportStatus,
    pub injection_hash: Option<StateHash>,
    pub lane_id: u32,
}

impl PhasePacket {
    pub fn new(
        packet_id: u64,
        source: (usize, usize),
        destination: (usize, usize),
        opcode: PacketOpcode,
        payload: &[u8],
        created_tick: u64,
    ) -> Self {
        let mut pkt = Self {
            packet_id,
            source,
            destination,
            opcode,
            payload: payload.to_vec(),
            sequence: 0,
            ttl: 64,
            parent_packet_id: None,
            episode_id: packet_id,
            checksum: [0u8; 32],
            created_tick,
            status: TransportStatus {
                phase: TransportPhase::Empty,
                quarantined: false,
                error: None,
            },
            injection_hash: None,
            lane_id: 0,
        };
        pkt.checksum = pkt.compute_checksum();
        pkt
    }

    pub fn header_bytes(&self) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&self.packet_id.to_le_bytes());
        v.extend_from_slice(&(self.source.0 as u32).to_le_bytes());
        v.extend_from_slice(&(self.source.1 as u32).to_le_bytes());
        v.extend_from_slice(&(self.destination.0 as u32).to_le_bytes());
        v.extend_from_slice(&(self.destination.1 as u32).to_le_bytes());
        v.push(self.opcode.as_u8());
        v.extend_from_slice(&self.sequence.to_le_bytes());
        v.extend_from_slice(&self.ttl.to_le_bytes());
        v.extend_from_slice(&self.payload);
        v
    }

    /// Immutable fields for integrity (excludes in-flight TTL).
    pub fn checksum_bytes(&self) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&self.packet_id.to_le_bytes());
        v.extend_from_slice(&(self.source.0 as u32).to_le_bytes());
        v.extend_from_slice(&(self.source.1 as u32).to_le_bytes());
        v.extend_from_slice(&(self.destination.0 as u32).to_le_bytes());
        v.extend_from_slice(&(self.destination.1 as u32).to_le_bytes());
        v.push(self.opcode.as_u8());
        v.extend_from_slice(&self.sequence.to_le_bytes());
        v.extend_from_slice(&self.payload);
        if let Some(parent) = self.parent_packet_id {
            v.extend_from_slice(&parent.to_le_bytes());
        }
        v.extend_from_slice(&self.episode_id.to_le_bytes());
        v
    }

    pub fn compute_checksum(&self) -> StateHash {
        tagged_hash("GPL_TRANSPORT_PKT_V1", &[&self.checksum_bytes()])
    }

    pub fn verify_checksum(&self) -> Result<(), TransportError> {
        if self.compute_checksum() != self.checksum {
            return Err(TransportError::BadChecksum);
        }
        Ok(())
    }

    /// Advance transport phase using canonical compose; caller records statechain.
    pub fn set_phase<F>(
        &mut self,
        next: TransportPhase,
        record: &mut F,
    ) -> Result<(), TransportError>
    where
        F: FnMut(
            PhaseState,
            PhaseState,
            &str,
            PhaseState,
            PhaseState,
            &[u8],
        ) -> Result<(), TransportError>,
    {
        let prev = self.status.phase.as_phase();
        let next_p = next.as_phase();
        let left = prev;
        let right = next_p;
        let _composed = compose(left, right);
        let op = match next {
            TransportPhase::Empty => "packet_complete",
            TransportPhase::Queued => "packet_queue",
            TransportPhase::InFlight => "glider_inject",
            TransportPhase::Received => "glider_receive",
        };
        record(prev, next_p, op, left, right, &self.header_bytes())?;
        self.status.phase = next;
        Ok(())
    }
}
