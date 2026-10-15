//! Identity persistence (`kernel::identity`, Gate 6) — canonical snapshot
//! encoding plus backends.
//!
//! Gold Utility persists the snapshot as PhaseFs [`PHASEFS_STORE_PATH`]
//! through `syscall::persist_write_file` (SD WAL on MBR partition 1, same
//! path as GPA install). Boot restore is a WAL redo into PhaseFs, then
//! [`Identity::restore_from_bytes`]. That is one media contract with the
//! journal, not a second FAT-root file.
//!
//! [`FatRootStore`] (`IDSTORE.BIN` on FAT partition 0) is the old path.
//! Gold persist does not write it. Boot may import it once if the WAL
//! file is missing, then persist onto partition 1.
//!
//! ## What this reuses
//! - `syscall::persist_write_file` / `journal_sd::run_ops` — the same WAL
//!   redo already used by PhaseStack install (A11).
//! - Canonical encodings already defined by Gates 2-5
//!   ([`crate::kernel::identity::certificate::encode_certificate`],
//!   [`PasswordVerifier`] packed fields, [`IdentityEvent`] packed fields).
//!
//! ## Honest atomicity claim
//! WAL last-commit recover, same class as other `persist_write_file`
//! callers. Not a two-phase commit across FAT p0 and journal p1.
//!
//! ## Non-goals
//! - No encryption of the store file (the password verifier is already
//!   Argon2id output; plaintext passwords are never persisted).
//! - No HMAC-as-signature (A18). No IOMMU.

use crate::kernel::hash::StateHash;
use crate::kernel::identity::certificate::{
    decode_certificate, encode_certificate, CertificateError, UserCertificateV1,
};
use crate::kernel::identity::credential::{
    PasswordAlgorithm, PasswordPolicy, PasswordVerifier, RootCredentialState, SALT_LEN,
    VERIFIER_LEN,
};
use crate::kernel::identity::ledger::{IdentityEvent, IdentityEventKind};
use crate::kernel::identity::principal::{PrincipalId, PrincipalKind, PrincipalRecord};
use crate::platform::Vec;

pub const STORE_MAGIC: &[u8; 8] = b"GPID\x01\x00\x00\x00";
pub const STORE_VERSION: u16 = 1;

/// PhaseFs path for the identity snapshot (WAL redo on partition 1).
pub const PHASEFS_STORE_PATH: &str = "/idstore.bin";

/// 8.3 name for the legacy FAT32 file (`IDSTORE.BIN`) on partition 0.
/// Gold persist does not write this. Boot may import it once.
pub const FAT_STORE_NAME_83: [u8; 11] = *b"IDSTORE BIN";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreError {
    BadMagic,
    UnknownVersion,
    Truncated,
    TooLarge,
    Decode(CertificateError),
    Principal,
    Credential,
    Io,
    NotFound,
}

const MAX_STORE_BYTES: usize = 256 * 1024;

pub trait IdentityStore {
    fn save(&mut self, bytes: &[u8]) -> Result<(), StoreError>;
    fn load(&mut self) -> Result<Option<Vec<u8>>, StoreError>;
}

/// RAM-only backend, used by tests and as the default until a real
/// filesystem backend is attached.
#[derive(Debug, Clone, Default)]
pub struct MemoryStore {
    bytes: Option<Vec<u8>>,
}

impl IdentityStore for MemoryStore {
    fn save(&mut self, bytes: &[u8]) -> Result<(), StoreError> {
        if bytes.len() > MAX_STORE_BYTES {
            return Err(StoreError::TooLarge);
        }
        self.bytes = Some(bytes.to_vec());
        Ok(())
    }

    fn load(&mut self) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self.bytes.clone())
    }
}

/// Hosted backend: one file on the host filesystem.
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub struct StdFsStore {
    pub path: std::path::PathBuf,
}

#[cfg(feature = "std")]
impl IdentityStore for StdFsStore {
    fn save(&mut self, bytes: &[u8]) -> Result<(), StoreError> {
        if bytes.len() > MAX_STORE_BYTES {
            return Err(StoreError::TooLarge);
        }
        std::fs::write(&self.path, bytes).map_err(|_| StoreError::Io)
    }

    fn load(&mut self) -> Result<Option<Vec<u8>>, StoreError> {
        match std::fs::read(&self.path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(StoreError::Io),
        }
    }
}

/// Legacy FAT p0 backend. Gold persist uses [`PHASEFS_STORE_PATH`] on the
/// journal instead. Kept so a missing WAL file can import an old card.
#[cfg(all(feature = "baremetal", feature = "zynq-a9", not(feature = "std")))]
#[derive(Debug, Clone, Copy, Default)]
pub struct FatRootStore;

#[cfg(all(feature = "baremetal", feature = "zynq-a9", not(feature = "std")))]
impl IdentityStore for FatRootStore {
    fn save(&mut self, bytes: &[u8]) -> Result<(), StoreError> {
        if bytes.len() > MAX_STORE_BYTES {
            return Err(StoreError::TooLarge);
        }
        let mut sd = crate::board::zynq7000::sd::SdCard::init().map_err(|_| StoreError::Io)?;
        let fat = crate::board::zynq7000::fat::Fat32::mount(&mut sd).map_err(|_| StoreError::Io)?;
        fat.write_root_file(&mut sd, &FAT_STORE_NAME_83, bytes)
            .map_err(|_| StoreError::Io)
    }

    fn load(&mut self) -> Result<Option<Vec<u8>>, StoreError> {
        let mut sd = crate::board::zynq7000::sd::SdCard::init().map_err(|_| StoreError::Io)?;
        let fat = crate::board::zynq7000::fat::Fat32::mount(&mut sd).map_err(|_| StoreError::Io)?;
        match fat.find_in_root(&mut sd, &FAT_STORE_NAME_83) {
            Ok((_, size)) => {
                let mut buf = alloc::vec![0u8; size as usize];
                let n = fat
                    .read_root_file(&mut sd, &FAT_STORE_NAME_83, &mut buf)
                    .map_err(|_| StoreError::Io)?;
                buf.truncate(n);
                Ok(Some(buf))
            }
            Err(crate::board::zynq7000::fat::FatError::NotFound) => Ok(None),
            Err(_) => Err(StoreError::Io),
        }
    }
}

struct Writer<'a> {
    out: &'a mut Vec<u8>,
}

impl<'a> Writer<'a> {
    fn u8(&mut self, v: u8) {
        self.out.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    fn bytes(&mut self, v: &[u8]) {
        self.out.extend_from_slice(v);
    }
}

struct Reader<'a> {
    src: &'a [u8],
    off: usize,
}

impl<'a> Reader<'a> {
    fn remaining(&self) -> usize {
        self.src.len().saturating_sub(self.off)
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], StoreError> {
        let end = self.off.checked_add(n).ok_or(StoreError::Truncated)?;
        if end > self.src.len() {
            return Err(StoreError::Truncated);
        }
        let slice = &self.src[self.off..end];
        self.off = end;
        Ok(slice)
    }
    fn u8(&mut self) -> Result<u8, StoreError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, StoreError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, StoreError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, StoreError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn hash(&mut self) -> Result<StateHash, StoreError> {
        let mut h = [0u8; 32];
        h.copy_from_slice(self.take(32)?);
        Ok(h)
    }
}

fn encode_verifier(w: &mut Writer<'_>, v: &PasswordVerifier) {
    w.u16(v.version);
    w.u8(v.algorithm.as_u8());
    w.bytes(&v.salt);
    w.bytes(&v.verifier);
    w.u32(v.memory_kib);
    w.u32(v.iterations);
    w.u8(v.lanes);
    w.u64(v.changed_generation);
    w.u8(u8::from(v.must_change));
    w.u16(v.failed_attempts);
    w.u64(v.lockout_until_chronos);
}

fn decode_verifier(r: &mut Reader<'_>) -> Result<PasswordVerifier, StoreError> {
    let version = r.u16()?;
    let algorithm = PasswordAlgorithm::from_u8(r.u8()?).ok_or(StoreError::UnknownVersion)?;
    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(r.take(SALT_LEN)?);
    let mut verifier = [0u8; VERIFIER_LEN];
    verifier.copy_from_slice(r.take(VERIFIER_LEN)?);
    Ok(PasswordVerifier {
        version,
        algorithm,
        salt,
        verifier,
        memory_kib: r.u32()?,
        iterations: r.u32()?,
        lanes: r.u8()?,
        changed_generation: r.u64()?,
        must_change: r.u8()? != 0,
        failed_attempts: r.u16()?,
        lockout_until_chronos: r.u64()?,
    })
}

fn encode_event(w: &mut Writer<'_>, e: &IdentityEvent) {
    w.u64(e.sequence);
    w.u8(e.kind.as_u8());
    w.u32(e.principal_id);
    w.u64(e.tick);
    w.bytes(&e.payload_hash);
    w.bytes(&e.previous_hash);
    w.bytes(&e.event_hash);
}

fn decode_event(r: &mut Reader<'_>) -> Result<IdentityEvent, StoreError> {
    Ok(IdentityEvent {
        sequence: r.u64()?,
        kind: IdentityEventKind::from_u8(r.u8()?).ok_or(StoreError::UnknownVersion)?,
        principal_id: r.u32()?,
        tick: r.u64()?,
        payload_hash: r.hash()?,
        previous_hash: r.hash()?,
        event_hash: r.hash()?,
    })
}

fn encode_hashes(w: &mut Writer<'_>, hashes: &[StateHash]) {
    w.u16(hashes.len() as u16);
    for h in hashes {
        w.bytes(h);
    }
}

fn decode_hashes(r: &mut Reader<'_>) -> Result<Vec<StateHash>, StoreError> {
    let n = r.u16()? as usize;
    let mut out = Vec::new();
    for _ in 0..n {
        out.push(r.hash()?);
    }
    Ok(out)
}

/// Packed identity snapshot. See [`crate::kernel::identity::Identity::encode`].
pub struct EncodedIdentity {
    pub installation_id: StateHash,
    pub root_pubkey_hash: StateHash,
    pub policy_hash: StateHash,
    pub password_policy: PasswordPolicy,
    pub root_state: RootCredentialState,
    pub root_verifier: Option<PasswordVerifier>,
    pub root_history: Vec<PasswordVerifier>,
    pub principals: Vec<(PrincipalId, PrincipalRecord)>,
    pub next_principal_id: PrincipalId,
    pub certificates: Vec<UserCertificateV1>,
    pub revoked: Vec<StateHash>,
    pub root_certificate_hash: Option<StateHash>,
    pub ledger_sequence: u64,
    pub ledger_chain_head: StateHash,
    pub ledger_events: Vec<IdentityEvent>,
    pub principal_leaves: Vec<StateHash>,
    pub certificate_leaves: Vec<StateHash>,
    pub credential_leaves: Vec<StateHash>,
    pub revocation_leaves: Vec<StateHash>,
    pub policy_leaves: Vec<StateHash>,
}

pub fn encode_identity(snap: &EncodedIdentity) -> Result<Vec<u8>, StoreError> {
    let mut out = Vec::new();
    let mut w = Writer { out: &mut out };
    w.bytes(STORE_MAGIC);
    w.u16(STORE_VERSION);
    w.bytes(&snap.installation_id);
    w.bytes(&snap.root_pubkey_hash);
    w.bytes(&snap.policy_hash);
    w.u16(snap.password_policy.minimum_length);
    w.u16(snap.password_policy.maximum_length);
    w.u16(snap.password_policy.maximum_failures);
    w.u64(snap.password_policy.initial_delay_ticks);
    w.u64(snap.password_policy.maximum_delay_ticks);
    w.bytes(&snap.password_policy.blocklist_root);
    w.u8(snap.password_policy.history_depth);
    w.u8(snap.root_state.as_u8());
    match &snap.root_verifier {
        Some(v) => {
            w.u8(1);
            encode_verifier(&mut w, v);
        }
        None => w.u8(0),
    }
    w.u8(snap.root_history.len() as u8);
    for v in &snap.root_history {
        encode_verifier(&mut w, v);
    }
    w.u16(snap.principals.len() as u16);
    for (id, rec) in &snap.principals {
        w.u32(*id);
        w.u8(rec.kind.as_u8());
        w.u8(u8::from(rec.enabled));
    }
    w.u32(snap.next_principal_id);
    w.u16(snap.certificates.len() as u16);
    for cert in &snap.certificates {
        let encoded = encode_certificate(cert);
        w.u16(encoded.len() as u16);
        w.bytes(&encoded);
    }
    encode_hashes(&mut w, &snap.revoked);
    match snap.root_certificate_hash {
        Some(h) => {
            w.u8(1);
            w.bytes(&h);
        }
        None => w.u8(0),
    }
    w.u64(snap.ledger_sequence);
    w.bytes(&snap.ledger_chain_head);
    w.u16(snap.ledger_events.len() as u16);
    for e in &snap.ledger_events {
        encode_event(&mut w, e);
    }
    encode_hashes(&mut w, &snap.principal_leaves);
    encode_hashes(&mut w, &snap.certificate_leaves);
    encode_hashes(&mut w, &snap.credential_leaves);
    encode_hashes(&mut w, &snap.revocation_leaves);
    encode_hashes(&mut w, &snap.policy_leaves);
    if out.len() > MAX_STORE_BYTES {
        return Err(StoreError::TooLarge);
    }
    Ok(out)
}

pub fn decode_identity(bytes: &[u8]) -> Result<EncodedIdentity, StoreError> {
    if bytes.len() > MAX_STORE_BYTES {
        return Err(StoreError::TooLarge);
    }
    let mut r = Reader { src: bytes, off: 0 };
    let magic = r.take(8)?;
    if magic != STORE_MAGIC {
        return Err(StoreError::BadMagic);
    }
    let version = r.u16()?;
    if version != STORE_VERSION {
        return Err(StoreError::UnknownVersion);
    }
    let installation_id = r.hash()?;
    let root_pubkey_hash = r.hash()?;
    let policy_hash = r.hash()?;
    let password_policy = PasswordPolicy {
        minimum_length: r.u16()?,
        maximum_length: r.u16()?,
        maximum_failures: r.u16()?,
        initial_delay_ticks: r.u64()?,
        maximum_delay_ticks: r.u64()?,
        blocklist_root: r.hash()?,
        history_depth: r.u8()?,
    };
    let root_state = RootCredentialState::from_u8(r.u8()?).ok_or(StoreError::UnknownVersion)?;
    let root_verifier = match r.u8()? {
        0 => None,
        1 => Some(decode_verifier(&mut r)?),
        _ => return Err(StoreError::UnknownVersion),
    };
    let hist_len = r.u8()? as usize;
    let mut root_history = Vec::new();
    for _ in 0..hist_len {
        root_history.push(decode_verifier(&mut r)?);
    }
    let pcount = r.u16()? as usize;
    let mut principals = Vec::new();
    for _ in 0..pcount {
        let id = r.u32()?;
        let kind = PrincipalKind::from_u8(r.u8()?).ok_or(StoreError::UnknownVersion)?;
        let enabled = r.u8()? != 0;
        principals.push((id, PrincipalRecord { kind, enabled }));
    }
    let next_principal_id = r.u32()?;
    let ccount = r.u16()? as usize;
    let mut certificates = Vec::new();
    for _ in 0..ccount {
        let len = r.u16()? as usize;
        let encoded = r.take(len)?;
        certificates.push(decode_certificate(encoded).map_err(StoreError::Decode)?);
    }
    let revoked = decode_hashes(&mut r)?;
    let root_certificate_hash = match r.u8()? {
        0 => None,
        1 => Some(r.hash()?),
        _ => return Err(StoreError::UnknownVersion),
    };
    let ledger_sequence = r.u64()?;
    let ledger_chain_head = r.hash()?;
    let ecount = r.u16()? as usize;
    let mut ledger_events = Vec::new();
    for _ in 0..ecount {
        ledger_events.push(decode_event(&mut r)?);
    }
    let principal_leaves = decode_hashes(&mut r)?;
    let certificate_leaves = decode_hashes(&mut r)?;
    let credential_leaves = decode_hashes(&mut r)?;
    let revocation_leaves = decode_hashes(&mut r)?;
    let policy_leaves = decode_hashes(&mut r)?;
    if r.remaining() != 0 {
        return Err(StoreError::UnknownVersion);
    }
    Ok(EncodedIdentity {
        installation_id,
        root_pubkey_hash,
        policy_hash,
        password_policy,
        root_state,
        root_verifier,
        root_history,
        principals,
        next_principal_id,
        certificates,
        revoked,
        root_certificate_hash,
        ledger_sequence,
        ledger_chain_head,
        ledger_events,
        principal_leaves,
        certificate_leaves,
        credential_leaves,
        revocation_leaves,
        policy_leaves,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::identity::Identity;
    use crate::kernel::identity::PrincipalKind;

    #[test]
    fn encode_decode_round_trips_a_mutated_identity() {
        let mut identity = Identity::new();
        identity.set_installation_id([9u8; 32]);
        let id = identity.create_principal(PrincipalKind::Operator, 1).unwrap();
        identity.disable_principal(id, 2).unwrap();

        let bytes = identity.encode().unwrap();
        let mut restored = Identity::new();
        restored.restore_from_bytes(&bytes).unwrap();

        assert_eq!(restored.installation_id(), [9u8; 32]);
        assert_eq!(restored.authority_root(), identity.authority_root());
        assert_eq!(
            restored.authority_state_root(),
            identity.authority_state_root()
        );
        assert_eq!(restored.ledger().sequence(), identity.ledger().sequence());
        assert_eq!(restored.ledger().verify(), Ok(()));
        assert!(!restored.principals().get(id).unwrap().enabled);
    }

    #[test]
    fn memory_store_survives_a_new_identity_instance() {
        let mut identity = Identity::new();
        identity.create_principal(PrincipalKind::Guest, 0).unwrap();
        let mut store = MemoryStore::default();
        identity.persist_to(&mut store).unwrap();

        let mut restored = Identity::new();
        restored.load_from(&mut store).unwrap();
        assert_eq!(restored.ledger().sequence(), 1);
        assert_eq!(restored.principals().len(), 2);
    }

    #[test]
    fn bad_magic_is_rejected() {
        let mut identity = Identity::new();
        assert_eq!(
            identity.restore_from_bytes(b"not-a-store"),
            Err(StoreError::BadMagic)
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn std_fs_store_round_trips_across_process_equivalent_reload() {
        let dir = std::env::temp_dir().join(format!(
            "kairos-identity-gate6-{}.bin",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&dir);
        let mut store = StdFsStore { path: dir.clone() };

        let mut identity = Identity::new();
        identity.create_principal(PrincipalKind::Mod, 7).unwrap();
        identity.persist_to(&mut store).unwrap();

        let mut restored = Identity::new();
        restored.load_from(&mut store).unwrap();
        assert_eq!(restored.ledger().sequence(), 1);
        assert_eq!(restored.principals().len(), 2);
        let _ = std::fs::remove_file(&dir);
    }

    #[cfg(all(feature = "phasefs-journal", feature = "std"))]
    #[test]
    fn identity_checkpoint_is_a_wal_redo_not_fat_root() {
        let mut kernel = crate::kernel::Kernel::new();
        kernel
            .identity_create_principal(PrincipalKind::Operator)
            .unwrap();
        let seq = kernel.identity.ledger().sequence();
        let bytes = kernel.fs.read_file(PHASEFS_STORE_PATH).unwrap();
        assert!(
            bytes.starts_with(STORE_MAGIC),
            "identity snapshot must be PhaseFs /idstore.bin"
        );

        let mut recovered = crate::fs::PhaseFs::new();
        kernel.fs_journal.recover(&mut recovered).unwrap();
        assert_eq!(recovered.read_file(PHASEFS_STORE_PATH).unwrap(), bytes);

        kernel.fs = recovered;
        kernel.identity = Identity::new();
        assert_ne!(kernel.identity.ledger().sequence(), seq);
        kernel.journal_post_recover();
        assert_eq!(kernel.identity.ledger().sequence(), seq);
        assert_eq!(kernel.identity.principals().len(), 2);
    }

    #[cfg(all(feature = "phasefs-journal", feature = "std"))]
    #[test]
    fn format_wal_then_recover_restores_identity() {
        let mut kernel = crate::kernel::Kernel::new();
        kernel
            .identity_create_principal(PrincipalKind::Operator)
            .unwrap();
        let seq = kernel.identity.ledger().sequence();
        let bytes = kernel.fs.read_file(PHASEFS_STORE_PATH).unwrap();
        kernel.fs_journal = crate::fs::journal::PhaseFsJournal::format_ram(&kernel.fs).unwrap();
        let mut recovered = crate::fs::PhaseFs::new();
        let report = kernel.fs_journal.recover(&mut recovered).unwrap();
        assert_eq!(report.class, crate::fs::journal::RecoverClass::Clean);
        assert!(!report.discarded);
        assert_eq!(recovered.read_file(PHASEFS_STORE_PATH).unwrap(), bytes);
        kernel.fs = recovered;
        kernel.identity = Identity::new();
        kernel.journal_post_recover();
        assert_eq!(kernel.identity.ledger().sequence(), seq);
        assert_eq!(kernel.identity.principals().len(), 2);
    }

    #[cfg(all(feature = "phasefs-journal", feature = "std"))]
    #[test]
    fn missing_idstore_after_recover_keeps_ram_defaults() {
        let mut kernel = crate::kernel::Kernel::new();
        assert!(kernel.fs.read_file(PHASEFS_STORE_PATH).is_err());
        kernel.journal_post_recover();
        assert_eq!(kernel.identity.principals().len(), 1);
        assert_eq!(kernel.identity.ledger().sequence(), 0);
    }
}
