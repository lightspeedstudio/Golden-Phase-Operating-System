//! Per-object key wrapping — passphrase KDF, ODK generation, wrap/unwrap.

use crate::kernel::anom::object_policy::{
    ObjectProtectionError, ProtectedObjectId, OBJECT_WRAP_TAG,
};
use crate::kernel::crypto::aead::{open_bytes, seal_bytes};
use crate::kernel::crypto::keyring::hkdf_sha256;
use crate::kernel::hash::{tagged_hash, StateHash};
use crate::platform::Vec;

pub const ODK_SIZE: usize = 32;
pub const WRAP_NONCE_SIZE: usize = 12;
pub const OBJECT_SALT_SIZE: usize = 32;

#[derive(Debug, Clone)]
pub struct WrappedObjectKey {
    pub object_salt: Vec<u8>,
    pub wrap_nonce: [u8; 12],
    pub wrapped_object_key: Vec<u8>,
    pub wrap_tag: Vec<u8>,
}

pub fn secure_random_bytes(len: usize) -> Vec<u8> {
    let mut out = Vec::new();
    out.resize(len, 0);
    fill_object_entropy(&mut out);
    out
}

fn fill_object_entropy(dest: &mut [u8]) {
    #[cfg(all(
        target_arch = "arm",
        feature = "zynq-a9",
        any(feature = "ssh-bm", feature = "identity")
    ))]
    {
        crate::board::zynq7000::rng::fill(dest);
    }
    #[cfg(not(all(
        target_arch = "arm",
        feature = "zynq-a9",
        any(feature = "ssh-bm", feature = "identity")
    )))]
    {
        use core::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let c = COUNTER.fetch_add(1, Ordering::SeqCst);
        #[cfg(feature = "std")]
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        #[cfg(not(feature = "std"))]
        let t = c as u128;
        let mut seed = tagged_hash(
            "KAIROS_OBJECT_ENTROPY_V1",
            &[&c.to_le_bytes(), &t.to_le_bytes()],
        );
        let mut off = 0;
        while off < dest.len() {
            let n = core::cmp::min(32, dest.len() - off);
            dest[off..off + n].copy_from_slice(&seed[..n]);
            off += n;
            seed = tagged_hash("KAIROS_OBJECT_ENTROPY_CHAIN_V1", &[&seed]);
        }
    }
}

pub fn generate_object_data_key() -> StateHash {
    let bytes = secure_random_bytes(ODK_SIZE);
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes[..ODK_SIZE]);
    key
}

pub fn generate_object_salt() -> Vec<u8> {
    secure_random_bytes(OBJECT_SALT_SIZE)
}

pub fn wrap_context_bytes(
    equation_root: &StateHash,
    boot_session_root: &StateHash,
    object_id: &ProtectedObjectId,
    object_salt: &[u8],
    version: u32,
) -> Vec<u8> {
    let mut ctx = Vec::new();
    ctx.extend_from_slice(OBJECT_WRAP_TAG.as_bytes());
    ctx.extend_from_slice(equation_root);
    ctx.extend_from_slice(boot_session_root);
    ctx.push(object_id.kind as u8);
    ctx.extend_from_slice(object_id.value.as_bytes());
    ctx.extend_from_slice(object_salt);
    ctx.extend_from_slice(&version.to_le_bytes());
    ctx
}

pub fn derive_wrap_key(
    passphrase: &[u8],
    equation_root: &StateHash,
    boot_session_root: &StateHash,
    object_id: &ProtectedObjectId,
    object_salt: &[u8],
    version: u32,
) -> StateHash {
    let ctx = wrap_context_bytes(
        equation_root,
        boot_session_root,
        object_id,
        object_salt,
        version,
    );
    hkdf_sha256(passphrase, object_salt, &ctx)
}

pub fn wrap_object_key(
    k_wrap: &StateHash,
    object_data_key: &StateHash,
    equation_root: &StateHash,
    boot_session_root: &StateHash,
    object_id: &ProtectedObjectId,
    object_salt: &[u8],
    version: u32,
) -> Result<WrappedObjectKey, ObjectProtectionError> {
    let nonce_bytes = secure_random_bytes(WRAP_NONCE_SIZE);
    let mut wrap_nonce = [0u8; 12];
    wrap_nonce.copy_from_slice(&nonce_bytes[..WRAP_NONCE_SIZE]);
    let aad = wrap_context_bytes(
        equation_root,
        boot_session_root,
        object_id,
        object_salt,
        version,
    );
    let wrapped = seal_bytes(k_wrap, &wrap_nonce, &aad, object_data_key)
        .map_err(|_| ObjectProtectionError::CryptoUnavailable)?;
    Ok(WrappedObjectKey {
        object_salt: object_salt.to_vec(),
        wrap_nonce,
        wrapped_object_key: wrapped,
        wrap_tag: Vec::new(),
    })
}

pub fn unwrap_object_key(
    passphrase: &[u8],
    equation_root: &StateHash,
    boot_session_root: &StateHash,
    object_id: &ProtectedObjectId,
    wrapped: &WrappedObjectKey,
    version: u32,
) -> Result<StateHash, ObjectProtectionError> {
    let k_wrap = derive_wrap_key(
        passphrase,
        equation_root,
        boot_session_root,
        object_id,
        &wrapped.object_salt,
        version,
    );
    let aad = wrap_context_bytes(
        equation_root,
        boot_session_root,
        object_id,
        &wrapped.object_salt,
        version,
    );
    let plain = open_bytes(
        &k_wrap,
        &wrapped.wrap_nonce,
        &aad,
        &wrapped.wrapped_object_key,
    )
    .map_err(|_| ObjectProtectionError::AuthenticationFailed)?;
    if plain.len() != ODK_SIZE {
        return Err(ObjectProtectionError::AuthenticationFailed);
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&plain);
    Ok(key)
}

pub fn zeroize_key(key: &mut StateHash) {
    key.fill(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::anom::object_policy::ProtectedObjectKind;
    use crate::kernel::anom::object_policy::OBJECT_PROTECTION_VERSION;

    fn sample_id() -> ProtectedObjectId {
        ProtectedObjectId::new(ProtectedObjectKind::Process, "7")
    }

    #[test]
    fn odk_is_random() {
        let a = generate_object_data_key();
        let b = generate_object_data_key();
        assert_ne!(a, b);
    }

    #[test]
    fn different_salts_different_wrap_keys() {
        let eq = [1u8; 32];
        let bsr = [2u8; 32];
        let id = sample_id();
        let salt_a = generate_object_salt();
        let salt_b = generate_object_salt();
        let ka = derive_wrap_key(b"pass", &eq, &bsr, &id, &salt_a, OBJECT_PROTECTION_VERSION);
        let kb = derive_wrap_key(b"pass", &eq, &bsr, &id, &salt_b, OBJECT_PROTECTION_VERSION);
        assert_ne!(ka, kb);
    }

    #[test]
    fn different_object_ids_different_wrap_keys() {
        let eq = [1u8; 32];
        let bsr = [2u8; 32];
        let salt = generate_object_salt();
        let id_a = ProtectedObjectId::new(ProtectedObjectKind::Process, "7");
        let id_b = ProtectedObjectId::new(ProtectedObjectKind::Process, "8");
        let ka = derive_wrap_key(b"pass", &eq, &bsr, &id_a, &salt, OBJECT_PROTECTION_VERSION);
        let kb = derive_wrap_key(b"pass", &eq, &bsr, &id_b, &salt, OBJECT_PROTECTION_VERSION);
        assert_ne!(ka, kb);
    }

    #[test]
    fn wrap_unwrap_round_trip() {
        let eq = [1u8; 32];
        let bsr = [2u8; 32];
        let id = sample_id();
        let salt = generate_object_salt();
        let odk = generate_object_data_key();
        let k_wrap = derive_wrap_key(b"pass", &eq, &bsr, &id, &salt, OBJECT_PROTECTION_VERSION);
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
        let opened =
            unwrap_object_key(b"pass", &eq, &bsr, &id, &wrapped, OBJECT_PROTECTION_VERSION)
                .unwrap();
        assert_eq!(opened, odk);
    }

    #[test]
    fn wrong_passphrase_fails() {
        let eq = [1u8; 32];
        let bsr = [2u8; 32];
        let id = sample_id();
        let salt = generate_object_salt();
        let odk = generate_object_data_key();
        let k_wrap = derive_wrap_key(b"pass", &eq, &bsr, &id, &salt, OBJECT_PROTECTION_VERSION);
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
}
