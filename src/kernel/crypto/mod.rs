//! Kernel crypto — HKDF + AEAD + ed25519 (hosted std and bare metal).

#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod aead;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod keyring;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod nonce;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod phase_hash;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod sealed_event;
#[cfg(any(feature = "std", feature = "baremetal"))]
pub mod sign;

#[cfg(any(feature = "std", feature = "baremetal"))]
pub use keyring::AnomKeyring;

/// Self-test the on-board crypto primitives: AES-256-GCM seal/open round-trip
/// and ed25519 sign/verify (plus tamper rejection). Returns a one-line report.
#[cfg(feature = "baremetal")]
pub fn selftest() -> crate::platform::String {
    use crate::kernel::hash::tagged_hash;

    let key = tagged_hash("KAIROS_CRYPTO_SELFTEST_KEY", &[b"seed"]);
    let nonce = [7u8; 12];
    let aad = b"selftest-aad";
    let msg = b"golden phase crypto selftest";

    let aead_ok = match aead::seal_bytes(&key, &nonce, aad, msg) {
        Ok(ct) => matches!(aead::open_bytes(&key, &nonce, aad, &ct), Ok(pt) if pt == msg),
        Err(_) => false,
    };

    let seed = tagged_hash("KAIROS_CRYPTO_SELFTEST_ED25519", &[b"seed"]);
    let kp = sign::SignKeypair::from_seed(&seed);
    let pk = kp.public_bytes();
    let sig = kp.sign(msg);
    let sign_ok = sign::verify(&pk, msg, &sig);
    let reject_tamper = !sign::verify(&pk, b"tampered message", &sig);

    let pass = aead_ok && sign_ok && reject_tamper;
    alloc::format!(
        "crypto selftest: aead={} ed25519_sign={} reject_tamper={} => {}",
        yesno(aead_ok),
        yesno(sign_ok),
        yesno(reject_tamper),
        if pass { "PASS" } else { "FAIL" }
    )
}

#[cfg(feature = "baremetal")]
fn yesno(b: bool) -> &'static str {
    if b {
        "ok"
    } else {
        "FAIL"
    }
}
