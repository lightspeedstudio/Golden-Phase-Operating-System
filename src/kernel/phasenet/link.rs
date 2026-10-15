//! PhaseLink — frame format for phase packets over a carrier.

use crate::kernel::authority::StateHash;
use crate::kernel::phasenet::packet::{decode_packet, encode_packet, PhasePacket};
use crate::platform::Vec;
use crate::stateclock::StateClockSnapshot;

use crate::kernel::hash::tagged_hash;

pub const PHASE_LINK_MAGIC: &[u8; 4] = b"KPHN";
pub const PHASE_LINK_VERSION: u8 = 1;
pub const PHASE_LINK_FRAME_TAG: &str = "KAIROS_PHASE_LINK_FRAME_V1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PayloadKind {
    PhasePacket = 1,
    RawStream = 2,
    Ping = 3,
}

impl PayloadKind {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(PayloadKind::PhasePacket),
            2 => Some(PayloadKind::RawStream),
            3 => Some(PayloadKind::Ping),
            _ => None,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseLinkClock {
    pub proper_tick: u64,
    pub kairos_tick: u64,
    pub sector_id: u8,
    pub x: f64,
}

impl From<StateClockSnapshot> for PhaseLinkClock {
    fn from(s: StateClockSnapshot) -> Self {
        Self::from(&s)
    }
}

impl From<&StateClockSnapshot> for PhaseLinkClock {
    fn from(s: &StateClockSnapshot) -> Self {
        Self {
            proper_tick: s.proper_tick,
            kairos_tick: s.kairos_tick,
            sector_id: s.sector_id,
            x: s.x,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PhaseLinkFrame {
    pub version: u8,
    pub src_addr: StateHash,
    pub dst_addr: StateHash,
    pub boot_session_root: StateHash,
    pub equation_root: StateHash,
    pub channel: u32,
    pub sequence: u64,
    pub clock: PhaseLinkClock,
    pub payload_kind: PayloadKind,
    pub payload: Vec<u8>,
    pub proof_hash: StateHash,
    pub previous_frame_hash: StateHash,
    pub frame_hash: StateHash,
}

impl PhaseLinkFrame {
    #[allow(clippy::too_many_arguments)]
    pub fn from_packet(
        src_addr: StateHash,
        dst_addr: StateHash,
        boot_session_root: StateHash,
        equation_root: StateHash,
        channel: u32,
        sequence: u64,
        clock: PhaseLinkClock,
        previous_frame_hash: StateHash,
        packet: &PhasePacket,
    ) -> Self {
        let payload = encode_packet(packet);
        let mut frame = Self {
            version: PHASE_LINK_VERSION,
            src_addr,
            dst_addr,
            boot_session_root,
            equation_root,
            channel,
            sequence,
            clock,
            payload_kind: PayloadKind::PhasePacket,
            payload,
            proof_hash: packet.proof_hash,
            previous_frame_hash,
            frame_hash: [0u8; 32],
        };
        frame.frame_hash = compute_frame_hash(&frame);
        frame
    }

    pub fn packet(
        &self,
    ) -> Result<PhasePacket, crate::kernel::phasenet::packet::PacketDecodeError> {
        if self.payload_kind != PayloadKind::PhasePacket {
            return Err(crate::kernel::phasenet::packet::PacketDecodeError::BadKind);
        }
        decode_packet(&self.payload)
    }

    pub fn verify_frame_hash(&self) -> bool {
        compute_frame_hash(self) == self.frame_hash
    }
}

pub fn compute_frame_hash(frame: &PhaseLinkFrame) -> StateHash {
    let body = encode_frame_body(frame);
    tagged_hash(PHASE_LINK_FRAME_TAG, &[&body])
}

pub fn encode_frame(frame: &PhaseLinkFrame) -> Vec<u8> {
    let mut out = encode_frame_body(frame);
    out.extend_from_slice(&frame.frame_hash);
    out
}

fn encode_frame_body(frame: &PhaseLinkFrame) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(PHASE_LINK_MAGIC);
    buf.push(frame.version);
    buf.extend_from_slice(&frame.src_addr);
    buf.extend_from_slice(&frame.dst_addr);
    buf.extend_from_slice(&frame.boot_session_root);
    buf.extend_from_slice(&frame.equation_root);
    buf.extend_from_slice(&frame.channel.to_le_bytes());
    buf.extend_from_slice(&frame.sequence.to_le_bytes());
    buf.extend_from_slice(&frame.clock.proper_tick.to_le_bytes());
    buf.extend_from_slice(&frame.clock.kairos_tick.to_le_bytes());
    buf.push(frame.clock.sector_id);
    buf.extend_from_slice(&frame.clock.x.to_le_bytes());
    buf.push(frame.payload_kind.as_u8());
    buf.extend_from_slice(&(frame.payload.len() as u32).to_le_bytes());
    buf.extend_from_slice(&frame.payload);
    buf.extend_from_slice(&frame.proof_hash);
    buf.extend_from_slice(&frame.previous_frame_hash);
    buf
}

pub fn decode_frame(data: &[u8]) -> Result<PhaseLinkFrame, FrameDecodeError> {
    if data.len() < 4 + 1 + 32 * 4 + 4 + 8 + 8 + 8 + 1 + 8 + 1 + 4 + 32 + 32 + 32 {
        return Err(FrameDecodeError::Truncated);
    }
    let mut off = 0usize;
    if &data[off..off + 4] != PHASE_LINK_MAGIC {
        return Err(FrameDecodeError::BadMagic);
    }
    off += 4;
    let version = data[off];
    off += 1;
    if version != PHASE_LINK_VERSION {
        return Err(FrameDecodeError::BadVersion);
    }
    let src_addr = read_hash(data, &mut off)?;
    let dst_addr = read_hash(data, &mut off)?;
    let boot_session_root = read_hash(data, &mut off)?;
    let equation_root = read_hash(data, &mut off)?;
    let channel = read_u32(data, &mut off)?;
    let sequence = read_u64(data, &mut off)?;
    let proper_tick = read_u64(data, &mut off)?;
    let kairos_tick = read_u64(data, &mut off)?;
    let sector_id = read_u8(data, &mut off)?;
    let x = read_f64(data, &mut off)?;
    let payload_kind =
        PayloadKind::from_u8(read_u8(data, &mut off)?).ok_or(FrameDecodeError::BadPayloadKind)?;
    let payload_len = read_u32(data, &mut off)? as usize;
    if off + payload_len + 32 + 32 + 32 > data.len() {
        return Err(FrameDecodeError::Truncated);
    }
    let payload = data[off..off + payload_len].to_vec();
    off += payload_len;
    let proof_hash = read_hash(data, &mut off)?;
    let previous_frame_hash = read_hash(data, &mut off)?;
    let frame_hash = read_hash(data, &mut off)?;
    if off != data.len() {
        return Err(FrameDecodeError::Trailing);
    }
    let frame = PhaseLinkFrame {
        version,
        src_addr,
        dst_addr,
        boot_session_root,
        equation_root,
        channel,
        sequence,
        clock: PhaseLinkClock {
            proper_tick,
            kairos_tick,
            sector_id,
            x,
        },
        payload_kind,
        payload,
        proof_hash,
        previous_frame_hash,
        frame_hash,
    };
    if !frame.verify_frame_hash() {
        return Err(FrameDecodeError::BadHash);
    }
    Ok(frame)
}

fn read_u8(data: &[u8], off: &mut usize) -> Result<u8, FrameDecodeError> {
    if *off >= data.len() {
        return Err(FrameDecodeError::Truncated);
    }
    let v = data[*off];
    *off += 1;
    Ok(v)
}

fn read_u32(data: &[u8], off: &mut usize) -> Result<u32, FrameDecodeError> {
    if *off + 4 > data.len() {
        return Err(FrameDecodeError::Truncated);
    }
    let v = u32::from_le_bytes([data[*off], data[*off + 1], data[*off + 2], data[*off + 3]]);
    *off += 4;
    Ok(v)
}

fn read_u64(data: &[u8], off: &mut usize) -> Result<u64, FrameDecodeError> {
    if *off + 8 > data.len() {
        return Err(FrameDecodeError::Truncated);
    }
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&data[*off..*off + 8]);
    *off += 8;
    Ok(u64::from_le_bytes(bytes))
}

fn read_f64(data: &[u8], off: &mut usize) -> Result<f64, FrameDecodeError> {
    Ok(f64::from_bits(read_u64(data, off)?))
}

fn read_hash(data: &[u8], off: &mut usize) -> Result<StateHash, FrameDecodeError> {
    if *off + 32 > data.len() {
        return Err(FrameDecodeError::Truncated);
    }
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&data[*off..*off + 32]);
    *off += 32;
    Ok(hash)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameDecodeError {
    Truncated,
    Trailing,
    BadMagic,
    BadVersion,
    BadPayloadKind,
    BadHash,
}

impl FrameDecodeError {
    pub fn reason(&self) -> &'static str {
        match self {
            FrameDecodeError::Truncated => "truncated frame",
            FrameDecodeError::Trailing => "trailing frame bytes",
            FrameDecodeError::BadMagic => "bad PhaseLink magic",
            FrameDecodeError::BadVersion => "unsupported PhaseLink version",
            FrameDecodeError::BadPayloadKind => "unknown payload kind",
            FrameDecodeError::BadHash => "frame hash mismatch",
        }
    }
}
