//! GPC1 secret-backed checkpoint authentication host tests.
//!
//! Independent K_checkpoint vector: `tests/vectors/checkpoint_auth_hkdf_hmac.py`.
//! TEST-ONLY secrets. Not production keys.

use kairos::core::gphase_core::PhaseState;
use kairos::kernel::checkpoint_auth::{
    decode_record, derive_k_checkpoint, encode_mac_body, encode_record, hmac_checkpoint,
    payload_root, parse_development_secret, seal_record, verify_record, zeroize,
    AuthenticatedCheckpointFields, CheckpointAuthController, CheckpointContext,
    CheckpointKeyBackend, CheckpointKeySource, EphemeralProvisionedSecret, HardwareBoundSecret,
    KeyError, LegacyIntegrityOnly, FORMAT_VERSION, HKDF_SALT,
};
use kairos::kernel::crypto::sign;
use kairos::kernel::hash::StateHash;
use kairos::kernel::state_proof::ObjectKind;
use kairos::kernel::state_root::SubsystemKind;
use kairos::kernel::statechain::StatechainLedger;
use kairos::kernel::statechain_checkpoint::{
    derive_signing_key, sign_checkpoint, verify_checkpoint,
};
use kairos::kernel::Kernel;
use kairos::userland::parser::{parse, Command};

const TEST_SECRET_A: [u8; 32] = [0x11; 32];
const TEST_SECRET_B: [u8; 32] = [0x22; 32];
const DEVICE_A: StateHash = [0xA1; 32];
const BOOT_A: StateHash = [0xB0; 32];
const KERNEL_ROOT: StateHash = [0x31; 32];
const EQUATION: StateHash = [0x32; 32];
const PRIME: StateHash = [0x33; 32];
const APP_ROOT: StateHash = [0x34; 32];
const KEY_EPOCH: u32 = 7;

/// Known-answer from independent Python (`tests/vectors/checkpoint_auth_hkdf_hmac.py`).
const CANONICAL_K_HEX: &str = "d4d2c36f81b69d2df94a019643582e8cce700a7421454b32ce1495960224bca6";

fn fixture_fields(seq: u64, prev: StateHash) -> AuthenticatedCheckpointFields {
    AuthenticatedCheckpointFields {
        version: FORMAT_VERSION,
        sequence: seq,
        previous_root: prev,
        kernel_root: KERNEL_ROOT,
        equation_root: EQUATION,
        boot_session_root: BOOT_A,
        chronos_tick: 11,
        kairos_tick: 22,
        key_epoch: KEY_EPOCH,
        key_backend: CheckpointKeyBackend::EphemeralProvisionedSecret,
        payload_root: payload_root(&PRIME, &APP_ROOT),
        device_id: DEVICE_A,
    }
}

fn ctx_a() -> CheckpointContext {
    CheckpointContext {
        device_id: DEVICE_A,
        boot_session_root: BOOT_A,
        key_epoch: KEY_EPOCH,
        key_backend: CheckpointKeyBackend::EphemeralProvisionedSecret,
    }
}

fn source_a() -> EphemeralProvisionedSecret<'static> {
    EphemeralProvisionedSecret::new(&TEST_SECRET_A)
}

fn hex32(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn independent_kdf_matches_library_and_is_not_public_equation_root() {
    let mut library = [0u8; 32];
    derive_k_checkpoint(&TEST_SECRET_A, &ctx_a(), &mut library);
    assert_eq!(hex32(&library), CANONICAL_K_HEX);
    let mut public_only = [0u8; 32];
    LegacyIntegrityOnly {
        equation_root: EQUATION,
    }
    .derive_checkpoint_key(&ctx_a(), &mut public_only)
    .unwrap();
    assert_ne!(library, public_only);
    assert_eq!(HKDF_SALT, b"GPOS/checkpoint/v1");
}

#[test]
fn valid_checkpoint_verifies() {
    let src = source_a();
    let fields = fixture_fields(0, [0u8; 32]);
    let record = seal_record(&src, &ctx_a(), &fields).unwrap();
    let got = verify_record(&src, &ctx_a(), &record).unwrap();
    assert_eq!(got, fields);
    let (_f, tag) = decode_record(&record).unwrap();
    assert_eq!(tag.len(), 32);
    assert!(!record.windows(32).any(|w| w == TEST_SECRET_A));
}

fn tamper(
    src: &EphemeralProvisionedSecret<'_>,
    record: &[u8],
    mutate: impl Fn(&mut AuthenticatedCheckpointFields),
) {
    let (mut fields, tag) = decode_record(record).unwrap();
    mutate(&mut fields);
    let tampered = encode_record(&fields, &tag);
    assert!(verify_record(src, &ctx_a(), &tampered).is_err());
}

#[test]
fn every_canonical_field_modified_fails() {
    let src = source_a();
    let record = seal_record(&src, &ctx_a(), &fixture_fields(0, [0u8; 32])).unwrap();
    tamper(&src, &record, |f| f.version = 99);
    tamper(&src, &record, |f| f.sequence ^= 1);
    tamper(&src, &record, |f| f.previous_root[0] ^= 1);
    tamper(&src, &record, |f| f.kernel_root[0] ^= 1);
    tamper(&src, &record, |f| f.equation_root[0] ^= 1);
    tamper(&src, &record, |f| f.boot_session_root[0] ^= 1);
    tamper(&src, &record, |f| f.chronos_tick ^= 1);
    tamper(&src, &record, |f| f.kairos_tick ^= 1);
    tamper(&src, &record, |f| f.key_epoch ^= 1);
    tamper(&src, &record, |f| {
        f.key_backend = CheckpointKeyBackend::LegacyIntegrityOnly
    });
    tamper(&src, &record, |f| f.payload_root[0] ^= 1);
    tamper(&src, &record, |f| f.device_id[0] ^= 1);
    let (fields, mut tag) = decode_record(&record).unwrap();
    tag[0] ^= 1;
    let tampered = encode_record(&fields, &tag);
    assert_eq!(
        verify_record(&src, &ctx_a(), &tampered),
        Err(KeyError::BadSignature)
    );
}

#[test]
fn public_equation_root_cannot_forge_authenticated_checkpoint() {
    let src = source_a();
    let fields = fixture_fields(0, [0u8; 32]);
    let record = seal_record(&src, &ctx_a(), &fields).unwrap();
    let public = derive_signing_key(&EQUATION);
    let body = encode_mac_body(&fields);
    let forged_tag = hmac_checkpoint(&public, &body);
    let forged = encode_record(&fields, &forged_tag);
    assert_eq!(
        verify_record(&src, &ctx_a(), &forged),
        Err(KeyError::BadSignature)
    );
    assert_ne!(decode_record(&record).unwrap().1, forged_tag);
}

#[test]
fn old_public_derived_v1_forgery_fails_against_authenticated_backend() {
    let public = derive_signing_key(&EQUATION);
    let v1 = sign_checkpoint(&public, 0, 1, &KERNEL_ROOT, &EQUATION, &PRIME);
    assert!(verify_checkpoint(&public, &v1));
    let legacy = LegacyIntegrityOnly {
        equation_root: EQUATION,
    };
    assert_eq!(
        seal_record(&legacy, &ctx_a(), &fixture_fields(0, [0u8; 32])),
        Err(KeyError::AuthUnavailable)
    );
}

#[test]
fn correct_secret_verifies_different_secret_is_bad_signature() {
    let src_a = source_a();
    let src_b = EphemeralProvisionedSecret::new(&TEST_SECRET_B);
    let record = seal_record(&src_a, &ctx_a(), &fixture_fields(0, [0u8; 32])).unwrap();
    verify_record(&src_a, &ctx_a(), &record).unwrap();
    assert_eq!(
        verify_record(&src_b, &ctx_a(), &record),
        Err(KeyError::BadSignature)
    );
}

#[test]
fn missing_secret_refuses_sealing() {
    let mut ctrl = CheckpointAuthController::new();
    assert_eq!(ctrl.backend(), CheckpointKeyBackend::AuthUnavailable);
    assert!(ctrl.status_text().contains("KEY_BACKEND: AUTH_UNAVAILABLE"));
    assert_eq!(
        ctrl.seal(BOOT_A, KERNEL_ROOT, EQUATION, 11, 22, APP_ROOT),
        Err(KeyError::AuthUnavailable)
    );
}

#[test]
fn zero_length_secret_refuses() {
    assert_eq!(parse_development_secret(""), Err(KeyError::BadSecret));
    assert_eq!(parse_development_secret("   "), Err(KeyError::BadSecret));
    let mut ctrl = CheckpointAuthController::new();
    assert_eq!(
        ctrl.provision_from_line(""),
        Err(KeyError::BadSecret)
    );
}

#[test]
fn truncated_or_extended_tag_fails() {
    let src = source_a();
    let record = seal_record(&src, &ctx_a(), &fixture_fields(0, [0u8; 32])).unwrap();
    assert_eq!(
        verify_record(&src, &ctx_a(), &record[..record.len() - 1]),
        Err(KeyError::Truncated)
    );
    let mut extended = record.clone();
    extended.extend_from_slice(&[1, 2, 3, 4]);
    assert_eq!(
        verify_record(&src, &ctx_a(), &extended),
        Err(KeyError::Extended)
    );
}

#[test]
fn hmac_tag_is_32_bytes_and_ed25519_does_not_accept_it() {
    let src = source_a();
    let fields = fixture_fields(0, [0u8; 32]);
    let record = seal_record(&src, &ctx_a(), &fields).unwrap();
    let (_f, tag) = decode_record(&record).unwrap();
    assert_eq!(tag.len(), 32);
    let body = encode_mac_body(&fields);
    let kp = sign::SignKeypair::from_seed(&TEST_SECRET_A);
    let ed = kp.sign(&body);
    assert_eq!(ed.len(), 64);
    let mut padded = [0u8; 64];
    padded[..32].copy_from_slice(&tag);
    assert!(!sign::verify(&kp.public_bytes(), &body, &padded));
}

#[cfg(feature = "phasesentinel")]
#[test]
fn psc1_remains_ed25519() {
    use kairos::kernel::sentinel::SentinelCheckpoint;
    let mut psc1 = SentinelCheckpoint {
        generation: 1,
        content_root: KERNEL_ROOT,
        r_state: KERNEL_ROOT,
        r_memory: KERNEL_ROOT,
        r_program: KERNEL_ROOT,
        r_ps: [0u8; 32],
        signature: [0u8; 64],
    };
    assert!(psc1.canonical_tbs().starts_with(b"PSC1"));
    psc1.sign(&TEST_SECRET_A);
    let pk = sign::SignKeypair::from_seed(&TEST_SECRET_A).public_bytes();
    assert!(psc1.verify(&pk).is_ok());
    assert_eq!(psc1.signature.len(), 64);
}

#[cfg(feature = "identity")]
#[test]
fn identity_certificates_remain_ed25519() {
    use kairos::kernel::identity::certificate::{
        sign_certificate, verify_certificate_signature, UserCertificateV1,
        CERTIFICATE_VERSION_V1, CERT_FLAG_CAN_ISSUE, CERT_FLAG_ROOT, NO_ISSUER, SCOPE_HOST,
    };
    use kairos::kernel::identity::principal::PrincipalKind;
    let kp = sign::SignKeypair::from_seed(&TEST_SECRET_A);
    let mut cert = UserCertificateV1 {
        version: CERTIFICATE_VERSION_V1,
        serial: 1,
        subject: 0,
        subject_kind: PrincipalKind::Root,
        subject_public_key: kp.public_bytes(),
        issuer: 0,
        issuer_certificate_hash: NO_ISSUER,
        policy_root: [0u8; 32],
        authority_generation_start: 0,
        authority_generation_end: None,
        allowed_scope: SCOPE_HOST,
        flags: CERT_FLAG_ROOT | CERT_FLAG_CAN_ISSUE,
        signature: [0u8; 64],
    };
    cert.signature = sign_certificate(&cert, &kp);
    assert_eq!(cert.signature.len(), 64);
    assert!(verify_certificate_signature(&cert, &kp.public_bytes()));
    let mut hmac_as_sig = [0u8; 64];
    hmac_as_sig[..32].copy_from_slice(&hmac_checkpoint(&TEST_SECRET_A, b"not-a-cert"));
    cert.signature = hmac_as_sig;
    assert!(!verify_certificate_signature(&cert, &kp.public_bytes()));
}

#[test]
fn derived_key_is_zeroized() {
    let mut k = [0xAAu8; 32];
    derive_k_checkpoint(&TEST_SECRET_A, &ctx_a(), &mut k);
    assert_ne!(k, [0u8; 32]);
    zeroize(&mut k);
    assert_eq!(k, [0u8; 32]);
}

#[test]
fn status_and_seal_text_do_not_contain_secret() {
    let mut kernel = Kernel::new();
    kernel.checkpoint_auth.set_device_id(DEVICE_A);
    kernel.checkpoint_auth.provision_secret(&TEST_SECRET_A).unwrap();
    let status = kernel.checkpoint_auth_status_text();
    let secret_hex: String = TEST_SECRET_A.iter().map(|b| format!("{b:02x}")).collect();
    assert!(!status.contains(&secret_hex));
    assert!(!status.contains("1111111111111111"));
    assert!(status.contains("EPHEMERAL_PROVISIONED_SECRET"));
    assert!(status.contains("HARDWARE_BOUND: NO"));
}

#[test]
fn hardware_bound_is_unavailable() {
    let hw = HardwareBoundSecret;
    assert_eq!(
        hw.backend_id(),
        CheckpointKeyBackend::HardwareBoundSecret
    );
    let mut out = [0u8; 32];
    assert_eq!(
        hw.derive_checkpoint_key(&ctx_a(), &mut out),
        Err(KeyError::Unavailable)
    );
    let kernel = Kernel::new();
    let text = kernel.checkpoint_auth_hardware_text();
    assert!(text.contains("AUTH_UNAVAILABLE"));
    assert!(text.contains("HARDWARE_BOUND: NO"));
    assert!(text.contains("custom-board key storage"));
}

#[test]
fn provision_via_argv_is_refused_by_parser() {
    match parse("checkpoint-auth provision aabbccddeeff") {
        Command::Usage(msg) => assert!(msg.contains("not argv")),
        other => panic!("expected usage, got {other:?}"),
    }
    assert_eq!(
        parse("checkpoint-auth provision"),
        Command::CheckpointAuthProvision
    );
    assert_eq!(
        parse("checkpoint-auth status"),
        Command::CheckpointAuthStatus
    );
}

#[test]
fn kernel_seal_verify_wrong_secret() {
    let mut kernel = Kernel::new();
    kernel
        .statechain
        .record_transition(
            ObjectKind::Process,
            0,
            SubsystemKind::Process,
            PhaseState::Phi0,
            PhaseState::Phi1,
            "wake",
            None,
            b"",
        )
        .unwrap();
    kernel.checkpoint_auth.set_device_id(DEVICE_A);
    kernel.authority.boot_session_root = BOOT_A;
    kernel
        .checkpoint_auth
        .provision_secret(&TEST_SECRET_A)
        .unwrap();
    let record = kernel.checkpoint_auth_seal().unwrap();
    kernel.checkpoint_auth_verify_bytes(&record).unwrap();
    kernel
        .checkpoint_auth
        .provision_secret(&TEST_SECRET_B)
        .unwrap();
    assert_eq!(
        kernel.checkpoint_auth_verify_bytes(&record),
        Err(KeyError::BadSignature)
    );
}

#[test]
fn v1_integrity_only_still_verifies_on_legacy_path() {
    let mut ledger = StatechainLedger::new();
    ledger
        .record_transition(
            ObjectKind::Process,
            0,
            SubsystemKind::Process,
            PhaseState::Phi0,
            PhaseState::Phi1,
            "wake",
            None,
            b"",
        )
        .unwrap();
    let cp = ledger.seal_checkpoint().unwrap();
    assert!(verify_checkpoint(ledger.signing_key(), &cp));
    let kernel = Kernel::new();
    assert!(kernel
        .checkpoint_auth_status_text()
        .contains("KEY_BACKEND: AUTH_UNAVAILABLE"));
}
