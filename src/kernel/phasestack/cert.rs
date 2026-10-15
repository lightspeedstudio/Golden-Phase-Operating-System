//! Developer certificate v1 — separate type and PhaseHash domains from identity.

use crate::kernel::crypto::phase_hash::{ph_developer_cert, ph_developer_cert_tbs};
use crate::kernel::crypto::sign;
use crate::kernel::hash::StateHash;
use crate::kernel::phasestack::canonical::{put_u16, put_u64, put_u8, take_fixed, take_u16, take_u64, take_u8};
use crate::kernel::phasestack::types::{
    CERTIFICATE_VERSION_V1, CERT_ALG_ED25519, CERT_FLAG_CAN_ISSUE, CERT_FLAG_ROOT, MAX_CERTIFICATES,
    MAX_CHAIN_DEPTH, NO_ISSUER, PhaseStackError, PrincipalId,
};
use crate::platform::{HashMap, HashSet, Vec};

pub type CertificateSerial = u64;

/// Development CA public pin. Generated offline; matching secret is not
/// in this tree. SHA-256 of these 32 bytes:
/// `eddd244124fdae03508d3a6a1f85e6f9bd717b76a3e903431aab6e436e2d953c`.
pub const ROOT_PUBLIC_KEY_PIN: [u8; 32] = [
    0x30, 0x79, 0x6c, 0xc3, 0xdf, 0x9a, 0x86, 0xed, 0xca, 0x40, 0xc3, 0xa6, 0x72, 0x94, 0x5a, 0x6f,
    0x78, 0x58, 0x73, 0x20, 0x2f, 0xd3, 0xaf, 0x31, 0x0a, 0x2a, 0x0d, 0x70, 0x25, 0x5c, 0x62, 0xff,
];

/// Retired research CA public pin. Packages whose chain roots here are refused.
pub const RETIRED_RESEARCH_CA_PIN: [u8; 32] = [
    0x26, 0x78, 0xbd, 0x0c, 0xaa, 0x83, 0xfc, 0x05, 0xaa, 0x71, 0xde, 0x05, 0x7d, 0x01, 0xd7, 0xb5,
    0xeb, 0x31, 0x73, 0x92, 0xf7, 0x17, 0xb0, 0x3a, 0x2a, 0xc0, 0x79, 0xdd, 0x6f, 0x12, 0xf0, 0x8e,
];

pub fn is_retired_research_ca(pk: &[u8; 32]) -> bool {
    pk == &RETIRED_RESEARCH_CA_PIN
}

pub fn embedded_root_public_key() -> [u8; 32] {
    ROOT_PUBLIC_KEY_PIN
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeveloperCertificateV1 {
    pub version: u16,
    pub serial: CertificateSerial,
    pub algorithm: u8,
    pub subject_id: PrincipalId,
    pub subject_public_key: [u8; 32],
    pub issuer_id: PrincipalId,
    pub issuer_certificate_hash: StateHash,
    pub package_type_mask: u16,
    pub scope_mask: u8,
    pub generation_start: u64,
    pub generation_end: Option<u64>,
    pub flags: u16,
    pub signature: [u8; 64],
}

pub fn encode_tbs(cert: &DeveloperCertificateV1) -> Vec<u8> {
    let mut out = Vec::new();
    put_u16(&mut out, cert.version);
    put_u64(&mut out, cert.serial);
    put_u8(&mut out, cert.algorithm);
    out.extend_from_slice(&cert.subject_id);
    out.extend_from_slice(&cert.subject_public_key);
    out.extend_from_slice(&cert.issuer_id);
    out.extend_from_slice(&cert.issuer_certificate_hash);
    put_u16(&mut out, cert.package_type_mask);
    put_u8(&mut out, cert.scope_mask);
    put_u64(&mut out, cert.generation_start);
    match cert.generation_end {
        Some(end) => {
            put_u8(&mut out, 1);
            put_u64(&mut out, end);
        }
        None => {
            put_u8(&mut out, 0);
            put_u64(&mut out, 0);
        }
    }
    put_u16(&mut out, cert.flags);
    out
}

pub fn encode_certificate(cert: &DeveloperCertificateV1) -> Vec<u8> {
    let mut out = encode_tbs(cert);
    out.extend_from_slice(&cert.signature);
    out
}

pub fn decode_certificate(bytes: &[u8]) -> Result<DeveloperCertificateV1, PhaseStackError> {
    const TBS_LEN: usize = 2 + 8 + 1 + 32 + 32 + 32 + 32 + 2 + 1 + 8 + 1 + 8 + 2;
    if bytes.len() != TBS_LEN + 64 {
        return Err(PhaseStackError::UnknownVersion);
    }
    let mut off = 0usize;
    let version = take_u16(bytes, &mut off)?;
    let serial = take_u64(bytes, &mut off)?;
    let algorithm = take_u8(bytes, &mut off)?;
    let subject_id = take_fixed::<32>(bytes, &mut off)?;
    let subject_public_key = take_fixed::<32>(bytes, &mut off)?;
    let issuer_id = take_fixed::<32>(bytes, &mut off)?;
    let issuer_certificate_hash = take_fixed::<32>(bytes, &mut off)?;
    let package_type_mask = take_u16(bytes, &mut off)?;
    let scope_mask = take_u8(bytes, &mut off)?;
    let generation_start = take_u64(bytes, &mut off)?;
    let end_flag = take_u8(bytes, &mut off)?;
    let end_val = take_u64(bytes, &mut off)?;
    let flags = take_u16(bytes, &mut off)?;
    if off != TBS_LEN {
        return Err(PhaseStackError::UnknownVersion);
    }
    let signature = take_fixed::<64>(bytes, &mut off)?;
    let generation_end = match end_flag {
        0 => None,
        1 => Some(end_val),
        _ => return Err(PhaseStackError::UnknownVersion),
    };
    Ok(DeveloperCertificateV1 {
        version,
        serial,
        algorithm,
        subject_id,
        subject_public_key,
        issuer_id,
        issuer_certificate_hash,
        package_type_mask,
        scope_mask,
        generation_start,
        generation_end,
        flags,
        signature,
    })
}

pub fn certificate_hash(cert: &DeveloperCertificateV1) -> StateHash {
    let tbs = encode_tbs(cert);
    ph_developer_cert(&tbs, &cert.signature)
}

pub fn verify_signature(cert: &DeveloperCertificateV1, issuer_pk: &[u8; 32]) -> bool {
    if cert.algorithm != CERT_ALG_ED25519 {
        return false;
    }
    let tbs_hash = ph_developer_cert_tbs(&encode_tbs(cert));
    sign::verify(issuer_pk, &tbs_hash, &cert.signature)
}

#[cfg(any(test, all(feature = "std", feature = "phasestack-sign-dev")))]
pub fn sign_certificate(
    cert: &mut DeveloperCertificateV1,
    issuer_sk: &sign::SignKeypair,
) {
    let tbs_hash = ph_developer_cert_tbs(&encode_tbs(cert));
    cert.signature = issuer_sk.sign(&tbs_hash);
}

#[derive(Debug, Clone)]
pub struct DeveloperCertificateStore {
    certs: HashMap<StateHash, DeveloperCertificateV1>,
    serials: HashSet<CertificateSerial>,
    root_public_key: [u8; 32],
}

impl Default for DeveloperCertificateStore {
    fn default() -> Self {
        Self::new()
    }
}

impl DeveloperCertificateStore {
    pub fn new() -> Self {
        Self {
            certs: HashMap::new(),
            serials: HashSet::new(),
            root_public_key: embedded_root_public_key(),
        }
    }

    pub fn root_public_key(&self) -> [u8; 32] {
        self.root_public_key
    }

    pub fn with_root_public_key(root_public_key: [u8; 32]) -> Self {
        Self {
            certs: HashMap::new(),
            serials: HashSet::new(),
            root_public_key,
        }
    }

    pub fn insert(&mut self, cert: DeveloperCertificateV1) -> Result<StateHash, PhaseStackError> {
        if self.certs.len() >= MAX_CERTIFICATES {
            return Err(PhaseStackError::TableFull);
        }
        if !self.serials.insert(cert.serial) {
            return Err(PhaseStackError::DuplicateSerial);
        }
        let hash = certificate_hash(&cert);
        if self.certs.contains_key(&hash) {
            return Err(PhaseStackError::DuplicateCert);
        }
        self.certs.insert(hash, cert);
        Ok(hash)
    }

    pub fn get(&self, hash: StateHash) -> Option<&DeveloperCertificateV1> {
        self.certs.get(&hash)
    }

    pub fn find_by_subject_key(&self, pk: &[u8; 32]) -> Option<&DeveloperCertificateV1> {
        self.certs.values().find(|c| &c.subject_public_key == pk)
    }

    pub fn known_subject_key(&self, pk: &[u8; 32]) -> bool {
        self.find_by_subject_key(pk).is_some() || pk == &self.root_public_key
    }

    /// Walk `leaf` to the embedded Root public key. `now_generation` is the
    /// live authority generation used for the validity window check.
    pub fn verify_chain(
        &self,
        leaf: &DeveloperCertificateV1,
        chain: &[DeveloperCertificateV1],
        now_generation: u64,
        revoked: &dyn Fn(StateHash) -> bool,
    ) -> Result<StateHash, PhaseStackError> {
        let mut walked: Vec<DeveloperCertificateV1> = Vec::new();
        walked.push(leaf.clone());
        walked.extend(chain.iter().cloned());
        if walked.len() > MAX_CHAIN_DEPTH {
            return Err(PhaseStackError::ChainTooDeep);
        }
        let mut seen_hash = HashSet::new();
        let mut seen_serial = HashSet::new();
        let mut current_pk = [0u8; 32];
        for (i, cert) in walked.iter().enumerate() {
            if cert.version != CERTIFICATE_VERSION_V1 {
                return Err(PhaseStackError::UnknownVersion);
            }
            if cert.algorithm != CERT_ALG_ED25519 {
                return Err(PhaseStackError::AlgorithmRejected);
            }
            let hash = certificate_hash(cert);
            if !seen_hash.insert(hash) {
                return Err(PhaseStackError::ChainCycle);
            }
            if !seen_serial.insert(cert.serial) {
                return Err(PhaseStackError::DuplicateSerial);
            }
            if revoked(hash) {
                return Err(PhaseStackError::Revoked);
            }
            if now_generation < cert.generation_start {
                return Err(PhaseStackError::GenerationExpired);
            }
            if let Some(end) = cert.generation_end {
                if now_generation > end {
                    return Err(PhaseStackError::GenerationExpired);
                }
            }
            let issuer_pk = if cert.issuer_certificate_hash == NO_ISSUER {
                if (cert.flags & CERT_FLAG_ROOT) == 0 {
                    return Err(PhaseStackError::NotSelfSigned);
                }
                if is_retired_research_ca(&cert.subject_public_key) {
                    return Err(PhaseStackError::RetiredSigner);
                }
                if cert.subject_public_key != self.root_public_key {
                    return Err(PhaseStackError::UnknownCert);
                }
                if cert.subject_id != cert.issuer_id {
                    return Err(PhaseStackError::NotSelfSigned);
                }
                cert.subject_public_key
            } else {
                let issuer = walked
                    .iter()
                    .find(|c| certificate_hash(c) == cert.issuer_certificate_hash)
                    .or_else(|| self.certs.get(&cert.issuer_certificate_hash))
                    .ok_or(PhaseStackError::UnknownIssuer)?;
                if (issuer.flags & CERT_FLAG_CAN_ISSUE) == 0 {
                    return Err(PhaseStackError::UnknownIssuer);
                }
                issuer.subject_public_key
            };
            if !verify_signature(cert, &issuer_pk) {
                return Err(PhaseStackError::SignatureInvalid);
            }
            if i == 0 {
                current_pk = cert.subject_public_key;
            }
            let _ = current_pk;
        }
        let last = walked.last().unwrap();
        if last.issuer_certificate_hash != NO_ISSUER || (last.flags & CERT_FLAG_ROOT) == 0 {
            return Err(PhaseStackError::UnknownIssuer);
        }
        Ok(certificate_hash(leaf))
    }
}

#[cfg(any(test, all(feature = "std", feature = "phasestack-sign-dev")))]
pub fn issue_root_cert_with(serial: u64, kp: &sign::SignKeypair) -> DeveloperCertificateV1 {
    let pk = kp.public_bytes();
    let mut cert = DeveloperCertificateV1 {
        version: CERTIFICATE_VERSION_V1,
        serial,
        algorithm: CERT_ALG_ED25519,
        subject_id: pk,
        subject_public_key: pk,
        issuer_id: pk,
        issuer_certificate_hash: NO_ISSUER,
        package_type_mask: crate::kernel::phasestack::types::PKG_TYPE_APP
            | crate::kernel::phasestack::types::PKG_TYPE_PLUGIN,
        scope_mask: crate::kernel::phasestack::types::SCOPE_USER
            | crate::kernel::phasestack::types::SCOPE_SYSTEM,
        generation_start: 0,
        generation_end: None,
        flags: CERT_FLAG_CAN_ISSUE | CERT_FLAG_ROOT,
        signature: [0u8; 64],
    };
    sign_certificate(&mut cert, kp);
    cert
}

#[cfg(any(test, all(feature = "std", feature = "phasestack-sign-dev")))]
pub fn issue_leaf_cert(
    serial: u64,
    subject_kp: &sign::SignKeypair,
    issuer: &DeveloperCertificateV1,
    issuer_sk: &sign::SignKeypair,
    generation_end: Option<u64>,
) -> DeveloperCertificateV1 {
    let mut cert = DeveloperCertificateV1 {
        version: CERTIFICATE_VERSION_V1,
        serial,
        algorithm: CERT_ALG_ED25519,
        subject_id: subject_kp.public_bytes(),
        subject_public_key: subject_kp.public_bytes(),
        issuer_id: issuer.subject_id,
        issuer_certificate_hash: certificate_hash(issuer),
        package_type_mask: crate::kernel::phasestack::types::PKG_TYPE_APP
            | crate::kernel::phasestack::types::PKG_TYPE_PLUGIN,
        scope_mask: crate::kernel::phasestack::types::SCOPE_USER
            | crate::kernel::phasestack::types::SCOPE_SYSTEM,
        generation_start: 0,
        generation_end,
        flags: 0,
        signature: [0u8; 64],
    };
    sign_certificate(&mut cert, issuer_sk);
    cert
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::crypto::sign::SignKeypair;

    fn store_with_root() -> (DeveloperCertificateStore, DeveloperCertificateV1, SignKeypair) {
        let kp = SignKeypair::from_seed(&[0x51u8; 32]);
        let mut store = DeveloperCertificateStore::with_root_public_key(kp.public_bytes());
        let root = issue_root_cert_with(1, &kp);
        store.insert(root.clone()).unwrap();
        (store, root, kp)
    }

    #[test]
    fn valid_root_chain_verifies() {
        let (store, root, root_kp) = store_with_root();
        let leaf_kp = SignKeypair::from_seed(&[9u8; 32]);
        let leaf = issue_leaf_cert(2, &leaf_kp, &root, &root_kp, None);
        let hash = store
            .verify_chain(&leaf, &[root], 0, &|_| false)
            .unwrap();
        assert_eq!(hash, certificate_hash(&leaf));
    }

    #[test]
    fn unknown_version_rejected() {
        let (store, mut root, root_kp) = store_with_root();
        root.version = 99;
        sign_certificate(&mut root, &root_kp);
        assert_eq!(
            store.verify_chain(&root, &[], 0, &|_| false),
            Err(PhaseStackError::UnknownVersion)
        );
    }

    #[test]
    fn algorithm_rejected() {
        let (store, mut root, root_kp) = store_with_root();
        root.algorithm = 7;
        sign_certificate(&mut root, &root_kp);
        assert_eq!(
            store.verify_chain(&root, &[], 0, &|_| false),
            Err(PhaseStackError::AlgorithmRejected)
        );
    }

    #[test]
    fn duplicate_serial_in_chain_rejected() {
        let (store, root, root_kp) = store_with_root();
        let leaf_kp = SignKeypair::from_seed(&[9u8; 32]);
        let leaf = issue_leaf_cert(1, &leaf_kp, &root, &root_kp, None);
        assert_eq!(
            store.verify_chain(&leaf, &[root], 0, &|_| false),
            Err(PhaseStackError::DuplicateSerial)
        );
    }

    #[test]
    fn chain_too_deep_rejected() {
        let (store, root, _) = store_with_root();
        let extras = vec![root.clone(); MAX_CHAIN_DEPTH];
        assert_eq!(
            store.verify_chain(&root, &extras, 0, &|_| false),
            Err(PhaseStackError::ChainTooDeep)
        );
    }

    #[test]
    fn cycle_rejected() {
        let (store, root, _) = store_with_root();
        assert_eq!(
            store.verify_chain(&root, &[root.clone()], 0, &|_| false),
            Err(PhaseStackError::ChainCycle)
        );
    }

    #[test]
    fn generation_window_rejected() {
        let (store, root, root_kp) = store_with_root();
        let leaf_kp = SignKeypair::from_seed(&[3u8; 32]);
        let leaf = issue_leaf_cert(4, &leaf_kp, &root, &root_kp, Some(10));
        assert_eq!(
            store.verify_chain(&leaf, &[root], 11, &|_| false),
            Err(PhaseStackError::GenerationExpired)
        );
    }

    #[test]
    fn tbs_hash_is_what_is_signed() {
        let kp = SignKeypair::from_seed(&[0x51u8; 32]);
        let root = issue_root_cert_with(1, &kp);
        let tbs = encode_tbs(&root);
        let h = ph_developer_cert_tbs(&tbs);
        assert!(sign::verify(
            &root.subject_public_key,
            &h,
            &root.signature
        ));
        assert!(!sign::verify(
            &root.subject_public_key,
            b"not-the-tbs-hash",
            &root.signature
        ));
    }

    #[test]
    fn board_pin_is_not_the_retired_research_ca() {
        assert_ne!(ROOT_PUBLIC_KEY_PIN, RETIRED_RESEARCH_CA_PIN);
        assert_eq!(embedded_root_public_key(), ROOT_PUBLIC_KEY_PIN);
    }

    #[test]
    fn retired_research_ca_is_refused() {
        let store = DeveloperCertificateStore::new();
        let mut retired = DeveloperCertificateV1 {
            version: CERTIFICATE_VERSION_V1,
            serial: 1,
            algorithm: CERT_ALG_ED25519,
            subject_id: RETIRED_RESEARCH_CA_PIN,
            subject_public_key: RETIRED_RESEARCH_CA_PIN,
            issuer_id: RETIRED_RESEARCH_CA_PIN,
            issuer_certificate_hash: NO_ISSUER,
            package_type_mask: crate::kernel::phasestack::types::PKG_TYPE_APP,
            scope_mask: crate::kernel::phasestack::types::SCOPE_USER,
            generation_start: 0,
            generation_end: None,
            flags: CERT_FLAG_CAN_ISSUE | CERT_FLAG_ROOT,
            signature: [0u8; 64],
        };
        let _ = &mut retired;
        assert_eq!(
            store.verify_chain(&retired, &[], 0, &|_| false),
            Err(PhaseStackError::RetiredSigner)
        );
    }
}
