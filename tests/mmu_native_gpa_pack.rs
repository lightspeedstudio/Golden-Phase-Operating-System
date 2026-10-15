//! Host pack of ARCH_NATIVE raw A32 GPA. Signing keys stay off the ARM image.

use kairos::arch::armv7::payloads;
use kairos::kernel::crypto::sign::SignKeypair;
use kairos::kernel::phasestack::cert::{issue_leaf_cert, issue_root_cert_with};
use kairos::kernel::phasestack::parser::{decode_manifest, entry, parse_gpa};
use kairos::kernel::phasestack::types::{AppExecutionKind, AppVersion, ARCH_NATIVE};
use kairos::kernel::phasestack::native::{self, encode_a32_exec_elf};
use kairos::sdk::phasestack::{pack_native_a32, pack_native_elf};

#[test]
fn packed_a32_gpa_is_native_not_elf() {
    let root_kp = SignKeypair::from_seed(&[0x51u8; 32]);
    let root = issue_root_cert_with(1, &root_kp);
    let leaf_kp = SignKeypair::from_seed(&[0x44u8; 32]);
    let leaf = issue_leaf_cert(2, &leaf_kp, &root, &root_kp, None);
    let code = payloads::legal_read();
    let bytes = pack_native_a32(
        "legal-read",
        AppVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        &code,
        &leaf_kp,
        &leaf,
        &[root],
    )
    .expect("pack");
    let parsed = parse_gpa(&bytes).expect("parse");
    let manifest = decode_manifest(entry(&parsed, "manifest.phase").unwrap()).unwrap();
    assert_eq!(manifest.exec_kind, AppExecutionKind::Native);
    assert_eq!(manifest.arch, ARCH_NATIVE);
    let payload = entry(&parsed, "code.phase").unwrap();
    assert_eq!(&payload[..4], &code[..4]);
    assert_ne!(&payload[..4], b"\x7fELF");
}

#[test]
fn elf_bytes_rejected_by_native_packer() {
    let root_kp = SignKeypair::from_seed(&[0x51u8; 32]);
    let root = issue_root_cert_with(1, &root_kp);
    let leaf_kp = SignKeypair::from_seed(&[0x44u8; 32]);
    let leaf = issue_leaf_cert(2, &leaf_kp, &root, &root_kp, None);
    let mut elf = vec![0x7f, b'E', b'L', b'F', 1];
    elf.resize(16, 0);
    let err = pack_native_a32(
        "elf",
        AppVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        &elf,
        &leaf_kp,
        &leaf,
        &[root],
    );
    assert!(err.is_err());
}

#[test]
fn packed_elf_gpa_is_native_elf() {
    let root_kp = SignKeypair::from_seed(&[0x51u8; 32]);
    let root = issue_root_cert_with(1, &root_kp);
    let leaf_kp = SignKeypair::from_seed(&[0x44u8; 32]);
    let leaf = issue_leaf_cert(2, &leaf_kp, &root, &root_kp, None);
    let elf = encode_a32_exec_elf(&payloads::legal_read()).expect("elf");
    let bytes = pack_native_elf(
        "legal-elf",
        AppVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        &elf,
        &leaf_kp,
        &leaf,
        &[root],
    )
    .expect("pack");
    let parsed = parse_gpa(&bytes).expect("parse");
    let manifest = decode_manifest(entry(&parsed, "manifest.phase").unwrap()).unwrap();
    assert_eq!(manifest.exec_kind, AppExecutionKind::Native);
    assert_eq!(manifest.arch, ARCH_NATIVE);
    let payload = entry(&parsed, "code.phase").unwrap();
    assert_eq!(&payload[..4], b"\x7fELF");
    let img = native::parse_elf(payload).unwrap();
    assert_eq!(img.entry_off, 0);
    assert_eq!(img.load_len, payloads::legal_read().len() as u32);
}
