//! PhaseStack Gates D–J host tests (`--features captable,phasestack`).
#![cfg(feature = "phasestack")]

use kairos::kernel::phasestack::native::{self, IsolationStatus};
use kairos::kernel::state_proof::ObjectKind;
use kairos::kernel::statechain_checkpoint::{
    sign_checkpoint, sign_checkpoint_v2, verify_checkpoint, verify_checkpoint_v2,
};
use kairos::kernel::phasestack::vm_runtime::PhaseVmRuntime;
use kairos::userland::parser::{parse, Command};
use kairos::vm::instruction::{encode_program, Instruction};

#[test]
fn no_verify_flag_is_rejected_by_parser() {
    match parse("app install x --no-verify") {
        Command::Usage(msg) => assert!(msg.contains("verification cannot be disabled")),
        other => panic!("expected usage, got {other:?}"),
    }
    match parse("app install x trust-any-cert") {
        Command::Usage(msg) => assert!(msg.contains("verification cannot be disabled")),
        other => panic!("expected usage, got {other:?}"),
    }
}

#[test]
fn object_kind_app_is_nine() {
    assert_eq!(ObjectKind::App.as_u8(), 9);
    assert_eq!(ObjectKind::from_u8(9), Some(ObjectKind::App));
}

#[test]
fn checkpoint_v2_binds_app_registry_root() {
    let key = [7u8; 32];
    let v1 = sign_checkpoint(&key, 1, 2, &[1u8; 32], &[2u8; 32], &[3u8; 32]);
    assert!(verify_checkpoint(&key, &v1));
    let v2 = sign_checkpoint_v2(&key, 1, 2, &[1u8; 32], &[2u8; 32], &[3u8; 32], &[9u8; 32]);
    assert!(verify_checkpoint_v2(&key, &v2));
    assert_ne!(v1.signature, v2.signature);
    assert!(!verify_checkpoint_v2(&key, &v1));
}

#[test]
fn native_mmu_tests_report_the_raw_a32_elf_split() {
    let raw = native::mmu_wx_status();
    #[cfg(not(all(target_arch = "arm", feature = "mmu-app-isolation")))]
    assert_eq!(raw, IsolationStatus::Unavailable);
    #[cfg(all(target_arch = "arm", feature = "mmu-app-isolation"))]
    assert!(raw == IsolationStatus::Blocked || raw == IsolationStatus::RawA32Ready);
    assert_eq!(native::elf_execute_status(), IsolationStatus::ElfRefused);
    let bytes = native::encode_a32_exec_elf(&[
        0x00, 0x00, 0xa0, 0xe3, 0x01, 0x70, 0xa0, 0xe3, 0x00, 0x00, 0x00, 0xef,
    ])
    .unwrap();
    assert!(native::parse_elf(&bytes).unwrap().is_elf);
}

#[test]
fn plugin_and_host_vms_are_separate() {
    let code = encode_program(&[Instruction::Halt]);
    let mut host = PhaseVmRuntime::new(&code, kairos::kernel::phasestack::types::PHASEVM_MAX_MEM).unwrap();
    let mut plugin = PhaseVmRuntime::new(&code, kairos::kernel::phasestack::types::PHASEVM_MAX_MEM).unwrap();
    host.execute(8).unwrap();
    plugin.execute(8).unwrap();
}

#[test]
fn default_features_do_not_enable_phasestack_in_cargo_toml() {
    let cargo = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
    let default_line = cargo
        .lines()
        .find(|l| l.starts_with("default ="))
        .expect("default features");
    assert!(
        !default_line.contains("phasestack"),
        "phasestack leaked into default: {default_line}"
    );
}

#[test]
fn mod_app_and_developer_revoke_parse() {
    match parse("mod app install /tmp/a.gpa") {
        Command::ModApp { args } => assert_eq!(args[0], "install"),
        other => panic!("expected ModApp, got {other:?}"),
    }
    match parse("mod cert developer-revoke 00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff") {
        Command::ModCertDeveloperRevoke { hash_hex } => assert_eq!(hash_hex.len(), 64),
        other => panic!("expected ModCertDeveloperRevoke, got {other:?}"),
    }
}

#[test]
fn arm_elf_scan_rejects_dev_ca_and_bypass() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        root.join("target/armv7a-none-eabi/release/kairos_zynq"),
        root.join("target/armv7a-none-eabi/release/kairos_zynq.elf"),
        root.join("fpga/zynq/evidence/kairos_zynq_boot/BOOT_PHASESTACK_SIGNED_GPA_20260814/kairos_zynq.elf"),
        root.join("fpga/zynq/evidence/kairos_zynq_boot/BOOT_PHASESTACK_SIGNED_GPA_20260814/BOOT.BIN"),
    ];
    let mut scanned = false;
    for path in candidates {
        if !path.exists() {
            continue;
        }
        scanned = true;
        let data = std::fs::read(&path).expect("read ARM image");
        for needle in [
            b"phasestack-dev" as &[u8],
            b"phasestack-sign-dev",
            b"KAIROS_PHASESTACK_RESEARCH_CA_SEED_V1",
            b"research_root_keypair",
            b"skip_signature_check",
            b"allow_unsigned_if_local",
            b"run_unchecked",
        ] {
            assert!(
                !contains_bytes(&data, needle),
                "forbidden bytes in {}: {}",
                path.display(),
                core::str::from_utf8(needle).unwrap()
            );
        }
    }
    let _ = scanned;
}

fn contains_bytes(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn cert_source_has_no_research_ca_seed() {
    let cert = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/kernel/phasestack/cert.rs"
    ));
    assert!(!cert.contains("KAIROS_PHASESTACK_RESEARCH_CA_SEED_V1"));
    assert!(!cert.contains("research_root_keypair"));
    assert!(!cert.contains("phasestack-dev"));
}

#[test]
fn committed_hello_gpa_verifies_against_public_pin() {
    use kairos::kernel::phasestack::capability::GrantSet;
    use kairos::kernel::phasestack::cert::{
        DeveloperCertificateStore, ROOT_PUBLIC_KEY_PIN,
    };
    use kairos::kernel::phasestack::revocation::PackageRevocationStore;
    use kairos::kernel::phasestack::types::{InstallScope, StagedPackage};
    use kairos::kernel::phasestack::verifier::{verify_package, VerifyInputs};

    let pin_file = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fpga/zynq/release/phasestack/ROOT_PUBLIC_KEY_PIN.sha256"
    ));
    assert_eq!(
        pin_file.trim(),
        "eddd244124fdae03508d3a6a1f85e6f9bd717b76a3e903431aab6e436e2d953c"
    );

    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fpga/zynq/release/phasestack/hello.gpa"
    ));
    assert_eq!(bytes.len(), 962);
    let staged = StagedPackage {
        bytes: bytes.to_vec(),
    };
    let store = DeveloperCertificateStore::new();
    let pkg = verify_package(VerifyInputs {
        staged: &staged,
        certs: &store,
        revocation: &PackageRevocationStore::new(),
        now_generation: 0,
        grants: GrantSet::empty(),
        scope: InstallScope::User,
        installed_version: None,
    })
    .expect("committed hello.gpa must verify on the public pin");
    assert_eq!(pkg.name(), "hello-a7f3c91e");
    assert_eq!(pkg.publisher_key_id(), ROOT_PUBLIC_KEY_PIN);

    let mut flipped = bytes.to_vec();
    flipped[20] ^= 0xFF;
    let bad = StagedPackage { bytes: flipped };
    assert!(verify_package(VerifyInputs {
        staged: &bad,
        certs: &store,
        revocation: &PackageRevocationStore::new(),
        now_generation: 0,
        grants: GrantSet::empty(),
        scope: InstallScope::User,
        installed_version: None,
    })
    .is_err());
}

#[test]
fn utility_feature_list_excludes_sign_dev() {
    let cargo = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
    let start = cargo.find("utility = [").expect("utility features");
    let rest = &cargo[start..];
    let end = rest.find(']').expect("utility close");
    let block = &rest[..end];
    assert!(block.contains("\"phasestack\""));
    assert!(!block.contains("phasestack-dev"));
    assert!(!block.contains("phasestack-sign-dev"));
}
