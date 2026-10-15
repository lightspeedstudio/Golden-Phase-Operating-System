//! Negative suite: host ELF execute stays unavailable (n20/n40); verified native ELF launch on host is n51.

use super::*;
use crate::kernel::crypto::sign::SignKeypair;
use crate::kernel::phasestack::capability::GrantSet;
use crate::kernel::phasestack::cert::{
    issue_leaf_cert, issue_root_cert_with, sign_certificate, DeveloperCertificateStore,
};
use crate::kernel::phasestack::hooks::{payload_from_bytes, HookGrant, HookMessage, PhaseHookBus};
use crate::kernel::phasestack::install::{self, app_registry_root};
use crate::kernel::phasestack::loader::PhaseAppLoader;
use crate::kernel::phasestack::native::{self, IsolationStatus};
use crate::kernel::phasestack::parser::{encode_gpa, parse_gpa, GpaEntry};
use crate::kernel::phasestack::revocation::PackageRevocationStore;
use crate::kernel::phasestack::types::{
    AppState, InstallScope, MAX_CHAIN_DEPTH, MAX_GPA_NAME_LEN, MAX_HOOK_FUEL, MAX_HOOK_PAYLOAD,
    MAX_HOOK_QUEUE, PRIVILEGED_HOOK_PRIORITY, PhaseStackError, StandardHook, StagedPackage,
};
use crate::kernel::phasestack::verifier::{verify_package, VerifyInputs};
use crate::kernel::phasestack::verifier::tests::{signed_gpa, signed_gpa_with_generation};
use crate::kernel::phasestack::vm_runtime::PhaseVmRuntime;
use crate::kernel::Kernel;
use crate::userland::parser::{parse, Command};
use crate::vm::instruction::{encode_program, Instruction, Register};

fn gpa_with_name(name: &str) -> Result<crate::platform::Vec<u8>, PhaseStackError> {
    encode_gpa(&[GpaEntry {
        name: name.into(),
        data: b"x".to_vec(),
    }])
}

fn verify_now(
    staged: &StagedPackage,
    store: &DeveloperCertificateStore,
    rev: &PackageRevocationStore,
    grants: GrantSet,
    now_generation: u64,
) -> Result<crate::kernel::phasestack::types::VerifiedPackage, PhaseStackError> {
    verify_package(VerifyInputs {
        staged,
        certs: store,
        revocation: rev,
        now_generation,
        grants,
        scope: InstallScope::User,
        installed_version: None,
    })
}

#[test]
fn n01_unknown_cert_version() {
    let root_kp = SignKeypair::from_seed(&[0x51u8; 32]);
    let mut store = DeveloperCertificateStore::with_root_public_key(root_kp.public_bytes());
    let mut root = issue_root_cert_with(1, &root_kp);
    store.insert(root.clone()).unwrap();
    root.version = 99;
    sign_certificate(&mut root, &root_kp);
    assert_eq!(
        store.verify_chain(&root, &[], 0, &|_| false),
        Err(PhaseStackError::UnknownVersion)
    );
}

#[test]
fn n02_duplicate_serial() {
    let root_kp = SignKeypair::from_seed(&[0x51u8; 32]);
    let mut store = DeveloperCertificateStore::with_root_public_key(root_kp.public_bytes());
    store.insert(issue_root_cert_with(1, &root_kp)).unwrap();
    assert_eq!(
        store.insert(issue_root_cert_with(1, &root_kp)),
        Err(PhaseStackError::DuplicateSerial)
    );
}

#[test]
fn n03_chain_too_deep() {
    let root_kp = SignKeypair::from_seed(&[0x51u8; 32]);
    let store = DeveloperCertificateStore::with_root_public_key(root_kp.public_bytes());
    let root = issue_root_cert_with(1, &root_kp);
    let extras = vec![root.clone(); MAX_CHAIN_DEPTH];
    assert_eq!(
        store.verify_chain(&root, &extras, 0, &|_| false),
        Err(PhaseStackError::ChainTooDeep)
    );
}

#[test]
fn n04_cycle() {
    let root_kp = SignKeypair::from_seed(&[0x51u8; 32]);
    let store = DeveloperCertificateStore::with_root_public_key(root_kp.public_bytes());
    let root = issue_root_cert_with(1, &root_kp);
    assert_eq!(
        store.verify_chain(&root, &[root.clone()], 0, &|_| false),
        Err(PhaseStackError::ChainCycle)
    );
}

#[test]
fn n05_bad_algorithm() {
    let root_kp = SignKeypair::from_seed(&[0x51u8; 32]);
    let store = DeveloperCertificateStore::with_root_public_key(root_kp.public_bytes());
    let mut root = issue_root_cert_with(1, &root_kp);
    root.algorithm = 3;
    sign_certificate(&mut root, &root_kp);
    assert_eq!(
        store.verify_chain(&root, &[], 0, &|_| false),
        Err(PhaseStackError::AlgorithmRejected)
    );
}

#[test]
fn n06_path_traversal() {
    assert_eq!(gpa_with_name("a/../b"), Err(PhaseStackError::PathTraversal));
}

#[test]
fn n07_absolute_path() {
    assert_eq!(gpa_with_name("/abs"), Err(PhaseStackError::AbsolutePath));
}

#[test]
fn n08_gpa_overflow() {
    let mut crafted = crate::platform::Vec::new();
    crafted.extend_from_slice(parser::GPA_MAGIC);
    crate::kernel::phasestack::canonical::put_u16(&mut crafted, 1);
    crate::kernel::phasestack::canonical::put_u32(&mut crafted, 1);
    crate::kernel::phasestack::canonical::put_u16(&mut crafted, 1);
    crafted.push(b'a');
    crate::kernel::phasestack::canonical::put_u32(&mut crafted, u32::MAX);
    crate::kernel::phasestack::canonical::put_u32(&mut crafted, 4);
    assert_eq!(parse_gpa(&crafted), Err(PhaseStackError::Overflow));
}

#[test]
fn n09_duplicate_gpa_entry() {
    let mut crafted = crate::platform::Vec::new();
    crafted.extend_from_slice(parser::GPA_MAGIC);
    crate::kernel::phasestack::canonical::put_u16(&mut crafted, 1);
    crate::kernel::phasestack::canonical::put_u32(&mut crafted, 2);
    for _ in 0..2 {
        crate::kernel::phasestack::canonical::put_u16(&mut crafted, 1);
        crafted.push(b'a');
        crate::kernel::phasestack::canonical::put_u32(&mut crafted, 0);
        crate::kernel::phasestack::canonical::put_u32(&mut crafted, 0);
    }
    assert_eq!(parse_gpa(&crafted), Err(PhaseStackError::DuplicateEntry));
}

#[test]
fn n10_unknown_gpa_version() {
    let mut b = encode_gpa(&[GpaEntry {
        name: "a".into(),
        data: b"x".to_vec(),
    }])
    .unwrap();
    b[4] = 2;
    assert_eq!(parse_gpa(&b), Err(PhaseStackError::UnknownVersion));
}

#[test]
fn n11_bad_magic() {
    assert_eq!(parse_gpa(b"PK\x03\x04xxxx"), Err(PhaseStackError::BadMagic));
}

#[test]
fn n12_oversize_name() {
    let name = "n".repeat(MAX_GPA_NAME_LEN + 1);
    assert_eq!(gpa_with_name(&name), Err(PhaseStackError::Oversize));
}

#[test]
fn n13_unknown_cert() {
    let store = DeveloperCertificateStore::new();
    let rogue = SignKeypair::from_seed(&[0x99u8; 32]);
    let mut fake = issue_root_cert_with(9, &rogue);
    fake.subject_public_key = rogue.public_bytes();
    fake.subject_id = rogue.public_bytes();
    fake.issuer_id = rogue.public_bytes();
    sign_certificate(&mut fake, &rogue);
    assert_eq!(
        store.verify_chain(&fake, &[], 0, &|_| false),
        Err(PhaseStackError::UnknownCert)
    );
}

#[test]
fn n14_wrong_signature() {
    let (staged, store, grants) = signed_gpa(true);
    let mut parsed = parse_gpa(&staged.bytes).unwrap();
    let sig = parsed
        .entries
        .iter_mut()
        .find(|e| e.name == "package.sig")
        .unwrap();
    sig.data[0] ^= 0xff;
    let staged = StagedPackage {
        bytes: encode_gpa(&parsed.entries).unwrap(),
    };
    assert_eq!(
        verify_now(&staged, &store, &PackageRevocationStore::new(), grants, 0),
        Err(PhaseStackError::WrongSignature)
    );
}

#[test]
fn n15_revoked_cert() {
    let (staged, store, grants) = signed_gpa(true);
    let pkg = verify_now(&staged, &store, &PackageRevocationStore::new(), grants, 0).unwrap();
    let mut rev = PackageRevocationStore::new();
    rev.revoke_certificate(pkg.developer_cert_hash());
    assert_eq!(
        verify_now(&staged, &store, &rev, grants, 0),
        Err(PhaseStackError::Revoked)
    );
}

#[test]
fn n16_revoked_package() {
    let (staged, store, grants) = signed_gpa(true);
    let pkg = verify_now(&staged, &store, &PackageRevocationStore::new(), grants, 0).unwrap();
    let mut rev = PackageRevocationStore::new();
    rev.revoke_package(pkg.package_root());
    assert_eq!(
        verify_now(&staged, &store, &rev, grants, 0),
        Err(PhaseStackError::Revoked)
    );
}

#[test]
fn n17_expired_generation() {
    let (staged, store, grants) = signed_gpa_with_generation(true, Some(1));
    assert_eq!(
        verify_now(&staged, &store, &PackageRevocationStore::new(), grants, 2),
        Err(PhaseStackError::GenerationExpired)
    );
}

#[test]
fn n18_unknown_issuer() {
    let root_kp = SignKeypair::from_seed(&[0x51u8; 32]);
    let store = DeveloperCertificateStore::with_root_public_key(root_kp.public_bytes());
    let root = issue_root_cert_with(1, &root_kp);
    let leaf_kp = SignKeypair::from_seed(&[0x22u8; 32]);
    let mut leaf = issue_leaf_cert(2, &leaf_kp, &root, &root_kp, None);
    leaf.issuer_certificate_hash = [0x11u8; 32];
    sign_certificate(&mut leaf, &root_kp);
    assert_eq!(
        store.verify_chain(&leaf, &[], 0, &|_| false),
        Err(PhaseStackError::UnknownIssuer)
    );
}

#[test]
fn n19_unsigned() {
    let (mut staged, store, grants) = signed_gpa(true);
    let mut parsed = parse_gpa(&staged.bytes).unwrap();
    let sig = parsed
        .entries
        .iter_mut()
        .find(|e| e.name == "package.sig")
        .unwrap();
    sig.data = vec![0u8; 32];
    staged.bytes = encode_gpa(&parsed.entries).unwrap();
    assert_eq!(
        verify_now(&staged, &store, &PackageRevocationStore::new(), grants, 0),
        Err(PhaseStackError::Unsigned)
    );
}

#[test]
fn n20_native_execute_unavailable() {
    let bytes = native::encode_a32_exec_elf(&[
        0x00, 0x00, 0xa0, 0xe3, 0x01, 0x70, 0xa0, 0xe3, 0x00, 0x00, 0x00, 0xef,
    ])
    .unwrap();
    let img = native::parse_elf(&bytes).unwrap();
    let mut kernel = Kernel::new();
    assert_eq!(
        native::execute(&img, &bytes, &mut kernel, 0, [0u8; 32]),
        Err(PhaseStackError::NativeIsolationUnavailable)
    );
}

#[test]
fn n21_plugin_cap_escalation() {
    use crate::kernel::captable::{RIGHT_READ, RIGHT_WRITE};
    assert_eq!(
        capability::deny_escalation(RIGHT_READ | RIGHT_WRITE, RIGHT_READ),
        Err(PhaseStackError::PluginCapEscalation)
    );
}

#[test]
fn n22_privileged_priority_auto_grant() {
    let mut reg = hooks::PhaseHookRegistry::new();
    assert_eq!(
        reg.grant(
            [1u8; 32],
            hooks::HookGrant {
                hook: StandardHook::OnStart,
                max_priority: PRIVILEGED_HOOK_PRIORITY,
            },
        ),
        Err(PhaseStackError::PrivilegedPriorityDenied)
    );
}

#[test]
fn n23_load_denied_on_code_hash_mismatch() {
    let code = encode_program(&[Instruction::Halt]);
    let pkg = types::VerifiedPackage::from_parts(
        [1u8; 32],
        types::AppVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        [2u8; 32],
        [0x11u8; 32],
        [3u8; 32],
        [4u8; 32],
        [5u8; 32],
        [6u8; 32],
        types::AppExecutionKind::PhaseVm,
        0,
        0,
            types::ABI_VERSION_V1,
        "t".into(),
        code,
        types::PKG_TYPE_APP,
        types::ExecEnvelope::default(),
    );
    let inst = verifier::commit_installation(
        pkg,
        GrantSet::empty(),
        InstallScope::User,
        "/x".into(),
    )
    .unwrap();
    assert_eq!(
        PhaseAppLoader::load(&inst).unwrap_err(),
        PhaseStackError::LoadDenied
    );
}

#[test]
fn n24_launch_rehash_quarantines() {
    let mut kernel = Kernel::new();
    let (staged, store, grants) = signed_gpa(true);
    kernel.phasestack.certs = store;
    let inst = install::install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
    let app_id = inst.package().app_id();
    let path = format!("{}/code.phase", inst.store_path());
    kernel.fs.unlock_path(&path).unwrap();
    kernel.fs.write_file(&path, b"tampered").unwrap();
    assert_eq!(
        install::launch(&mut kernel, app_id, 8),
        Err(PhaseStackError::PayloadRootMismatch)
    );
    assert_eq!(
        kernel.phasestack.registry.get(app_id).unwrap().state(),
        AppState::Quarantined
    );
}

#[test]
fn n25_immutable_store_rejects_write() {
    let mut kernel = Kernel::new();
    let (staged, store, grants) = signed_gpa(true);
    kernel.phasestack.certs = store;
    let inst = install::install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
    let path = format!("{}/code.phase", inst.store_path());
    assert!(matches!(
        kernel.fs.write_file(&path, b"x"),
        Err(crate::fs::FsError::Locked(_))
    ));
}

#[test]
fn n26_vm_oob() {
    let code = encode_program(&[Instruction::LoadMem {
        reg: Register::A,
        addr: 4000,
    }]);
    let mut rt = PhaseVmRuntime::new(&code, types::PHASEVM_MAX_MEM).unwrap();
    assert_eq!(rt.execute(4), Err(PhaseStackError::VmFault));
}

#[test]
fn n27_fuel_abort() {
    let code = encode_program(&[Instruction::Jmp { target: 0 }]);
    let mut rt = PhaseVmRuntime::new(&code, types::PHASEVM_MAX_MEM).unwrap();
    assert_eq!(rt.execute(3), Err(PhaseStackError::FuelAbort));
}

#[test]
fn n28_hook_queue_full() {
    let mut bus = PhaseHookBus::new();
    let src = [4u8; 32];
    bus.registry
        .grant(
            src,
            HookGrant {
                hook: StandardHook::OnLoad,
                max_priority: 8,
            },
        )
        .unwrap();
    let msg = HookMessage {
        hook: StandardHook::OnLoad,
        source: src,
        payload: payload_from_bytes(&[]).unwrap(),
        fuel: 10,
        deadline_tick: 100,
        mode: types::HookMode::Notify,
        priority: 8,
    };
    for _ in 0..MAX_HOOK_QUEUE {
        bus.require_delivery(msg.clone(), 1).unwrap();
    }
    assert_eq!(
        bus.require_delivery(msg, 1),
        Err(PhaseStackError::HookQueueFull)
    );
}

#[test]
fn n29_hook_payload_oversize() {
    let mut over = types::BoundedVec::new(MAX_HOOK_PAYLOAD + 8);
    for _ in 0..MAX_HOOK_PAYLOAD + 1 {
        over.push(1).unwrap();
    }
    let mut bus = PhaseHookBus::new();
    let src = [5u8; 32];
    bus.registry
        .grant(
            src,
            HookGrant {
                hook: StandardHook::OnLoad,
                max_priority: 8,
            },
        )
        .unwrap();
    let msg = HookMessage {
        hook: StandardHook::OnLoad,
        source: src,
        payload: over,
        fuel: 10,
        deadline_tick: 100,
        mode: types::HookMode::Notify,
        priority: 8,
    };
    assert_eq!(
        bus.require_delivery(msg, 1),
        Err(PhaseStackError::HookPayloadOversize)
    );
}

#[test]
fn n30_hook_fuel_exceeded() {
    let mut bus = PhaseHookBus::new();
    let src = [6u8; 32];
    bus.registry
        .grant(
            src,
            HookGrant {
                hook: StandardHook::OnLoad,
                max_priority: 8,
            },
        )
        .unwrap();
    let msg = HookMessage {
        hook: StandardHook::OnLoad,
        source: src,
        payload: payload_from_bytes(&[]).unwrap(),
        fuel: MAX_HOOK_FUEL + 1,
        deadline_tick: 100,
        mode: types::HookMode::Notify,
        priority: 8,
    };
    assert_eq!(
        bus.require_delivery(msg, 1),
        Err(PhaseStackError::HookFuelExceeded)
    );
}

#[test]
fn n31_hook_deadline() {
    let mut bus = PhaseHookBus::new();
    let src = [7u8; 32];
    bus.registry
        .grant(
            src,
            HookGrant {
                hook: StandardHook::OnLoad,
                max_priority: 8,
            },
        )
        .unwrap();
    let msg = HookMessage {
        hook: StandardHook::OnLoad,
        source: src,
        payload: payload_from_bytes(&[]).unwrap(),
        fuel: 10,
        deadline_tick: 5,
        mode: types::HookMode::Notify,
        priority: 8,
    };
    assert_eq!(
        bus.require_delivery(msg, 6),
        Err(PhaseStackError::HookDeadline)
    );
}

#[test]
fn n32_plugin_fault_does_not_kill_host() {
    let host_code = encode_program(&[Instruction::Halt]);
    let plugin_code = encode_program(&[Instruction::LoadMem {
        reg: Register::A,
        addr: 4000,
    }]);
    let mut plugin = PhaseVmRuntime::new(&plugin_code, types::PHASEVM_MAX_MEM).unwrap();
    assert_eq!(plugin.execute(8), Err(PhaseStackError::VmFault));
    let mut host = PhaseVmRuntime::new(&host_code, types::PHASEVM_MAX_MEM).unwrap();
    host.execute(8).unwrap();
}

#[test]
fn n33_native_plugin_denied() {
    let bytes = native::encode_a32_exec_elf(&[
        0x00, 0x00, 0xa0, 0xe3, 0x01, 0x70, 0xa0, 0xe3, 0x00, 0x00, 0x00, 0xef,
    ])
    .unwrap();
    let img = native::parse_elf(&bytes).unwrap();
    assert_eq!(
        native::execute_native_plugin(&img),
        Err(PhaseStackError::NativePluginDenied)
    );
}

#[test]
fn n34_root_cannot_skip_verify() {
    let mut kernel = Kernel::new();
    assert_eq!(
        kernel.authority.authority,
        crate::kernel::authority::AuthorityMode::Root
    );
    let staged = StagedPackage {
        bytes: b"not-a-gpa".to_vec(),
    };
    assert_eq!(
        install::install(&mut kernel, &staged, GrantSet::empty(), InstallScope::System),
        Err(PhaseStackError::BadMagic)
    );
}

#[test]
fn n35_no_verify_flag_rejected() {
    match parse("app install x --no-verify") {
        Command::Usage(msg) => assert!(msg.contains("verification cannot be disabled")),
        other => panic!("expected usage, got {other:?}"),
    }
}

#[test]
fn n36_grant_not_subset() {
    let (staged, store, grants) = signed_gpa(false);
    assert_eq!(
        verify_now(&staged, &store, &PackageRevocationStore::new(), grants, 0),
        Err(PhaseStackError::GrantNotSubset)
    );
}

#[test]
fn n37_registry_leaf_change_changes_root() {
    let mut kernel = Kernel::new();
    let before = app_registry_root(&kernel.phasestack);
    let (staged, store, grants) = signed_gpa(true);
    kernel.phasestack.certs = store;
    install::install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
    assert_ne!(before, app_registry_root(&kernel.phasestack));
}

#[test]
fn n38_failed_install_leaves_no_row() {
    let mut kernel = Kernel::new();
    let (staged, store, grants) = signed_gpa(false);
    kernel.phasestack.certs = store;
    assert!(install::install(&mut kernel, &staged, grants, InstallScope::User).is_err());
    assert!(kernel.phasestack.registry.is_empty());
}

#[test]
fn n39_mmu_wx_reports_raw_a32_not_blanket_blocked() {
    let raw = native::mmu_wx_status();
    #[cfg(not(all(target_arch = "arm", feature = "mmu-app-isolation")))]
    assert_eq!(raw, IsolationStatus::Unavailable);
    #[cfg(all(target_arch = "arm", feature = "mmu-app-isolation"))]
    assert!(raw == IsolationStatus::Blocked || raw == IsolationStatus::RawA32Ready);
    assert_ne!(raw, native::elf_execute_status());
}

#[test]
fn n40_elf_execute_blocked_not_pass() {
    assert_eq!(native::elf_execute_status(), IsolationStatus::ElfRefused);
    let bytes = native::encode_a32_exec_elf(&[
        0x00, 0x00, 0xa0, 0xe3, 0x01, 0x70, 0xa0, 0xe3, 0x00, 0x00, 0x00, 0xef,
    ])
    .unwrap();
    let img = native::parse_elf(&bytes).unwrap();
    let mut kernel = Kernel::new();
    assert_eq!(
        native::execute(&img, &bytes, &mut kernel, 0, [0u8; 32]),
        Err(PhaseStackError::NativeIsolationUnavailable)
    );
}

#[test]
fn n51_native_elf_verifies_host_launch_unavailable() {
    use crate::kernel::phasestack::parser::CanonicalManifest;
    use crate::kernel::phasestack::verifier::tests::signed_gpa_custom;
    let (staged, store, grants) = signed_gpa_custom(|m, code| {
        let pubk = m.publisher_key_id;
        let name = m.name.clone();
        let version = m.version;
        *m = CanonicalManifest::native_a32(name, version);
        m.publisher_key_id = pubk;
        *code = native::encode_a32_exec_elf(&[
            0x00, 0x00, 0xa0, 0xe3, 0x01, 0x70, 0xa0, 0xe3, 0x00, 0x00, 0x00, 0xef,
        ])
        .unwrap();
    });
    let mut kernel = Kernel::new();
    kernel.phasestack.certs = store;
    let inst = install::install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
    assert_eq!(
        install::launch(&mut kernel, inst.package().app_id(), 8),
        Err(PhaseStackError::NativeIsolationUnavailable)
    );
}

#[test]
fn n41_retired_research_ca_refused() {
    use crate::kernel::phasestack::cert::RETIRED_RESEARCH_CA_PIN;
    use crate::kernel::phasestack::types::{
        CERTIFICATE_VERSION_V1, CERT_ALG_ED25519, CERT_FLAG_CAN_ISSUE, CERT_FLAG_ROOT, NO_ISSUER,
    };
    let store = DeveloperCertificateStore::new();
    let retired = crate::kernel::phasestack::cert::DeveloperCertificateV1 {
        version: CERTIFICATE_VERSION_V1,
        serial: 1,
        algorithm: CERT_ALG_ED25519,
        subject_id: RETIRED_RESEARCH_CA_PIN,
        subject_public_key: RETIRED_RESEARCH_CA_PIN,
        issuer_id: RETIRED_RESEARCH_CA_PIN,
        issuer_certificate_hash: NO_ISSUER,
        package_type_mask: types::PKG_TYPE_APP,
        scope_mask: types::SCOPE_USER,
        generation_start: 0,
        generation_end: None,
        flags: CERT_FLAG_CAN_ISSUE | CERT_FLAG_ROOT,
        signature: [0u8; 64],
    };
    assert_eq!(
        store.verify_chain(&retired, &[], 0, &|_| false),
        Err(PhaseStackError::RetiredSigner)
    );
}

#[test]
fn n42_undeclared_print_refused() {
    use crate::core::gphase_core::PhaseState;
    use crate::kernel::phasestack::verifier::tests::signed_gpa_custom;
    let (staged, store, grants) = signed_gpa_custom(|_m, code| {
        *code = encode_program(&[
            Instruction::Load {
                reg: Register::A,
                imm: PhaseState::Phi1,
            },
            Instruction::Print { reg: Register::A },
            Instruction::Halt,
        ]);
    });
    assert_eq!(
        verify_now(&staged, &store, &PackageRevocationStore::new(), grants, 0),
        Err(PhaseStackError::UndeclaredSyscall)
    );
}

#[test]
fn n43_undeclared_mmio_refused() {
    use crate::kernel::phasestack::verifier::tests::signed_gpa_custom;
    let (staged, store, grants) = signed_gpa_custom(|m, _code| {
        m.mmio_caps = 1;
    });
    assert_eq!(
        verify_now(&staged, &store, &PackageRevocationStore::new(), grants, 0),
        Err(PhaseStackError::UndeclaredMmio)
    );
}

#[test]
fn n44_excessive_memory_refused() {
    use crate::kernel::phasestack::parser::{decode_manifest, encode_manifest, CanonicalManifest};
    let mut m = CanonicalManifest::phase_vm(
        "x".into(),
        types::AppVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
    );
    m.mem_bytes = types::PHASEVM_MAX_MEM + 1;
    assert_eq!(
        decode_manifest(&encode_manifest(&m)),
        Err(PhaseStackError::ExcessiveMemory)
    );
}

#[test]
fn n45_bad_entry_refused() {
    use crate::kernel::phasestack::verifier::tests::signed_gpa_custom;
    let (staged, store, grants) = signed_gpa_custom(|m, _code| {
        m.entry_point = 4;
    });
    assert_eq!(
        verify_now(&staged, &store, &PackageRevocationStore::new(), grants, 0),
        Err(PhaseStackError::BadEntry)
    );
}

#[test]
fn n46_wrong_arch_refused() {
    use crate::kernel::phasestack::parser::{decode_manifest, encode_manifest, CanonicalManifest};
    let mut m = CanonicalManifest::phase_vm(
        "x".into(),
        types::AppVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
    );
    m.arch = types::ARCH_NATIVE;
    assert_eq!(
        decode_manifest(&encode_manifest(&m)),
        Err(PhaseStackError::WrongArch)
    );
}

#[test]
fn n47_duplicate_package_id() {
    let mut kernel = Kernel::new();
    let (staged, store, grants) = signed_gpa(true);
    kernel.phasestack.certs = store;
    install::install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
    assert_eq!(
        install::install(&mut kernel, &staged, grants, InstallScope::User),
        Err(PhaseStackError::AlreadyInstalled)
    );
}

#[test]
fn n48_launch_after_revoke() {
    let mut kernel = Kernel::new();
    let (staged, store, grants) = signed_gpa(true);
    kernel.phasestack.certs = store;
    let inst = install::install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
    let app_id = inst.package().app_id();
    install::revoke(&mut kernel, app_id).unwrap();
    assert_eq!(
        install::launch(&mut kernel, app_id, 8).unwrap_err(),
        PhaseStackError::Revoked
    );
}

#[test]
fn n49_rollback_version() {
    use crate::kernel::phasestack::types::AppVersion;
    let (staged, store, grants) = signed_gpa(true);
    let older = AppVersion {
        major: 2,
        minor: 0,
        patch: 0,
    };
    assert_eq!(
        verify_package(VerifyInputs {
            staged: &staged,
            certs: &store,
            revocation: &PackageRevocationStore::new(),
            now_generation: 0,
            grants,
            scope: InstallScope::User,
            installed_version: Some((older, 1)),
        }),
        Err(PhaseStackError::RollbackVersion)
    );
}

#[test]
fn n50_launch_after_quarantine() {
    let mut kernel = Kernel::new();
    let (staged, store, grants) = signed_gpa(true);
    kernel.phasestack.certs = store;
    let inst = install::install(&mut kernel, &staged, grants, InstallScope::User).unwrap();
    let app_id = inst.package().app_id();
    kernel
        .phasestack
        .registry
        .set_state(app_id, AppState::Quarantined)
        .unwrap();
    assert_eq!(
        install::launch(&mut kernel, app_id, 8).unwrap_err(),
        PhaseStackError::Quarantined
    );
}
