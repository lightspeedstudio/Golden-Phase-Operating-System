//! Generate GPLV1 oracle records from `gphase_core` only.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::core::gphase_core::{
    compose, left_group, reflect, right_group, topology_status, PhaseState, TopologyKind,
};

use super::format::{
    hex_digest, sha256_bytes, OracleHeader, OracleRecord, RecordFlags, RecordKind,
    ANCESTOR_ICE40_BITSTREAM_SHA256, COMPOSE_COUNT, REFLECT_COUNT, TOTAL_RECORDS, TRIPLE_COUNT,
};

#[derive(Debug)]
pub enum GenerateError {
    Io(io::Error),
    InvalidState,
}

impl From<io::Error> for GenerateError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl std::fmt::Display for GenerateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io: {e}"),
            Self::InvalidState => write!(f, "invalid phase state"),
        }
    }
}

impl std::error::Error for GenerateError {}

#[derive(Debug, Clone)]
pub struct GeneratedOracle {
    pub header: OracleHeader,
    pub records: Vec<OracleRecord>,
    pub blob: Vec<u8>,
}

fn ps(v: u8) -> Result<PhaseState, GenerateError> {
    PhaseState::from_u8(v).ok_or(GenerateError::InvalidState)
}

fn parity_u8(kind: TopologyKind) -> u8 {
    match kind {
        TopologyKind::Same => 0,
        TopologyKind::Reflection => 1,
    }
}

pub fn generate_oracle() -> Result<GeneratedOracle, GenerateError> {
    let header = OracleHeader::canonical();
    let mut records = Vec::with_capacity(TOTAL_RECORDS as usize);
    let mut sequence: u32 = 0;

    // Compose: a outer, b inner
    for a in 0u8..4 {
        for b in 0u8..4 {
            let pa = ps(a)?;
            let pb = ps(b)?;
            let ab = compose(pa, pb).as_u8();
            let rho = reflect(pa).as_u8();
            let rec = OracleRecord {
                kind: RecordKind::Compose,
                flags: RecordFlags(RecordFlags::COMPOSE_AB | RecordFlags::RHO_A),
                sequence,
                input_a: a,
                input_b: b,
                input_c: 0,
                compose_ab: ab,
                left_association: 0,
                right_association: 0,
                parity: 0,
                rho_a: rho,
                expected_digest: [0u8; 32],
            }
            .with_digest();
            records.push(rec);
            sequence += 1;
        }
    }

    // Reflection
    for a in 0u8..4 {
        let pa = ps(a)?;
        let rho = reflect(pa).as_u8();
        let rec = OracleRecord {
            kind: RecordKind::Reflection,
            flags: RecordFlags(RecordFlags::RHO_A),
            sequence,
            input_a: a,
            input_b: 0,
            input_c: 0,
            compose_ab: 0,
            left_association: 0,
            right_association: 0,
            parity: 0,
            rho_a: rho,
            expected_digest: [0u8; 32],
        }
        .with_digest();
        records.push(rec);
        sequence += 1;
    }

    // Triples: a outer, b middle, c inner
    for a in 0u8..4 {
        for b in 0u8..4 {
            for c in 0u8..4 {
                let pa = ps(a)?;
                let pb = ps(b)?;
                let pc = ps(c)?;
                let ab = compose(pa, pb).as_u8();
                let left = left_group(pa, pb, pc).as_u8();
                let right = right_group(pa, pb, pc).as_u8();
                let topo = topology_status(pa, pb, pc);
                let parity = parity_u8(topo.kind);
                let rho = reflect(pa).as_u8();
                let rec = OracleRecord {
                    kind: RecordKind::Triple,
                    flags: RecordFlags(
                        RecordFlags::COMPOSE_AB
                            | RecordFlags::LEFT_RIGHT_PARITY
                            | RecordFlags::RHO_A,
                    ),
                    sequence,
                    input_a: a,
                    input_b: b,
                    input_c: c,
                    compose_ab: ab,
                    left_association: left,
                    right_association: right,
                    parity,
                    rho_a: rho,
                    expected_digest: [0u8; 32],
                }
                .with_digest();
                records.push(rec);
                sequence += 1;
            }
        }
    }

    debug_assert_eq!(records.len(), TOTAL_RECORDS as usize);
    debug_assert_eq!(
        records
            .iter()
            .filter(|r| r.kind == RecordKind::Compose)
            .count(),
        COMPOSE_COUNT as usize
    );
    debug_assert_eq!(
        records
            .iter()
            .filter(|r| r.kind == RecordKind::Reflection)
            .count(),
        REFLECT_COUNT as usize
    );
    debug_assert_eq!(
        records
            .iter()
            .filter(|r| r.kind == RecordKind::Triple)
            .count(),
        TRIPLE_COUNT as usize
    );

    let mut blob = Vec::with_capacity(32 + 48 * records.len());
    blob.extend_from_slice(&header.encode());
    for r in &records {
        blob.extend_from_slice(&r.encode());
    }

    Ok(GeneratedOracle {
        header,
        records,
        blob,
    })
}

pub fn default_oracle_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fpga/zynq/oracle")
}

pub fn manifest_text(vectors_sha256_hex: &str) -> String {
    format!(
        "GATE=G0_MATH_ORACLE\n\
FORMAT=GPLV1\n\
HEADER_SIZE=32\n\
RECORD_SIZE=48\n\
VECTOR_COUNT=84\n\
COMPOSE_COUNT=16\n\
REFLECT_COUNT=4\n\
TRIPLE_COUNT=64\n\
ENCODING=PHI0:00 PHI1:01 PHI2:10 PHI3:11\n\
PARITY=0:SAME 1:REFLECTION\n\
DIGEST=SHA256_RECORD_BYTES_0_TO_15\n\
SOURCE=src/core/gphase_core.rs\n\
SEQUENCE_ORDER=COMPOSE_THEN_REFLECT_THEN_TRIPLES_A_B_C\n\
ANCESTOR_ICE40_BITSTREAM_SHA256={ANCESTOR_ICE40_BITSTREAM_SHA256}\n\
VECTORS_SHA256={vectors_sha256_hex}\n"
    )
}

pub fn write_oracle_artifacts(dir: &Path, oracle: &GeneratedOracle) -> Result<(), GenerateError> {
    fs::create_dir_all(dir)?;
    let vectors_path = dir.join("vectors.bin");
    fs::write(&vectors_path, &oracle.blob)?;

    let vectors_sha = hex_digest(&sha256_bytes(&oracle.blob));
    let manifest = manifest_text(&vectors_sha);
    let manifest_path = dir.join("manifest.txt");
    fs::write(&manifest_path, &manifest)?;

    let manifest_sha = hex_digest(&sha256_bytes(manifest.as_bytes()));
    let digests = format!("{vectors_sha}  vectors.bin\n{manifest_sha}  manifest.txt\n");
    fs::write(dir.join("digests.sha256"), digests)?;
    Ok(())
}

/// Verify every record still matches live `gphase_core`.
pub fn records_match_gphase_core(records: &[OracleRecord]) -> Result<(), String> {
    for r in records {
        let a =
            PhaseState::from_u8(r.input_a).ok_or_else(|| format!("bad a seq={}", r.sequence))?;
        match r.kind {
            RecordKind::Compose => {
                let b = PhaseState::from_u8(r.input_b)
                    .ok_or_else(|| format!("bad b seq={}", r.sequence))?;
                let ab = compose(a, b).as_u8();
                let rho = reflect(a).as_u8();
                if ab != r.compose_ab {
                    return Err(format!(
                        "compose mismatch seq={} expected={ab} got={}",
                        r.sequence, r.compose_ab
                    ));
                }
                if rho != r.rho_a {
                    return Err(format!(
                        "rho mismatch seq={} expected={rho} got={}",
                        r.sequence, r.rho_a
                    ));
                }
            }
            RecordKind::Reflection => {
                let rho = reflect(a).as_u8();
                if rho != r.rho_a {
                    return Err(format!(
                        "reflect mismatch seq={} expected={rho} got={}",
                        r.sequence, r.rho_a
                    ));
                }
            }
            RecordKind::Triple => {
                let b = PhaseState::from_u8(r.input_b)
                    .ok_or_else(|| format!("bad b seq={}", r.sequence))?;
                let c = PhaseState::from_u8(r.input_c)
                    .ok_or_else(|| format!("bad c seq={}", r.sequence))?;
                let ab = compose(a, b).as_u8();
                let left = left_group(a, b, c).as_u8();
                let right = right_group(a, b, c).as_u8();
                let topo = topology_status(a, b, c);
                let parity = parity_u8(topo.kind);
                let rho = reflect(a).as_u8();
                if ab != r.compose_ab
                    || left != r.left_association
                    || right != r.right_association
                    || parity != r.parity
                    || rho != r.rho_a
                {
                    return Err(format!(
                        "triple mismatch seq={} kind=3 a={} b={} c={} \
                         expected ab={} L={} R={} p={} rho={} \
                         got ab={} L={} R={} p={} rho={}",
                        r.sequence,
                        r.input_a,
                        r.input_b,
                        r.input_c,
                        ab,
                        left,
                        right,
                        parity,
                        rho,
                        r.compose_ab,
                        r.left_association,
                        r.right_association,
                        r.parity,
                        r.rho_a
                    ));
                }
            }
        }
    }
    Ok(())
}
