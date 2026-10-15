//! PhasePacket — native PhaseNet state message (layer 3).

use crate::core::gphase_core::PhaseState;
use crate::kernel::authority::StateHash;
use crate::kernel::phasenet::stream::PhaseStream;
use crate::platform::{String, Vec};
use crate::stateclock::StateClockSnapshot;

use crate::kernel::hash::tagged_hash;

pub const PHASE_PACKET_TAG: &str = "KAIROS_PHASE_PACKET_V1";
pub const PHASE_PACKET_PROOF_TAG: &str = "KAIROS_PHASE_PACKET_PROOF_V1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PhasePacketKind {
    StateUpdate = 1,
    Stream = 2,
    Ping = 3,
    Pong = 4,
}

impl PhasePacketKind {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(PhasePacketKind::StateUpdate),
            2 => Some(PhasePacketKind::Stream),
            3 => Some(PhasePacketKind::Ping),
            4 => Some(PhasePacketKind::Pong),
            _ => None,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone)]
pub struct PhasePacket {
    pub kind: PhasePacketKind,
    pub equation_root: StateHash,
    pub src_node: String,
    pub dst_node: String,
    pub domain: String,
    pub operation: String,
    pub old_phase: PhaseState,
    pub new_phase: PhaseState,
    pub snapshot: StateClockSnapshot,
    pub proof_hash: StateHash,
    pub previous_hash: StateHash,
    pub payload_stream: PhaseStream,
    pub packet_hash: StateHash,
}

impl PhasePacket {
    #[allow(clippy::too_many_arguments)]
    pub fn state_update(
        equation_root: StateHash,
        src_node: &str,
        dst_node: &str,
        domain: &str,
        operation: &str,
        old_phase: PhaseState,
        new_phase: PhaseState,
        snapshot: StateClockSnapshot,
        previous_hash: StateHash,
    ) -> Self {
        let mut packet = Self {
            kind: PhasePacketKind::StateUpdate,
            equation_root,
            src_node: src_node.into(),
            dst_node: dst_node.into(),
            domain: domain.into(),
            operation: operation.into(),
            old_phase,
            new_phase,
            snapshot,
            proof_hash: [0u8; 32],
            previous_hash,
            payload_stream: Vec::new(),
            packet_hash: [0u8; 32],
        };
        {
            packet.proof_hash = compute_packet_proof_hash(&packet);
            packet.packet_hash = compute_packet_hash(&packet);
        }
        packet
    }

    pub fn verify(&self) -> bool {
        compute_packet_hash(self) == self.packet_hash
            && compute_packet_proof_hash(self) == self.proof_hash
    }
}

pub fn compute_packet_proof_hash(packet: &PhasePacket) -> StateHash {
    let mut buf = Vec::new();
    buf.extend_from_slice(&packet.equation_root);
    buf.extend_from_slice(packet.src_node.as_bytes());
    buf.extend_from_slice(packet.dst_node.as_bytes());
    buf.extend_from_slice(packet.domain.as_bytes());
    buf.extend_from_slice(packet.operation.as_bytes());
    buf.push(packet.old_phase.as_u8());
    buf.push(packet.new_phase.as_u8());
    buf.extend_from_slice(&packet.snapshot.proper_tick.to_le_bytes());
    buf.extend_from_slice(&packet.snapshot.kairos_tick.to_le_bytes());
    buf.push(packet.snapshot.sector_id);
    buf.extend_from_slice(&packet.snapshot.x.to_le_bytes());
    buf.extend_from_slice(&packet.previous_hash);
    for phase in &packet.payload_stream {
        buf.push(phase.as_u8());
    }
    tagged_hash(PHASE_PACKET_PROOF_TAG, &[&buf])
}

pub fn compute_packet_hash(packet: &PhasePacket) -> StateHash {
    let mut buf = Vec::new();
    buf.push(packet.kind.as_u8());
    buf.extend_from_slice(&packet.equation_root);
    buf.extend_from_slice(packet.src_node.as_bytes());
    buf.extend_from_slice(packet.dst_node.as_bytes());
    buf.extend_from_slice(packet.domain.as_bytes());
    buf.extend_from_slice(packet.operation.as_bytes());
    buf.push(packet.old_phase.as_u8());
    buf.push(packet.new_phase.as_u8());
    buf.extend_from_slice(&packet.proof_hash);
    buf.extend_from_slice(&packet.previous_hash);
    for phase in &packet.payload_stream {
        buf.push(phase.as_u8());
    }
    tagged_hash(PHASE_PACKET_TAG, &[&buf])
}

pub fn encode_packet(packet: &PhasePacket) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.push(packet.kind.as_u8());
    buf.extend_from_slice(&packet.equation_root);
    write_string(&mut buf, &packet.src_node);
    write_string(&mut buf, &packet.dst_node);
    write_string(&mut buf, &packet.domain);
    write_string(&mut buf, &packet.operation);
    buf.push(packet.old_phase.as_u8());
    buf.push(packet.new_phase.as_u8());
    write_snapshot(&mut buf, &packet.snapshot);
    buf.extend_from_slice(&packet.proof_hash);
    buf.extend_from_slice(&packet.previous_hash);
    buf.push(packet.payload_stream.len() as u8);
    for phase in &packet.payload_stream {
        buf.push(phase.as_u8());
    }
    buf.extend_from_slice(&packet.packet_hash);
    buf
}

pub fn decode_packet(data: &[u8]) -> Result<PhasePacket, PacketDecodeError> {
    let mut off = 0usize;
    let kind = read_u8(data, &mut off)
        .and_then(|b| PhasePacketKind::from_u8(b).ok_or(PacketDecodeError::BadKind))?;
    let equation_root = read_hash(data, &mut off)?;
    let src_node = read_string(data, &mut off)?;
    let dst_node = read_string(data, &mut off)?;
    let domain = read_string(data, &mut off)?;
    let operation = read_string(data, &mut off)?;
    let old_phase =
        PhaseState::from_u8(read_u8(data, &mut off)?).ok_or(PacketDecodeError::BadPhase)?;
    let new_phase =
        PhaseState::from_u8(read_u8(data, &mut off)?).ok_or(PacketDecodeError::BadPhase)?;
    let snapshot = read_snapshot(data, &mut off)?;
    let proof_hash = read_hash(data, &mut off)?;
    let previous_hash = read_hash(data, &mut off)?;
    let stream_len = read_u8(data, &mut off)? as usize;
    let mut payload_stream = Vec::with_capacity(stream_len);
    for _ in 0..stream_len {
        let p = PhaseState::from_u8(read_u8(data, &mut off)?).ok_or(PacketDecodeError::BadPhase)?;
        payload_stream.push(p);
    }
    let packet_hash = read_hash(data, &mut off)?;
    if off != data.len() {
        return Err(PacketDecodeError::Trailing);
    }
    let packet = PhasePacket {
        kind,
        equation_root,
        src_node,
        dst_node,
        domain,
        operation,
        old_phase,
        new_phase,
        snapshot,
        proof_hash,
        previous_hash,
        payload_stream,
        packet_hash,
    };
    if !packet.verify() {
        return Err(PacketDecodeError::BadHash);
    }
    Ok(packet)
}

fn write_string(buf: &mut Vec<u8>, s: &str) {
    let len = s.len().min(u16::MAX as usize) as u16;
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(&s.as_bytes()[..len as usize]);
}

fn read_string(data: &[u8], off: &mut usize) -> Result<String, PacketDecodeError> {
    let len = read_u16(data, off)? as usize;
    if *off + len > data.len() {
        return Err(PacketDecodeError::Truncated);
    }
    let s = core::str::from_utf8(&data[*off..*off + len]).map_err(|_| PacketDecodeError::Utf8)?;
    *off += len;
    Ok(s.into())
}

fn write_snapshot(buf: &mut Vec<u8>, snap: &StateClockSnapshot) {
    buf.extend_from_slice(&snap.proper_tick.to_le_bytes());
    buf.extend_from_slice(&snap.kairos_tick.to_le_bytes());
    buf.push(snap.sector_id);
    buf.extend_from_slice(&snap.x.to_le_bytes());
}

fn read_snapshot(data: &[u8], off: &mut usize) -> Result<StateClockSnapshot, PacketDecodeError> {
    let proper_tick = read_u64(data, off)?;
    let kairos_tick = read_u64(data, off)?;
    let sector_id = read_u8(data, off)?;
    let x = read_f64(data, off)?;
    Ok(StateClockSnapshot {
        chronos_tick: 0,
        kairos_tick,
        proper_tick,
        state_tick: 0,
        epoch: 0,
        phi: 0.0,
        delta_phi: 0.0,
        sector_id,
        epsilon: 0.0,
        x,
        dilation_factor: 1.0,
        skew: 0,
    })
}

fn read_u8(data: &[u8], off: &mut usize) -> Result<u8, PacketDecodeError> {
    if *off >= data.len() {
        return Err(PacketDecodeError::Truncated);
    }
    let v = data[*off];
    *off += 1;
    Ok(v)
}

fn read_u16(data: &[u8], off: &mut usize) -> Result<u16, PacketDecodeError> {
    if *off + 2 > data.len() {
        return Err(PacketDecodeError::Truncated);
    }
    let v = u16::from_le_bytes([data[*off], data[*off + 1]]);
    *off += 2;
    Ok(v)
}

fn read_u64(data: &[u8], off: &mut usize) -> Result<u64, PacketDecodeError> {
    if *off + 8 > data.len() {
        return Err(PacketDecodeError::Truncated);
    }
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&data[*off..*off + 8]);
    *off += 8;
    Ok(u64::from_le_bytes(bytes))
}

fn read_f64(data: &[u8], off: &mut usize) -> Result<f64, PacketDecodeError> {
    let bits = read_u64(data, off)?;
    Ok(f64::from_bits(bits))
}

fn read_hash(data: &[u8], off: &mut usize) -> Result<StateHash, PacketDecodeError> {
    if *off + 32 > data.len() {
        return Err(PacketDecodeError::Truncated);
    }
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&data[*off..*off + 32]);
    *off += 32;
    Ok(hash)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PacketDecodeError {
    Truncated,
    Trailing,
    BadKind,
    BadPhase,
    BadHash,
    Utf8,
}

impl PacketDecodeError {
    pub fn reason(&self) -> &'static str {
        match self {
            PacketDecodeError::Truncated => "truncated packet",
            PacketDecodeError::Trailing => "trailing packet bytes",
            PacketDecodeError::BadKind => "unknown packet kind",
            PacketDecodeError::BadPhase => "invalid phase byte",
            PacketDecodeError::BadHash => "packet hash mismatch",
            PacketDecodeError::Utf8 => "invalid utf-8",
        }
    }
}
