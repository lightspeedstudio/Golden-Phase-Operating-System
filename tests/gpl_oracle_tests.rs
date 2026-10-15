//! G0 GPLV1 oracle tests.

use std::fs;

use kairos::zynq_oracle::{
    compare_device_dump_str, decode_oracle, default_oracle_dir, dump_from_records,
    encode_phase_bits, generate_oracle, hex_digest, one_hot, phase_from_bits,
    records_match_gphase_core, sha256_bytes, write_oracle_artifacts, OracleHeader, FORMAT_VERSION,
    HEADER_SIZE, MAGIC, RECORD_SIZE, TOTAL_RECORDS,
};

#[test]
fn header_encoding_and_decoding() {
    let h = OracleHeader::canonical();
    let enc = h.encode();
    assert_eq!(&enc[0..4], &MAGIC);
    assert_eq!(u16::from_le_bytes([enc[4], enc[5]]), FORMAT_VERSION);
    assert_eq!(u16::from_le_bytes([enc[6], enc[7]]), HEADER_SIZE);
    assert_eq!(u16::from_le_bytes([enc[8], enc[9]]), RECORD_SIZE);
    let g = generate_oracle().unwrap();
    let (dh, _) = decode_oracle(&g.blob).unwrap();
    assert_eq!(dh, h);
}

#[test]
fn exactly_84_records_with_suite_counts() {
    let g = generate_oracle().unwrap();
    assert_eq!(g.records.len(), TOTAL_RECORDS as usize);
    assert_eq!(g.header.compose_count, 16);
    assert_eq!(g.header.reflect_count, 4);
    assert_eq!(g.header.triple_count, 64);
    let (_, recs) = decode_oracle(&g.blob).unwrap();
    assert_eq!(recs.len(), 84);
    assert_eq!(recs.iter().filter(|r| r.kind as u8 == 1).count(), 16);
    assert_eq!(recs.iter().filter(|r| r.kind as u8 == 2).count(), 4);
    assert_eq!(recs.iter().filter(|r| r.kind as u8 == 3).count(), 64);
}

#[test]
fn states_and_parity_in_range() {
    let g = generate_oracle().unwrap();
    for r in &g.records {
        assert!(r.input_a <= 3 && r.input_b <= 3 && r.input_c <= 3);
        assert!(r.compose_ab <= 3 && r.left_association <= 3 && r.right_association <= 3);
        assert!(r.rho_a <= 3);
        assert!(r.parity <= 1);
    }
}

#[test]
fn every_digest_validates_on_decode() {
    let g = generate_oracle().unwrap();
    decode_oracle(&g.blob).expect("digests must validate");
}

#[test]
fn regeneration_is_byte_identical() {
    let a = generate_oracle().unwrap();
    let b = generate_oracle().unwrap();
    assert_eq!(a.blob, b.blob);
}

#[test]
fn reloaded_records_match_gphase_core() {
    let g = generate_oracle().unwrap();
    let (_, recs) = decode_oracle(&g.blob).unwrap();
    records_match_gphase_core(&recs).unwrap();
}

#[test]
fn encoding_round_trip_phi() {
    for s in 0u8..4 {
        let bits = encode_phase_bits(s).unwrap();
        assert_eq!(bits, s);
        assert_eq!(phase_from_bits(bits).unwrap(), s);
    }
}

#[test]
fn one_hot_diagnostic_mapping() {
    assert_eq!(one_hot(0).unwrap(), 0b0001);
    assert_eq!(one_hot(1).unwrap(), 0b0010);
    assert_eq!(one_hot(2).unwrap(), 0b0100);
    assert_eq!(one_hot(3).unwrap(), 0b1000);
}

#[test]
fn positive_synthetic_device_dump_passes() {
    let g = generate_oracle().unwrap();
    let dump = dump_from_records(&g.records);
    let report = compare_device_dump_str(&g.blob, &dump).unwrap();
    assert!(report.ok);
    assert_eq!(report.mismatch_count, 0);
    assert_eq!(report.compose_pass, 16);
    assert_eq!(report.reflect_pass, 4);
    assert_eq!(report.algebra_pass, 64);
}

#[test]
fn one_bit_mutated_device_dump_fails() {
    let g = generate_oracle().unwrap();
    let mut dump = dump_from_records(&g.records);
    // Mutate first compose result: "C 0 0 0 0" -> flip last digit if 0 else set 0
    let first = dump.lines().next().unwrap().to_string();
    let parts: Vec<&str> = first.split_whitespace().collect();
    let y: u8 = parts[4].parse().unwrap();
    let mutated_y = (y + 1) % 4;
    let mut lines: Vec<String> = dump.lines().map(|l| l.to_string()).collect();
    lines[0] = format!("C {} {} {} {}", parts[1], parts[2], parts[3], mutated_y);
    dump = lines.join("\n") + "\n";
    let report = compare_device_dump_str(&g.blob, &dump).unwrap();
    assert!(!report.ok);
    assert!(report.mismatch_count > 0);
    assert!(report.first_failure.is_some());
}

#[test]
fn illegal_parity_value_is_rejected() {
    let g = generate_oracle().unwrap();
    // Find first triple line and set parity=2
    let dump = dump_from_records(&g.records);
    let mut lines: Vec<String> = dump.lines().map(|l| l.to_string()).collect();
    for line in &mut lines {
        if line.starts_with('T') {
            let mut p: Vec<&str> = line.split_whitespace().collect();
            p[8] = "2";
            *line = p.join(" ");
            break;
        }
    }
    let dump = lines.join("\n") + "\n";
    let report = compare_device_dump_str(&g.blob, &dump).unwrap();
    assert!(!report.ok);
    assert!(report.parity_illegal > 0);
}

#[test]
fn truncated_vectors_rejected() {
    let g = generate_oracle().unwrap();
    let truncated = &g.blob[..g.blob.len() - 10];
    assert!(decode_oracle(truncated).is_err());
}

#[test]
fn corrupted_digest_rejected() {
    let mut g = generate_oracle().unwrap();
    let off = 32 + 16; // first record digest start
    g.blob[off] ^= 0x01;
    assert!(decode_oracle(&g.blob).is_err());
}

#[test]
fn unknown_format_version_rejected() {
    let mut g = generate_oracle().unwrap();
    g.blob[4] = 99;
    g.blob[5] = 0;
    assert!(decode_oracle(&g.blob).is_err());
}

#[test]
fn frozen_committed_artifacts_match_fresh_generation() {
    let g = generate_oracle().unwrap();
    let dir = default_oracle_dir();
    let frozen = dir.join("vectors.bin");
    if !frozen.exists() {
        // Allow generating once in-test if missing (first bootstrap)
        write_oracle_artifacts(&dir, &g).unwrap();
    }
    let frozen_bytes = fs::read(&frozen).expect("frozen vectors.bin");
    assert_eq!(
        frozen_bytes, g.blob,
        "committed vectors.bin must match fresh generation"
    );
    let manifest = fs::read_to_string(dir.join("manifest.txt")).unwrap();
    let digests = fs::read_to_string(dir.join("digests.sha256")).unwrap();
    let vsha = hex_digest(&sha256_bytes(&g.blob));
    assert!(manifest.contains(&vsha));
    assert!(digests.contains(&vsha));
}
