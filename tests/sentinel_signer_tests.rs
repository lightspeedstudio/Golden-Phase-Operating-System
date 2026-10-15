use kairos::kernel::crypto::phase_hash::ph_content;
use kairos::kernel::crypto::sign::SignKeypair;
use kairos::kernel::sentinel::launch::authorize_launch;
use kairos::kernel::sentinel::manifest::ProgramManifest;
use kairos::kernel::sentinel::signer::SentinelSigner;
use kairos::kernel::sentinel::{PhaseSentinel, SentinelError};

fn seed_a() -> [u8; 32] {
    [0x11; 32]
}

fn signed_manifest(sentinel: &mut PhaseSentinel, content: &[u8]) -> ProgramManifest {
    let kp = SignKeypair::from_seed(&seed_a());
    let pk = kp.public_bytes();
    sentinel.signers.pin_root(pk);
    sentinel.policy.max_rights = u32::MAX;
    sentinel.policy.min_generation = 0;
    if !sentinel.policy.allows_signer(&pk) {
        sentinel.policy.allowed_signers.push(pk);
    }
    let mut m = ProgramManifest {
        generation: 1,
        expiry_tick: u64::MAX,
        content_hash: ph_content(content),
        requested_rights: 1,
        signer_pubkey: pk,
        signature: [0u8; 64],
    };
    m.sign(&seed_a());
    m
}

#[test]
fn verify_accepts_valid_signature() {
    let mut s = PhaseSentinel::new();
    let m = signed_manifest(&mut s, b"prog");
    assert!(m.verify_signature().is_ok());
    assert!(authorize_launch(&mut s, 1, b"prog", &m, 0).is_ok());
}

#[test]
fn negative_mutates_every_protected_field() {
    let mut s = PhaseSentinel::new();
    let good = signed_manifest(&mut s, b"prog");
    let mut bad = good.clone();
    bad.generation = 0;
    s.policy.min_generation = 2;
    assert_eq!(
        authorize_launch(&mut s, 1, b"prog", &bad, 0),
        Err(SentinelError::Generation)
    );
    s.policy.min_generation = 0;

    let mut bad = good.clone();
    bad.expiry_tick = 0;
    assert_eq!(
        authorize_launch(&mut s, 1, b"prog", &bad, 1),
        Err(SentinelError::Expired)
    );

    let mut bad = good.clone();
    bad.content_hash = [9u8; 32];
    assert_eq!(
        authorize_launch(&mut s, 1, b"prog", &bad, 0),
        Err(SentinelError::Unsigned)
    );

    let mut bad = good.clone();
    bad.requested_rights = 1 << 31;
    s.policy.max_rights = 1;
    assert_eq!(
        authorize_launch(&mut s, 1, b"prog", &bad, 0),
        Err(SentinelError::RightsEscalation)
    );
    s.policy.max_rights = u32::MAX;

    let mut bad = good.clone();
    bad.signer_pubkey = [0x22; 32];
    assert_eq!(
        authorize_launch(&mut s, 1, b"prog", &bad, 0),
        Err(SentinelError::Unauthorized)
    );

    let mut bad = good.clone();
    bad.signature[0] ^= 1;
    assert_eq!(
        authorize_launch(&mut s, 1, b"prog", &bad, 0),
        Err(SentinelError::BadSignature)
    );
}

#[test]
fn revocation_and_generation_reject() {
    let mut s = PhaseSentinel::new();
    let m = signed_manifest(&mut s, b"prog");
    s.signers.revoke(&m.signer_pubkey).unwrap();
    assert_eq!(
        authorize_launch(&mut s, 1, b"prog", &m, 0),
        Err(SentinelError::Revoked)
    );
    let kp = SignKeypair::from_seed(&seed_a());
    s.signers.install(SentinelSigner {
        pubkey: kp.public_bytes(),
        generation: 0,
        expires_tick: 5,
        revoked: false,
    })
    .unwrap();
    let m = signed_manifest(&mut s, b"prog2");
    assert_eq!(
        authorize_launch(&mut s, 1, b"prog2", &m, 5),
        Err(SentinelError::Expired)
    );
}

#[test]
fn identity_certs_are_not_program_signers() {
    let mut s = PhaseSentinel::new();
    let kp = SignKeypair::from_seed(&[0x33; 32]);
    assert!(s.signers.lookup(&kp.public_bytes()).is_none());
    let err = authorize_launch(
        &mut s,
        1,
        b"x",
        &ProgramManifest {
            generation: 0,
            expiry_tick: u64::MAX,
            content_hash: ph_content(b"x"),
            requested_rights: 0,
            signer_pubkey: kp.public_bytes(),
            signature: [0u8; 64],
        },
        0,
    )
    .unwrap_err();
    assert!(
        err == SentinelError::NoAnchor || err == SentinelError::Unauthorized,
        "{err:?}"
    );
}
