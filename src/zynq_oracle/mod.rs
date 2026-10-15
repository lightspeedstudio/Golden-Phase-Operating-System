//! Frozen GPLV1 host oracle for the Zynq proof ladder (G0+).
//!
//! All mathematical results come from [`crate::core::gphase_core`].
//! This module only packs, digests, decodes, and compares.

mod compare;
mod decode;
mod format;
mod g64_export;
mod generate;

pub use compare::{
    compare_device_dump, compare_device_dump_str, dump_from_records, CompareReport, DeviceDumpError,
};
pub use decode::{decode_oracle, load_oracle_file, OracleDecodeError};
pub use format::{
    encode_phase_bits, hex_digest, one_hot, phase_from_bits, sha256_bytes, OracleHeader,
    OracleRecord, RecordFlags, RecordKind, ANCESTOR_ICE40_BITSTREAM_SHA256, FORMAT_VERSION,
    HEADER_SIZE, MAGIC, RECORD_SIZE, TOTAL_RECORDS,
};
pub use g64_export::write_g64_research_oracle;
pub use generate::{
    default_oracle_dir, generate_oracle, records_match_gphase_core, write_oracle_artifacts,
    GenerateError, GeneratedOracle,
};
