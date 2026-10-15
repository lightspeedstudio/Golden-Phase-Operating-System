//! Per-object ANOM protection integration tests.

use kairos::core::gphase_core::PhaseState;
use kairos::kernel::anom::object_key::{
    derive_wrap_key, generate_object_data_key, generate_object_salt, unwrap_object_key,
    wrap_object_key, OBJECT_SALT_SIZE,
};
use kairos::kernel::anom::object_policy::{
    ObjectProtectionError, ProtectedObjectId, ProtectedObjectKind, OBJECT_PROTECTION_VERSION,
};
use kairos::kernel::anom::object_protection::{
    close_object, open_object, protect_object, rekey_object, unprotect_object,
};
use kairos::kernel::anom::object_record::{
    open_protected_phase, seal_protected_phase, ObjectStateAadContext, ProtectedPhasePlaintext,
};
use kairos::kernel::anom::object_registry::ObjectProtectionRegistry;
use kairos::kernel::authority::{AuthorityMode, PublicPhaseView, VisibilityMode};
use kairos::kernel::state_proof::ObjectKind;
use kairos::Runtime;

const TEST_PASS: &str = "kairos-object-test-pass";
const NEW_PASS: &str = "kairos-object-new-pass";

fn booted() -> Runtime {
    let _ = std::fs::remove_dir_all(".kairos/anom/objects");
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    rt
}

fn booted_with_input(lines: &[&str]) -> Runtime {
    let _ = std::fs::remove_dir_all(".kairos/anom/objects");
    let mut rt = Runtime::with_input(lines.iter().copied());
    rt.boot().unwrap();
    rt
}

fn run_shell(rt: &mut Runtime) {
    let pid = rt.hosted.init_pid.unwrap();
    rt.hosted.kernel.syscall(pid).unwrap();
    rt.hosted.kernel.kreturn(pid).unwrap();
    rt.hosted.kernel.schedule(pid).unwrap();
    rt.run().unwrap();
}

#[test]
fn protect_valid_process_succeeds() {
    let mut rt = booted();
    let canonical =
        protect_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap();
    assert_eq!(canonical, "process:0");
    assert!(rt
        .hosted
        .kernel
        .object_protection
        .registry
        .is_protected(&ProtectedObjectId::new(ProtectedObjectKind::Process, "0")));
}

#[test]
fn protect_missing_object_fails() {
    let mut rt = booted();
    let err =
        protect_object(&mut rt.hosted.kernel, "process:999", TEST_PASS.as_bytes()).unwrap_err();
    assert_eq!(err, ObjectProtectionError::ObjectNotFound);
}

#[test]
fn protect_already_protected_fails() {
    let mut rt = booted();
    protect_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap();
    let err = protect_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap_err();
    assert_eq!(err, ObjectProtectionError::AlreadyProtected);
}

#[test]
fn passphrase_confirmation_mismatch_via_shell() {
    let mut rt = booted_with_input(&["anom protect process:0 -p", "pass-a", "pass-b"]);
    run_shell(&mut rt);
    let out = rt.output_lines().join("\n");
    assert!(out.contains("passphrase confirmation mismatch"));
}

#[test]
fn persisted_record_has_no_passphrase_or_odk() {
    let mut rt = booted();
    protect_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap();
    let record = rt
        .hosted
        .kernel
        .object_protection
        .registry
        .record(&ProtectedObjectId::new(ProtectedObjectKind::Process, "0"))
        .unwrap();
    let serialized = std::fs::read(format!(
        ".kairos/anom/objects/{}",
        ObjectProtectionRegistry::filename_for(&record.object_id)
    ))
    .unwrap();
    let text = String::from_utf8_lossy(&serialized);
    assert!(!text.contains(TEST_PASS));
    assert!(!record.wrapped_object_key.is_empty());
    assert_ne!(record.object_salt.len(), 0);
}

#[test]
fn protected_process_shows_commitment() {
    let mut rt = booted();
    protect_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap();
    let view = rt.hosted.kernel.public_process_view(0);
    match view {
        PublicPhaseView::AnomCommitment(_) => {}
        other => panic!("expected commitment, got {other:?}"),
    }
}

#[test]
fn kernel_internal_state_still_available() {
    let mut rt = booted();
    protect_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap();
    let proc = rt.hosted.kernel.get_process(0).unwrap();
    assert_eq!(proc.state, PhaseState::Phi2);
}

#[test]
fn open_reveals_verified_phase() {
    let mut rt = booted();
    protect_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap();
    let (phase, _) = open_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap();
    assert_eq!(phase, PhaseState::Phi2);
}

#[test]
fn open_does_not_print_raw_key() {
    let mut rt = booted_with_input(&["anom open process:0 -p", TEST_PASS]);
    protect_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap();
    run_shell(&mut rt);
    let out = rt.output_lines().join("\n");
    assert!(!out.contains("object data key"));
    assert!(out.contains("true state:"));
}

#[test]
fn privileged_open_records_statechain_event() {
    let mut rt = booted();
    protect_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap();
    open_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap();
    let chain = rt
        .hosted
        .kernel
        .statechain
        .chain(ObjectKind::Process, 0)
        .unwrap();
    assert!(chain
        .events
        .iter()
        .any(|e| e.proof.operation_name == "privileged_object_open"));
}

#[test]
fn failed_open_records_event() {
    let mut rt = booted();
    protect_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap();
    let _ = open_object(&mut rt.hosted.kernel, "process:0", b"wrong-passphrase");
    let chain = rt
        .hosted
        .kernel
        .statechain
        .chain(ObjectKind::Process, 0)
        .unwrap();
    assert!(chain
        .events
        .iter()
        .any(|e| e.proof.operation_name == "failed_object_open"));
}

#[test]
fn close_clears_operator_access() {
    let mut rt = booted();
    protect_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap();
    open_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap();
    close_object(&mut rt.hosted.kernel, "process:0").unwrap();
    let id = ProtectedObjectId::new(ProtectedObjectKind::Process, "0");
    assert!(!rt.hosted.kernel.object_protection.access.is_open(&id));
}

#[test]
fn rekey_accepts_new_passphrase() {
    let mut rt = booted();
    protect_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap();
    rekey_object(
        &mut rt.hosted.kernel,
        "process:0",
        TEST_PASS.as_bytes(),
        NEW_PASS.as_bytes(),
    )
    .unwrap();
    assert!(open_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).is_err());
    let (phase, _) = open_object(&mut rt.hosted.kernel, "process:0", NEW_PASS.as_bytes()).unwrap();
    assert_eq!(phase, PhaseState::Phi2);
}

#[test]
fn unprotect_keeps_historical_records() {
    let mut rt = booted();
    protect_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap();
    let count_before = rt
        .hosted
        .kernel
        .object_protection
        .registry
        .phase_records(&ProtectedObjectId::new(ProtectedObjectKind::Process, "0"))
        .len();
    unprotect_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap();
    let count_after = rt
        .hosted
        .kernel
        .object_protection
        .registry
        .phase_records(&ProtectedObjectId::new(ProtectedObjectKind::Process, "0"))
        .len();
    assert_eq!(count_before, count_after);
    assert!(!rt
        .hosted
        .kernel
        .object_protection
        .registry
        .is_protected(&ProtectedObjectId::new(ProtectedObjectKind::Process, "0")));
}

#[test]
fn protect_updates_subsystem_merkle() {
    let mut rt = booted();
    let root_before = rt.hosted.kernel.statechain.kernel_root();
    protect_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap();
    assert_ne!(rt.hosted.kernel.statechain.kernel_root(), root_before);
}

#[test]
fn boot_splash_shows_protected_object_count() {
    let mut rt = booted();
    protect_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap();
    let pid = rt.hosted.init_pid.unwrap();
    let info = kairos::userland::splash::BootSplashInfo::from_kernel(&rt.hosted.kernel, pid, true);
    let text = kairos::userland::splash::render_boot_screen(&info);
    assert!(text.contains("protected objects:      1"));
}

#[test]
fn anom_status_object_works() {
    let mut rt = booted_with_input(&["anom status process:0"]);
    protect_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap();
    run_shell(&mut rt);
    let out = rt.output_lines().join("\n");
    assert!(out.contains("protected/locked"));
    assert!(out.contains("chain: OK"));
}

#[test]
fn anom_list_works() {
    let mut rt = booted_with_input(&["anom list"]);
    protect_object(&mut rt.hosted.kernel, "process:0", TEST_PASS.as_bytes()).unwrap();
    run_shell(&mut rt);
    let out = rt.output_lines().join("\n");
    assert!(out.contains("process:0"));
    assert!(out.contains("ANOM:"));
}

#[test]
fn global_anom_mode_still_works() {
    let mut rt = booted();
    kairos::kernel::anom::enter_anom_mode(&mut rt.hosted.kernel, b"kairos-anom-test-passphrase")
        .unwrap();
    assert_eq!(rt.hosted.kernel.authority.authority, AuthorityMode::Anom);
}

#[test]
fn root_transparent_mode_still_works() {
    let rt = booted();
    assert_eq!(rt.hosted.kernel.authority.authority, AuthorityMode::Root);
    assert_eq!(
        rt.hosted.kernel.authority.visibility,
        VisibilityMode::Transparent
    );
}

fn sample_aad_ctx(id: &ProtectedObjectId) -> ObjectStateAadContext {
    ObjectStateAadContext {
        equation_root: [1u8; 32],
        boot_session_root: [2u8; 32],
        object_id: id.clone(),
        protection_version: OBJECT_PROTECTION_VERSION,
        sequence_number: 0,
        previous_event_hash: [0u8; 32],
        operation: "test".into(),
        public_commitment: [0u8; 32],
        chronos_tick: 10,
        kairos_tick: 5,
        sector_id: 3,
    }
}

#[test]
fn phase_record_round_trip() {
    let id = ProtectedObjectId::new(ProtectedObjectKind::Process, "7");
    let odk = generate_object_data_key();
    let nonce = [9u8; 12];
    let mut ctx = sample_aad_ctx(&id);
    let plain = ProtectedPhasePlaintext::from_clock(
        PhaseState::Phi1,
        PhaseState::Phi2,
        "test",
        &kairos::StateClockSnapshot {
            chronos_tick: 10,
            kairos_tick: 5,
            proper_tick: 10,
            state_tick: 10,
            epoch: 0,
            phi: 0.0,
            delta_phi: 0.0,
            sector_id: 3,
            epsilon: 0.0,
            x: 0.0,
            dilation_factor: 1.0,
            skew: 0,
        },
        vec![],
        vec![1, 2, 3],
    );
    let sealed = seal_protected_phase(&odk, &nonce, &plain, &ctx).unwrap();
    ctx.public_commitment = sealed.public_commitment;
    ctx.sequence_number = sealed.sequence_number;
    let opened = open_protected_phase(&odk, &sealed, &ctx).unwrap();
    assert_eq!(opened.true_phase, PhaseState::Phi2);
}

#[test]
fn tamper_equation_root_fails_decrypt() {
    let id = ProtectedObjectId::new(ProtectedObjectKind::Process, "7");
    let odk = generate_object_data_key();
    let nonce = [1u8; 12];
    let ctx = sample_aad_ctx(&id);
    let plain = ProtectedPhasePlaintext {
        previous_phase: PhaseState::Phi1,
        true_phase: PhaseState::Phi2,
        operation: "test".into(),
        proper_tick: 10,
        kairos_tick: 5,
        sector_id: 3,
        private_trajectory: vec![],
        random_padding: vec![0],
    };
    let sealed = seal_protected_phase(&odk, &nonce, &plain, &ctx).unwrap();
    let mut bad = ctx.clone();
    bad.equation_root = [99u8; 32];
    bad.public_commitment = sealed.public_commitment;
    assert!(open_protected_phase(&odk, &sealed, &bad).is_err());
}

#[test]
fn tamper_boot_session_root_fails_decrypt() {
    let id = ProtectedObjectId::new(ProtectedObjectKind::Process, "7");
    let odk = generate_object_data_key();
    let nonce = [2u8; 12];
    let ctx = sample_aad_ctx(&id);
    let plain = ProtectedPhasePlaintext {
        previous_phase: PhaseState::Phi1,
        true_phase: PhaseState::Phi2,
        operation: "test".into(),
        proper_tick: 10,
        kairos_tick: 5,
        sector_id: 3,
        private_trajectory: vec![],
        random_padding: vec![0],
    };
    let sealed = seal_protected_phase(&odk, &nonce, &plain, &ctx).unwrap();
    let mut bad = ctx.clone();
    bad.boot_session_root = [88u8; 32];
    bad.public_commitment = sealed.public_commitment;
    assert!(open_protected_phase(&odk, &sealed, &bad).is_err());
}

#[test]
fn tamper_object_id_fails_decrypt() {
    let id = ProtectedObjectId::new(ProtectedObjectKind::Process, "7");
    let odk = generate_object_data_key();
    let nonce = [3u8; 12];
    let ctx = sample_aad_ctx(&id);
    let plain = ProtectedPhasePlaintext {
        previous_phase: PhaseState::Phi1,
        true_phase: PhaseState::Phi2,
        operation: "test".into(),
        proper_tick: 10,
        kairos_tick: 5,
        sector_id: 3,
        private_trajectory: vec![],
        random_padding: vec![0],
    };
    let sealed = seal_protected_phase(&odk, &nonce, &plain, &ctx).unwrap();
    let mut bad = ctx.clone();
    bad.object_id = ProtectedObjectId::new(ProtectedObjectKind::Process, "8");
    bad.public_commitment = sealed.public_commitment;
    assert!(open_protected_phase(&odk, &sealed, &bad).is_err());
}

#[test]
fn tamper_sequence_fails_decrypt() {
    let id = ProtectedObjectId::new(ProtectedObjectKind::Process, "7");
    let odk = generate_object_data_key();
    let nonce = [4u8; 12];
    let ctx = sample_aad_ctx(&id);
    let plain = ProtectedPhasePlaintext {
        previous_phase: PhaseState::Phi1,
        true_phase: PhaseState::Phi2,
        operation: "test".into(),
        proper_tick: 10,
        kairos_tick: 5,
        sector_id: 3,
        private_trajectory: vec![],
        random_padding: vec![0],
    };
    let sealed = seal_protected_phase(&odk, &nonce, &plain, &ctx).unwrap();
    let mut bad = ctx.clone();
    bad.sequence_number = 99;
    bad.public_commitment = sealed.public_commitment;
    assert!(open_protected_phase(&odk, &sealed, &bad).is_err());
}

#[test]
fn tamper_operation_fails_decrypt() {
    let id = ProtectedObjectId::new(ProtectedObjectKind::Process, "7");
    let odk = generate_object_data_key();
    let nonce = [5u8; 12];
    let ctx = sample_aad_ctx(&id);
    let plain = ProtectedPhasePlaintext {
        previous_phase: PhaseState::Phi1,
        true_phase: PhaseState::Phi2,
        operation: "test".into(),
        proper_tick: 10,
        kairos_tick: 5,
        sector_id: 3,
        private_trajectory: vec![],
        random_padding: vec![0],
    };
    let sealed = seal_protected_phase(&odk, &nonce, &plain, &ctx).unwrap();
    let mut bad = ctx.clone();
    bad.operation = "tampered".into();
    bad.public_commitment = sealed.public_commitment;
    assert!(open_protected_phase(&odk, &sealed, &bad).is_err());
}

#[test]
fn tamper_sector_fails_decrypt() {
    let id = ProtectedObjectId::new(ProtectedObjectKind::Process, "7");
    let odk = generate_object_data_key();
    let nonce = [6u8; 12];
    let ctx = sample_aad_ctx(&id);
    let plain = ProtectedPhasePlaintext {
        previous_phase: PhaseState::Phi1,
        true_phase: PhaseState::Phi2,
        operation: "test".into(),
        proper_tick: 10,
        kairos_tick: 5,
        sector_id: 3,
        private_trajectory: vec![],
        random_padding: vec![0],
    };
    let sealed = seal_protected_phase(&odk, &nonce, &plain, &ctx).unwrap();
    let mut bad = ctx.clone();
    bad.sector_id = 9;
    bad.public_commitment = sealed.public_commitment;
    assert!(open_protected_phase(&odk, &sealed, &bad).is_err());
}

#[test]
fn tamper_previous_hash_fails_decrypt() {
    let id = ProtectedObjectId::new(ProtectedObjectKind::Process, "7");
    let odk = generate_object_data_key();
    let nonce = [7u8; 12];
    let ctx = sample_aad_ctx(&id);
    let plain = ProtectedPhasePlaintext {
        previous_phase: PhaseState::Phi1,
        true_phase: PhaseState::Phi2,
        operation: "test".into(),
        proper_tick: 10,
        kairos_tick: 5,
        sector_id: 3,
        private_trajectory: vec![],
        random_padding: vec![0],
    };
    let sealed = seal_protected_phase(&odk, &nonce, &plain, &ctx).unwrap();
    let mut bad = ctx.clone();
    bad.previous_event_hash = [77u8; 32];
    bad.public_commitment = sealed.public_commitment;
    assert!(open_protected_phase(&odk, &sealed, &bad).is_err());
}

#[test]
fn tamper_ciphertext_fails_decrypt() {
    let id = ProtectedObjectId::new(ProtectedObjectKind::Process, "7");
    let odk = generate_object_data_key();
    let nonce = [8u8; 12];
    let ctx = sample_aad_ctx(&id);
    let plain = ProtectedPhasePlaintext {
        previous_phase: PhaseState::Phi1,
        true_phase: PhaseState::Phi2,
        operation: "test".into(),
        proper_tick: 10,
        kairos_tick: 5,
        sector_id: 3,
        private_trajectory: vec![],
        random_padding: vec![0],
    };
    let mut sealed = seal_protected_phase(&odk, &nonce, &plain, &ctx).unwrap();
    let mut bad_ctx = ctx.clone();
    bad_ctx.public_commitment = sealed.public_commitment;
    sealed.ciphertext[0] ^= 0xff;
    assert!(open_protected_phase(&odk, &sealed, &bad_ctx).is_err());
}

#[test]
fn wrap_key_derivation_differs_by_salt() {
    let eq = [1u8; 32];
    let bsr = [2u8; 32];
    let id = ProtectedObjectId::new(ProtectedObjectKind::Process, "0");
    let salt_a = generate_object_salt();
    let salt_b = generate_object_salt();
    let ka = derive_wrap_key(
        TEST_PASS.as_bytes(),
        &eq,
        &bsr,
        &id,
        &salt_a,
        OBJECT_PROTECTION_VERSION,
    );
    let kb = derive_wrap_key(
        TEST_PASS.as_bytes(),
        &eq,
        &bsr,
        &id,
        &salt_b,
        OBJECT_PROTECTION_VERSION,
    );
    assert_ne!(ka, kb);
    assert_eq!(salt_a.len(), OBJECT_SALT_SIZE);
}

#[test]
fn wrapped_key_open_round_trip() {
    let eq = [1u8; 32];
    let bsr = [2u8; 32];
    let id = ProtectedObjectId::new(ProtectedObjectKind::Process, "0");
    let salt = generate_object_salt();
    let odk = generate_object_data_key();
    let k_wrap = derive_wrap_key(
        TEST_PASS.as_bytes(),
        &eq,
        &bsr,
        &id,
        &salt,
        OBJECT_PROTECTION_VERSION,
    );
    let wrapped = wrap_object_key(
        &k_wrap,
        &odk,
        &eq,
        &bsr,
        &id,
        &salt,
        OBJECT_PROTECTION_VERSION,
    )
    .unwrap();
    let opened = unwrap_object_key(
        TEST_PASS.as_bytes(),
        &eq,
        &bsr,
        &id,
        &wrapped,
        OBJECT_PROTECTION_VERSION,
    )
    .unwrap();
    assert_eq!(opened, odk);
}

#[test]
fn wrong_passphrase_wrap_fails() {
    let eq = [1u8; 32];
    let bsr = [2u8; 32];
    let id = ProtectedObjectId::new(ProtectedObjectKind::Process, "0");
    let salt = generate_object_salt();
    let odk = generate_object_data_key();
    let k_wrap = derive_wrap_key(
        TEST_PASS.as_bytes(),
        &eq,
        &bsr,
        &id,
        &salt,
        OBJECT_PROTECTION_VERSION,
    );
    let wrapped = wrap_object_key(
        &k_wrap,
        &odk,
        &eq,
        &bsr,
        &id,
        &salt,
        OBJECT_PROTECTION_VERSION,
    )
    .unwrap();
    assert!(unwrap_object_key(
        b"wrong",
        &eq,
        &bsr,
        &id,
        &wrapped,
        OBJECT_PROTECTION_VERSION
    )
    .is_err());
}

use std::string::String;
