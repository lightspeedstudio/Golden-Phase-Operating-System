//! AnomKeyring — derived session keys (never stores passphrase).

use crate::kernel::hash::StateHash;

pub const K_ANOM_PHASE_STATE: &str = "KAIROS_ANOM_PHASE_STATE_V1";
pub const K_ANOM_STATECHAIN_MAC: &str = "KAIROS_ANOM_STATECHAIN_MAC_V1";
pub const K_ANOM_REPLAY_EXPORT: &str = "KAIROS_ANOM_REPLAY_EXPORT_V1";
pub const K_ANOM_SESSION_SEAL: &str = "KAIROS_ANOM_SESSION_SEAL_V1";

pub fn hkdf_sha256(ikm: &[u8], salt: &[u8], info: &[u8]) -> StateHash {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut extract = HmacSha256::new_from_slice(salt).expect("hmac key");
    extract.update(ikm);
    let prk = extract.finalize().into_bytes();
    let mut expand = HmacSha256::new_from_slice(&prk).expect("hmac key");
    expand.update(info);
    expand.update(&[1u8]);
    expand.finalize().into_bytes().into()
}

pub fn derive_k_root(passphrase: &[u8], equation_root: &StateHash) -> StateHash {
    hkdf_sha256(passphrase, equation_root, b"KAIROS_ROOT_AUTHORITY_V1")
}

pub fn derive_k_anom(k_root: &StateHash, boot_session_root: &StateHash) -> StateHash {
    hkdf_sha256(
        k_root,
        boot_session_root,
        b"KAIROS_ANOM_SECURITY_PROTOCOL_V1",
    )
}

pub fn derive_subkey(k_anom: &StateHash, label: &str) -> StateHash {
    hkdf_sha256(k_anom, k_anom, label.as_bytes())
}

#[derive(Debug, Clone)]
pub struct AnomKeyring {
    pub k_root: StateHash,
    pub k_anom: StateHash,
    pub k_anom_phase_state: StateHash,
    pub k_anom_statechain_mac: StateHash,
    pub k_anom_replay_export: StateHash,
    pub k_anom_session_seal: StateHash,
}

impl AnomKeyring {
    pub fn derive(
        passphrase: &[u8],
        equation_root: &StateHash,
        boot_session_root: &StateHash,
    ) -> Self {
        let k_root = derive_k_root(passphrase, equation_root);
        let k_anom = derive_k_anom(&k_root, boot_session_root);
        Self {
            k_root,
            k_anom,
            k_anom_phase_state: derive_subkey(&k_anom, K_ANOM_PHASE_STATE),
            k_anom_statechain_mac: derive_subkey(&k_anom, K_ANOM_STATECHAIN_MAC),
            k_anom_replay_export: derive_subkey(&k_anom, K_ANOM_REPLAY_EXPORT),
            k_anom_session_seal: derive_subkey(&k_anom, K_ANOM_SESSION_SEAL),
        }
    }
}
