//! Decode and validate GPLV1 oracle blobs.

use std::fs;
use std::io;
use std::path::Path;

use super::format::{
    OracleHeader, OracleRecord, RecordFlags, RecordKind, COMPOSE_COUNT, FORMAT_VERSION,
    HEADER_SIZE, MAGIC, RECORD_SIZE, REFLECT_COUNT, TOTAL_RECORDS, TRIPLE_COUNT,
};

#[derive(Debug)]
pub enum OracleDecodeError {
    Io(io::Error),
    Truncated,
    ExtraTrailingBytes,
    BadMagic,
    UnsupportedVersion(u16),
    BadHeaderSize(u16),
    BadRecordSize(u16),
    BadCounts,
    NonzeroReserved,
    UnknownKind(u8),
    UnsupportedFlags(u8),
    StateOutOfRange { field: &'static str, value: u8 },
    IllegalParity(u8),
    DigestMismatch { sequence: u32 },
    FlagFieldMismatch { sequence: u32, msg: String },
}

impl From<io::Error> for OracleDecodeError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl std::fmt::Display for OracleDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io: {e}"),
            Self::Truncated => write!(f, "truncated vectors.bin"),
            Self::ExtraTrailingBytes => write!(f, "extra trailing bytes"),
            Self::BadMagic => write!(f, "unknown magic"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported format version {v}"),
            Self::BadHeaderSize(v) => write!(f, "incorrect header size {v}"),
            Self::BadRecordSize(v) => write!(f, "incorrect record size {v}"),
            Self::BadCounts => write!(f, "incorrect record counts"),
            Self::NonzeroReserved => write!(f, "nonzero reserved field"),
            Self::UnknownKind(k) => write!(f, "unknown record kind {k}"),
            Self::UnsupportedFlags(fl) => write!(f, "unsupported record flags 0x{fl:02x}"),
            Self::StateOutOfRange { field, value } => {
                write!(f, "state out of range {field}={value}")
            }
            Self::IllegalParity(p) => write!(f, "illegal parity {p}"),
            Self::DigestMismatch { sequence } => {
                write!(f, "digest mismatch sequence={sequence}")
            }
            Self::FlagFieldMismatch { sequence, msg } => {
                write!(f, "flag/field mismatch sequence={sequence}: {msg}")
            }
        }
    }
}

impl std::error::Error for OracleDecodeError {}

fn read_u16(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

fn read_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

fn check_state(field: &'static str, v: u8) -> Result<(), OracleDecodeError> {
    if v > 3 {
        Err(OracleDecodeError::StateOutOfRange { field, value: v })
    } else {
        Ok(())
    }
}

pub fn decode_oracle(blob: &[u8]) -> Result<(OracleHeader, Vec<OracleRecord>), OracleDecodeError> {
    if blob.len() < HEADER_SIZE as usize {
        return Err(OracleDecodeError::Truncated);
    }
    if blob[0..4] != MAGIC {
        return Err(OracleDecodeError::BadMagic);
    }
    let format_version = read_u16(blob, 4);
    if format_version != FORMAT_VERSION {
        return Err(OracleDecodeError::UnsupportedVersion(format_version));
    }
    let header_size = read_u16(blob, 6);
    if header_size != HEADER_SIZE {
        return Err(OracleDecodeError::BadHeaderSize(header_size));
    }
    let record_size = read_u16(blob, 8);
    if record_size != RECORD_SIZE {
        return Err(OracleDecodeError::BadRecordSize(record_size));
    }
    if read_u16(blob, 10) != 0 {
        return Err(OracleDecodeError::NonzeroReserved);
    }
    let record_count = read_u32(blob, 12);
    let compose_count = read_u32(blob, 16);
    let reflect_count = read_u32(blob, 20);
    let triple_count = read_u32(blob, 24);
    let encoding_version = read_u32(blob, 28);
    if record_count != TOTAL_RECORDS
        || compose_count != COMPOSE_COUNT
        || reflect_count != REFLECT_COUNT
        || triple_count != TRIPLE_COUNT
        || encoding_version != 1
    {
        return Err(OracleDecodeError::BadCounts);
    }

    let expected_len = HEADER_SIZE as usize + RECORD_SIZE as usize * record_count as usize;
    if blob.len() < expected_len {
        return Err(OracleDecodeError::Truncated);
    }
    if blob.len() > expected_len {
        return Err(OracleDecodeError::ExtraTrailingBytes);
    }

    let header = OracleHeader {
        format_version,
        header_size,
        record_size,
        record_count,
        compose_count,
        reflect_count,
        triple_count,
        encoding_version,
    };

    let mut records = Vec::with_capacity(record_count as usize);
    let mut seen_compose = 0u32;
    let mut seen_reflect = 0u32;
    let mut seen_triple = 0u32;

    for i in 0..record_count as usize {
        let off = HEADER_SIZE as usize + i * RECORD_SIZE as usize;
        let rec = &blob[off..off + RECORD_SIZE as usize];
        let kind_u = rec[0];
        let kind = RecordKind::from_u8(kind_u).ok_or(OracleDecodeError::UnknownKind(kind_u))?;
        let flags = RecordFlags(rec[1]);
        if !flags.is_supported() {
            return Err(OracleDecodeError::UnsupportedFlags(flags.bits()));
        }
        if read_u16(rec, 2) != 0 {
            return Err(OracleDecodeError::NonzeroReserved);
        }
        let sequence = read_u32(rec, 4);
        let input_a = rec[8];
        let input_b = rec[9];
        let input_c = rec[10];
        let compose_ab = rec[11];
        let left_association = rec[12];
        let right_association = rec[13];
        let parity = rec[14];
        let rho_a = rec[15];
        let mut expected_digest = [0u8; 32];
        expected_digest.copy_from_slice(&rec[16..48]);

        check_state("input_a", input_a)?;
        check_state("input_b", input_b)?;
        check_state("input_c", input_c)?;

        match kind {
            RecordKind::Compose => {
                if !flags.has(RecordFlags::COMPOSE_AB) || !flags.has(RecordFlags::RHO_A) {
                    return Err(OracleDecodeError::FlagFieldMismatch {
                        sequence,
                        msg: "compose requires bits 0 and 2".into(),
                    });
                }
                if flags.has(RecordFlags::LEFT_RIGHT_PARITY) {
                    return Err(OracleDecodeError::FlagFieldMismatch {
                        sequence,
                        msg: "compose must not set bit 1".into(),
                    });
                }
                check_state("compose_ab", compose_ab)?;
                check_state("rho_a", rho_a)?;
                if left_association != 0 || right_association != 0 || parity != 0 {
                    return Err(OracleDecodeError::FlagFieldMismatch {
                        sequence,
                        msg: "compose unused result fields must be zero".into(),
                    });
                }
                seen_compose += 1;
            }
            RecordKind::Reflection => {
                if flags.bits() != RecordFlags::RHO_A {
                    return Err(OracleDecodeError::FlagFieldMismatch {
                        sequence,
                        msg: "reflection requires only bit 2".into(),
                    });
                }
                check_state("rho_a", rho_a)?;
                if compose_ab != 0
                    || left_association != 0
                    || right_association != 0
                    || parity != 0
                    || input_b != 0
                    || input_c != 0
                {
                    return Err(OracleDecodeError::FlagFieldMismatch {
                        sequence,
                        msg: "reflection unused fields must be zero".into(),
                    });
                }
                seen_reflect += 1;
            }
            RecordKind::Triple => {
                let need =
                    RecordFlags::COMPOSE_AB | RecordFlags::LEFT_RIGHT_PARITY | RecordFlags::RHO_A;
                if flags.bits() != need {
                    return Err(OracleDecodeError::FlagFieldMismatch {
                        sequence,
                        msg: "triple requires bits 0,1,2".into(),
                    });
                }
                check_state("compose_ab", compose_ab)?;
                check_state("left_association", left_association)?;
                check_state("right_association", right_association)?;
                check_state("rho_a", rho_a)?;
                if parity > 1 {
                    return Err(OracleDecodeError::IllegalParity(parity));
                }
                seen_triple += 1;
            }
        }

        let mut prefix = [0u8; 16];
        prefix.copy_from_slice(&rec[0..16]);
        let dig = OracleRecord::compute_digest(&prefix);
        if dig != expected_digest {
            return Err(OracleDecodeError::DigestMismatch { sequence });
        }

        records.push(OracleRecord {
            kind,
            flags,
            sequence,
            input_a,
            input_b,
            input_c,
            compose_ab,
            left_association,
            right_association,
            parity,
            rho_a,
            expected_digest,
        });
    }

    if seen_compose != COMPOSE_COUNT || seen_reflect != REFLECT_COUNT || seen_triple != TRIPLE_COUNT
    {
        return Err(OracleDecodeError::BadCounts);
    }

    Ok((header, records))
}

pub fn load_oracle_file(
    path: &Path,
) -> Result<(OracleHeader, Vec<OracleRecord>), OracleDecodeError> {
    let blob = fs::read(path)?;
    decode_oracle(&blob)
}
