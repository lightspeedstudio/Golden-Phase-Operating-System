//! GPLV1 binary layout constants and record types.

use sha2::{Digest, Sha256};

pub const MAGIC: [u8; 4] = *b"GPLV";
pub const FORMAT_VERSION: u16 = 1;
pub const HEADER_SIZE: u16 = 32;
pub const RECORD_SIZE: u16 = 48;
pub const TOTAL_RECORDS: u32 = 84;
pub const COMPOSE_COUNT: u32 = 16;
pub const REFLECT_COUNT: u32 = 4;
pub const TRIPLE_COUNT: u32 = 64;
pub const ENCODING_VERSION: u32 = 1;

pub const ANCESTOR_ICE40_BITSTREAM_SHA256: &str =
    "f499032615d2284b90fccfcf98207fae4f2dd2617868eba68549cc3102ae6dfc";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RecordKind {
    Compose = 1,
    Reflection = 2,
    Triple = 3,
}

impl RecordKind {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Compose),
            2 => Some(Self::Reflection),
            3 => Some(Self::Triple),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordFlags(pub u8);

impl RecordFlags {
    pub const COMPOSE_AB: u8 = 1 << 0;
    pub const LEFT_RIGHT_PARITY: u8 = 1 << 1;
    pub const RHO_A: u8 = 1 << 2;

    pub fn bits(self) -> u8 {
        self.0
    }

    pub fn has(self, bit: u8) -> bool {
        self.0 & bit != 0
    }

    pub fn is_supported(self) -> bool {
        self.0 & !(Self::COMPOSE_AB | Self::LEFT_RIGHT_PARITY | Self::RHO_A) == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleHeader {
    pub format_version: u16,
    pub header_size: u16,
    pub record_size: u16,
    pub record_count: u32,
    pub compose_count: u32,
    pub reflect_count: u32,
    pub triple_count: u32,
    pub encoding_version: u32,
}

impl OracleHeader {
    pub fn canonical() -> Self {
        Self {
            format_version: FORMAT_VERSION,
            header_size: HEADER_SIZE,
            record_size: RECORD_SIZE,
            record_count: TOTAL_RECORDS,
            compose_count: COMPOSE_COUNT,
            reflect_count: REFLECT_COUNT,
            triple_count: TRIPLE_COUNT,
            encoding_version: ENCODING_VERSION,
        }
    }

    pub fn encode(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[0..4].copy_from_slice(&MAGIC);
        out[4..6].copy_from_slice(&self.format_version.to_le_bytes());
        out[6..8].copy_from_slice(&self.header_size.to_le_bytes());
        out[8..10].copy_from_slice(&self.record_size.to_le_bytes());
        // 0x0A..0x0C reserved = 0
        out[12..16].copy_from_slice(&self.record_count.to_le_bytes());
        out[16..20].copy_from_slice(&self.compose_count.to_le_bytes());
        out[20..24].copy_from_slice(&self.reflect_count.to_le_bytes());
        out[24..28].copy_from_slice(&self.triple_count.to_le_bytes());
        out[28..32].copy_from_slice(&self.encoding_version.to_le_bytes());
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleRecord {
    pub kind: RecordKind,
    pub flags: RecordFlags,
    pub sequence: u32,
    pub input_a: u8,
    pub input_b: u8,
    pub input_c: u8,
    pub compose_ab: u8,
    pub left_association: u8,
    pub right_association: u8,
    pub parity: u8,
    pub rho_a: u8,
    pub expected_digest: [u8; 32],
}

impl OracleRecord {
    pub fn encode_prefix(&self) -> [u8; 16] {
        let mut p = [0u8; 16];
        p[0] = self.kind as u8;
        p[1] = self.flags.bits();
        // p[2..4] reserved
        p[4..8].copy_from_slice(&self.sequence.to_le_bytes());
        p[8] = self.input_a;
        p[9] = self.input_b;
        p[10] = self.input_c;
        p[11] = self.compose_ab;
        p[12] = self.left_association;
        p[13] = self.right_association;
        p[14] = self.parity;
        p[15] = self.rho_a;
        p
    }

    pub fn compute_digest(prefix: &[u8; 16]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(prefix);
        let out = hasher.finalize();
        let mut dig = [0u8; 32];
        dig.copy_from_slice(&out);
        dig
    }

    pub fn encode(&self) -> [u8; 48] {
        let mut out = [0u8; 48];
        let prefix = self.encode_prefix();
        out[0..16].copy_from_slice(&prefix);
        out[16..48].copy_from_slice(&self.expected_digest);
        out
    }

    pub fn with_digest(mut self) -> Self {
        let prefix = self.encode_prefix();
        self.expected_digest = Self::compute_digest(&prefix);
        self
    }
}

/// Digital encoding Φn ↔ 2-bit wire value (identity for 0..3).
pub fn encode_phase_bits(state: u8) -> Option<u8> {
    if state <= 3 {
        Some(state)
    } else {
        None
    }
}

pub fn phase_from_bits(bits: u8) -> Option<u8> {
    encode_phase_bits(bits)
}

/// One-hot diagnostic: Φ0→0001, Φ1→0010, Φ2→0100, Φ3→1000.
pub fn one_hot(state: u8) -> Option<u8> {
    match state {
        0 => Some(0b0001),
        1 => Some(0b0010),
        2 => Some(0b0100),
        3 => Some(0b1000),
        _ => None,
    }
}

pub fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let out = hasher.finalize();
    let mut dig = [0u8; 32];
    dig.copy_from_slice(&out);
    dig
}
