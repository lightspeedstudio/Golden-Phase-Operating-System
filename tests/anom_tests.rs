//! ANOM security mode — root transparent boot, optional post-boot protection.

use kairos::core::gphase_core::PhaseState;
use kairos::kernel::anom::decryptor::open_anom_phase_state;
use kairos::kernel::anom::protocol::{self, parse_root_manifest, ANOM_STATECHAIN_OBJECT_ID};
use kairos::kernel::anom::record::{build_aad, seal_anom_phase_state};
use kairos::kernel::anom::{enter_anom_mode, exit_anom_mode};
use kairos::kernel::authority::{AuthorityMode, PublicPhaseView, VisibilityMode};
use kairos::kernel::crypto::keyring::AnomKeyring;
use kairos::kernel::crypto::nonce::NonceGenerator;
use kairos::kernel::state_proof::ObjectKind;
use kairos::Runtime;

const TEST_PASS: &str = "kairos-anom-test-passphrase";

fn booted() -> Runtime {
    let mut rt = Runtime::new();
    rt.boot().unwrap();
    rt
}

fn booted_with_pass_input(extra: &[&str]) -> Runtime {
    let mut lines = vec!["anom security -p", TEST_PASS];
    lines.extend_from_slice(extra);
    let mut rt = Runtime::with_input(lines);
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
fn boots_authority_root() {
    let rt = booted();
    assert_eq!(rt.hosted.kernel.authority.authority, AuthorityMode::Root);
}

#[test]
fn boots_visibility_transparent() {
    let rt = booted();
    assert_eq!(
        rt.hosted.kernel.authority.visibility,
        VisibilityMode::Transparent
    );
}

#[test]
fn boot_splash_shows_authority_root() {
    let rt = booted();
    let pid = rt.hosted.init_pid.unwrap();
    let info = kairos::userland::splash::BootSplashInfo::from_kernel(&rt.hosted.kernel, pid, true);
    let text = kairos::userland::splash::render_boot_screen(&info);
    assert!(text.contains("authority:           root"));
}

#[test]
fn boot_splash_shows_anom_standby() {
    let rt = booted();
    let pid = rt.hosted.init_pid.unwrap();
    let info = kairos::userland::splash::BootSplashInfo::from_kernel(&rt.hosted.kernel, pid, true);
    let text = kairos::userland::splash::render_boot_screen(&info);
    assert!(text.contains("security protocol:   standby"));
}

#[test]
fn anom_security_enters_anom_mode() {
    let mut rt = booted_with_pass_input(&["anom status", "exit"]);
    run_shell(&mut rt);
    let out = rt.output_lines().join("\n");
    assert!(out.contains("ANOM security mode"));
    assert_eq!(rt.hosted.kernel.authority.authority, AuthorityMode::Anom);
}

#[test]
fn enter_anom_records_statechain_event() {
    let mut rt = booted();
    enter_anom_mode(&mut rt.hosted.kernel, TEST_PASS.as_bytes()).unwrap();
    let chain = rt
        .hosted
        .kernel
        .statechain
        .chain(ObjectKind::Driver, ANOM_STATECHAIN_OBJECT_ID)
        .unwrap();
    assert!(chain
        .events
        .iter()
        .any(|e| e.proof.operation_name == "enter_anom_mode"));
}

#[test]
fn enter_anom_updates_statechain_head() {
    let mut rt = booted();
    let tick_before = rt.hosted.kernel.statechain.tick();
    enter_anom_mode(&mut rt.hosted.kernel, TEST_PASS.as_bytes()).unwrap();
    assert!(rt.hosted.kernel.statechain.tick() > tick_before);
}

#[test]
fn enter_anom_updates_kernel_root() {
    let mut rt = booted();
    let root_before = rt.hosted.kernel.statechain.kernel_root();
    enter_anom_mode(&mut rt.hosted.kernel, TEST_PASS.as_bytes()).unwrap();
    assert_ne!(rt.hosted.kernel.statechain.kernel_root(), root_before);
}

#[test]
fn public_phase_view_becomes_commitment_after_transition() {
    let mut rt = booted();
    enter_anom_mode(&mut rt.hosted.kernel, TEST_PASS.as_bytes()).unwrap();
    let child = rt.spawn_background().unwrap();
    rt.hosted.kernel.wake(child).unwrap();
    let view = rt.hosted.kernel.public_process_view(child);
    match view {
        PublicPhaseView::AnomCommitment(_) => {}
        other => panic!("expected commitment, got {other:?}"),
    }
}

#[test]
fn decryptor_recovers_true_phase() {
    let rt = booted();
    let equation = rt.hosted.kernel.statechain.equation_root();
    let bsr = rt.hosted.kernel.authority.boot_session_root;
    let keyring = AnomKeyring::derive(TEST_PASS.as_bytes(), &equation, &bsr);
    let mut nonce_gen = NonceGenerator::new([1u8; 32]);
    let ctx = protocol::aad_context_for_kernel(&rt.hosted.kernel);
    let record = seal_anom_phase_state(
        &keyring,
        &mut nonce_gen,
        PhaseState::Phi2,
        PhaseState::Phi1,
        ObjectKind::Process,
        7,
        0,
        "test",
        [0u8; 32],
        &ctx,
    )
    .unwrap();
    let mut aad_ctx = ctx;
    aad_ctx.object_kind = ObjectKind::Process;
    aad_ctx.object_id = 7;
    aad_ctx.sequence_number = 0;
    aad_ctx.commitment_hash = record.commitment_hash;
    aad_ctx.operation = "test".into();
    let opened = open_anom_phase_state(&keyring, &record, &aad_ctx).unwrap();
    assert_eq!(opened, PhaseState::Phi2);
}

#[test]
fn wrong_passphrase_cannot_exit() {
    let mut rt = booted();
    enter_anom_mode(&mut rt.hosted.kernel, TEST_PASS.as_bytes()).unwrap();
    assert!(exit_anom_mode(&mut rt.hosted.kernel, b"wrong-pass").is_err());
}

#[test]
fn tamper_equation_root_breaks_decrypt() {
    let rt = booted();
    let equation = rt.hosted.kernel.statechain.equation_root();
    let bsr = rt.hosted.kernel.authority.boot_session_root;
    let keyring = AnomKeyring::derive(TEST_PASS.as_bytes(), &equation, &bsr);
    let mut nonce_gen = NonceGenerator::new([2u8; 32]);
    let mut ctx = protocol::aad_context_for_kernel(&rt.hosted.kernel);
    let record = seal_anom_phase_state(
        &keyring,
        &mut nonce_gen,
        PhaseState::Phi2,
        PhaseState::Phi1,
        ObjectKind::Process,
        1,
        0,
        "op",
        [0u8; 32],
        &ctx,
    )
    .unwrap();
    ctx.commitment_hash = record.commitment_hash;
    ctx.equation_root[0] ^= 0xFF;
    assert!(open_anom_phase_state(&keyring, &record, &ctx).is_err());
}

#[test]
fn tamper_boot_session_root_breaks_decrypt() {
    let rt = booted();
    let equation = rt.hosted.kernel.statechain.equation_root();
    let bsr = rt.hosted.kernel.authority.boot_session_root;
    let keyring = AnomKeyring::derive(TEST_PASS.as_bytes(), &equation, &bsr);
    let mut nonce_gen = NonceGenerator::new([3u8; 32]);
    let mut ctx = protocol::aad_context_for_kernel(&rt.hosted.kernel);
    let record = seal_anom_phase_state(
        &keyring,
        &mut nonce_gen,
        PhaseState::Phi1,
        PhaseState::Phi0,
        ObjectKind::Process,
        2,
        0,
        "op",
        [0u8; 32],
        &ctx,
    )
    .unwrap();
    ctx.commitment_hash = record.commitment_hash;
    ctx.boot_session_root[0] ^= 0xFF;
    assert!(open_anom_phase_state(&keyring, &record, &ctx).is_err());
}

#[test]
fn tamper_object_id_breaks_decrypt() {
    let rt = booted();
    let equation = rt.hosted.kernel.statechain.equation_root();
    let bsr = rt.hosted.kernel.authority.boot_session_root;
    let keyring = AnomKeyring::derive(TEST_PASS.as_bytes(), &equation, &bsr);
    let mut nonce_gen = NonceGenerator::new([4u8; 32]);
    let mut ctx = protocol::aad_context_for_kernel(&rt.hosted.kernel);
    let record = seal_anom_phase_state(
        &keyring,
        &mut nonce_gen,
        PhaseState::Phi3,
        PhaseState::Phi2,
        ObjectKind::Process,
        3,
        0,
        "op",
        [0u8; 32],
        &ctx,
    )
    .unwrap();
    ctx.commitment_hash = record.commitment_hash;
    ctx.object_id = 99;
    assert!(open_anom_phase_state(&keyring, &record, &ctx).is_err());
}

#[test]
fn tamper_sequence_breaks_decrypt() {
    let rt = booted();
    let equation = rt.hosted.kernel.statechain.equation_root();
    let bsr = rt.hosted.kernel.authority.boot_session_root;
    let keyring = AnomKeyring::derive(TEST_PASS.as_bytes(), &equation, &bsr);
    let mut nonce_gen = NonceGenerator::new([5u8; 32]);
    let mut ctx = protocol::aad_context_for_kernel(&rt.hosted.kernel);
    let record = seal_anom_phase_state(
        &keyring,
        &mut nonce_gen,
        PhaseState::Phi2,
        PhaseState::Phi2,
        ObjectKind::Process,
        4,
        1,
        "op",
        [0u8; 32],
        &ctx,
    )
    .unwrap();
    ctx.commitment_hash = record.commitment_hash;
    ctx.sequence_number = 2;
    assert!(open_anom_phase_state(&keyring, &record, &ctx).is_err());
}

#[test]
fn tamper_operation_breaks_decrypt() {
    let rt = booted();
    let equation = rt.hosted.kernel.statechain.equation_root();
    let bsr = rt.hosted.kernel.authority.boot_session_root;
    let keyring = AnomKeyring::derive(TEST_PASS.as_bytes(), &equation, &bsr);
    let mut nonce_gen = NonceGenerator::new([6u8; 32]);
    let mut ctx = protocol::aad_context_for_kernel(&rt.hosted.kernel);
    let record = seal_anom_phase_state(
        &keyring,
        &mut nonce_gen,
        PhaseState::Phi1,
        PhaseState::Phi0,
        ObjectKind::Process,
        5,
        0,
        "wake",
        [0u8; 32],
        &ctx,
    )
    .unwrap();
    ctx.commitment_hash = record.commitment_hash;
    ctx.operation = "schedule".into();
    assert!(open_anom_phase_state(&keyring, &record, &ctx).is_err());
}

#[test]
fn tamper_sector_breaks_decrypt() {
    let rt = booted();
    let equation = rt.hosted.kernel.statechain.equation_root();
    let bsr = rt.hosted.kernel.authority.boot_session_root;
    let keyring = AnomKeyring::derive(TEST_PASS.as_bytes(), &equation, &bsr);
    let mut nonce_gen = NonceGenerator::new([7u8; 32]);
    let mut ctx = protocol::aad_context_for_kernel(&rt.hosted.kernel);
    let record = seal_anom_phase_state(
        &keyring,
        &mut nonce_gen,
        PhaseState::Phi2,
        PhaseState::Phi1,
        ObjectKind::Process,
        6,
        0,
        "op",
        [0u8; 32],
        &ctx,
    )
    .unwrap();
    ctx.commitment_hash = record.commitment_hash;
    ctx.sector_id = ctx.sector_id.wrapping_add(1);
    assert!(open_anom_phase_state(&keyring, &record, &ctx).is_err());
}

#[test]
fn tamper_ciphertext_breaks_decrypt() {
    let rt = booted();
    let equation = rt.hosted.kernel.statechain.equation_root();
    let bsr = rt.hosted.kernel.authority.boot_session_root;
    let keyring = AnomKeyring::derive(TEST_PASS.as_bytes(), &equation, &bsr);
    let mut nonce_gen = NonceGenerator::new([8u8; 32]);
    let mut ctx = protocol::aad_context_for_kernel(&rt.hosted.kernel);
    let mut record = seal_anom_phase_state(
        &keyring,
        &mut nonce_gen,
        PhaseState::Phi2,
        PhaseState::Phi1,
        ObjectKind::Process,
        8,
        0,
        "op",
        [0u8; 32],
        &ctx,
    )
    .unwrap();
    ctx.commitment_hash = record.commitment_hash;
    if let Some(b) = record.ciphertext.first_mut() {
        *b ^= 0x55;
    }
    assert!(open_anom_phase_state(&keyring, &record, &ctx).is_err());
}

#[test]
fn tamper_aad_hash_breaks_decrypt() {
    let rt = booted();
    let equation = rt.hosted.kernel.statechain.equation_root();
    let bsr = rt.hosted.kernel.authority.boot_session_root;
    let keyring = AnomKeyring::derive(TEST_PASS.as_bytes(), &equation, &bsr);
    let mut nonce_gen = NonceGenerator::new([9u8; 32]);
    let ctx = protocol::aad_context_for_kernel(&rt.hosted.kernel);
    let mut record = seal_anom_phase_state(
        &keyring,
        &mut nonce_gen,
        PhaseState::Phi0,
        PhaseState::Phi0,
        ObjectKind::Process,
        9,
        0,
        "op",
        [0u8; 32],
        &ctx,
    )
    .unwrap();
    record.aad_hash[0] ^= 0xAA;
    let mut aad_ctx = ctx;
    aad_ctx.commitment_hash = record.commitment_hash;
    assert!(open_anom_phase_state(&keyring, &record, &aad_ctx).is_err());
}

#[test]
fn anom_status_reports_mode() {
    let mut rt = booted_with_pass_input(&["anom status", "exit"]);
    run_shell(&mut rt);
    let out = rt.output_lines().join("\n");
    assert!(out.contains("authority: anom"));
    assert!(out.contains("visibility: protected"));
}

#[test]
fn anom_off_exits_with_correct_passphrase() {
    let mut rt = booted_with_pass_input(&["anom off -p", TEST_PASS, "exit"]);
    run_shell(&mut rt);
    assert_eq!(rt.hosted.kernel.authority.authority, AuthorityMode::Root);
}

#[test]
fn exit_anom_records_event() {
    let mut rt = booted();
    enter_anom_mode(&mut rt.hosted.kernel, TEST_PASS.as_bytes()).unwrap();
    exit_anom_mode(&mut rt.hosted.kernel, TEST_PASS.as_bytes()).unwrap();
    let chain = rt
        .hosted
        .kernel
        .statechain
        .chain(ObjectKind::Driver, ANOM_STATECHAIN_OBJECT_ID)
        .unwrap();
    assert!(chain
        .events
        .iter()
        .any(|e| e.proof.operation_name == "exit_anom_mode"));
}

#[test]
fn privileged_open_records_event() {
    let mut rt = booted();
    enter_anom_mode(&mut rt.hosted.kernel, TEST_PASS.as_bytes()).unwrap();
    let child = rt.spawn_background().unwrap();
    rt.hosted.kernel.schedule(child).unwrap();
    protocol::privileged_open(
        &mut rt.hosted.kernel,
        &format!("process:{child}"),
        TEST_PASS.as_bytes(),
    )
    .unwrap();
    let chain = rt
        .hosted
        .kernel
        .statechain
        .chain(ObjectKind::Driver, ANOM_STATECHAIN_OBJECT_ID)
        .unwrap();
    assert!(chain
        .events
        .iter()
        .any(|e| e.proof.operation_name == "privileged_anom_open"));
}

#[cfg(not(feature = "std"))]
#[test]
fn baremetal_anom_crypto_available_after_boot() {
    let mut kernel = kairos::Kernel::new();
    finalize_boot(&mut kernel, 0);
    assert!(kernel.anom.is_crypto_available());
}

#[cfg(feature = "std")]
#[test]
fn hosted_anom_crypto_available_after_boot() {
    let rt = booted();
    assert!(rt.hosted.kernel.anom.is_crypto_available());
}

#[test]
fn boot_never_starts_in_anom() {
    let rt = booted();
    assert_ne!(rt.hosted.kernel.authority.authority, AuthorityMode::Anom);
    assert!(!rt.hosted.kernel.anom.is_active());
}

#[test]
fn prior_anom_history_displayed_not_active() {
    let mut history = kairos::kernel::authority::RootFileHistory::default();
    let manifest = b"last_authority_mode=anom\nlast_visibility_mode=protected\n";
    parse_root_manifest(manifest, &mut history);
    assert_eq!(history.last_authority_mode, AuthorityMode::Anom);
    let rt = booted();
    assert_eq!(rt.hosted.kernel.authority.authority, AuthorityMode::Root);
}

#[test]
fn build_aad_includes_domain_tag() {
    let rt = booted();
    let ctx = protocol::aad_context_for_kernel(&rt.hosted.kernel);
    let aad = build_aad(&ctx);
    assert!(aad.starts_with(b"KAIROS_ANOM_PHASE_V1"));
}
