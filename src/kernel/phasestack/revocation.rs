//! Package / certificate / issuer / generation revocation.

use crate::kernel::hash::StateHash;
use crate::kernel::phasestack::canonical::{put_u16, put_u64, put_u8, take_fixed, take_u16, take_u64, take_u8};
use crate::kernel::phasestack::types::PhaseStackError;
use crate::kernel::state_root::merkle_root;
use crate::platform::{HashSet, Vec};

const REVOCATION_MAGIC: &[u8; 4] = b"PSRV";
const REVOCATION_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevocationKind {
    Certificate,
    Package,
    Issuer,
    Generation,
}

impl RevocationKind {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Certificate => 0,
            Self::Package => 1,
            Self::Issuer => 2,
            Self::Generation => 3,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PackageRevocationStore {
    certs: HashSet<StateHash>,
    packages: HashSet<StateHash>,
    issuers: HashSet<StateHash>,
    generations: HashSet<u64>,
}

impl PackageRevocationStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn revoke_certificate(&mut self, hash: StateHash) {
        self.certs.insert(hash);
    }

    pub fn revoke_package(&mut self, root: StateHash) {
        self.packages.insert(root);
    }

    pub fn revoke_issuer(&mut self, issuer_hash: StateHash) {
        self.issuers.insert(issuer_hash);
    }

    pub fn revoke_generation(&mut self, generation: u64) {
        self.generations.insert(generation);
    }

    pub fn cert_revoked(&self, hash: StateHash) -> bool {
        self.certs.contains(&hash)
    }

    pub fn package_revoked(&self, root: StateHash) -> bool {
        self.packages.contains(&root)
    }

    pub fn issuer_revoked(&self, issuer: StateHash) -> bool {
        self.issuers.contains(&issuer)
    }

    pub fn generation_revoked(&self, generation: u64) -> bool {
        self.generations.contains(&generation)
    }

    pub fn check_package(
        &self,
        package_root: StateHash,
        cert_hash: StateHash,
        issuer_hash: StateHash,
        generation: u64,
    ) -> Result<(), PhaseStackError> {
        if self.package_revoked(package_root) {
            return Err(PhaseStackError::Revoked);
        }
        if self.cert_revoked(cert_hash) {
            return Err(PhaseStackError::Revoked);
        }
        if self.issuer_revoked(issuer_hash) {
            return Err(PhaseStackError::Revoked);
        }
        if self.generation_revoked(generation) {
            return Err(PhaseStackError::Revoked);
        }
        Ok(())
    }

    pub fn merkle_root(&self) -> StateHash {
        let mut leaves: Vec<StateHash> = Vec::new();
        let mut certs: Vec<_> = self.certs.iter().copied().collect();
        certs.sort_unstable();
        leaves.extend(certs);
        let mut pkgs: Vec<_> = self.packages.iter().copied().collect();
        pkgs.sort_unstable();
        leaves.extend(pkgs);
        let mut issuers: Vec<_> = self.issuers.iter().copied().collect();
        issuers.sort_unstable();
        leaves.extend(issuers);
        let mut gens: Vec<_> = self.generations.iter().copied().collect();
        gens.sort_unstable();
        for g in gens {
            let mut leaf = [0u8; 32];
            leaf[..8].copy_from_slice(&g.to_le_bytes());
            leaves.push(leaf);
        }
        merkle_root(&leaves)
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut certs: Vec<_> = self.certs.iter().copied().collect();
        certs.sort_unstable();
        let mut pkgs: Vec<_> = self.packages.iter().copied().collect();
        pkgs.sort_unstable();
        let mut issuers: Vec<_> = self.issuers.iter().copied().collect();
        issuers.sort_unstable();
        let mut gens: Vec<_> = self.generations.iter().copied().collect();
        gens.sort_unstable();
        let mut out = Vec::new();
        out.extend_from_slice(REVOCATION_MAGIC);
        put_u8(&mut out, REVOCATION_VERSION);
        put_u16(&mut out, certs.len() as u16);
        for h in &certs {
            out.extend_from_slice(h);
        }
        put_u16(&mut out, pkgs.len() as u16);
        for h in &pkgs {
            out.extend_from_slice(h);
        }
        put_u16(&mut out, issuers.len() as u16);
        for h in &issuers {
            out.extend_from_slice(h);
        }
        put_u16(&mut out, gens.len() as u16);
        for g in gens {
            put_u64(&mut out, g);
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, PhaseStackError> {
        if bytes.len() < 5 || &bytes[0..4] != REVOCATION_MAGIC {
            return Err(PhaseStackError::BadMagic);
        }
        let mut off = 4usize;
        if take_u8(bytes, &mut off)? != REVOCATION_VERSION {
            return Err(PhaseStackError::UnknownVersion);
        }
        let mut store = Self::new();
        let n_certs = take_u16(bytes, &mut off)? as usize;
        for _ in 0..n_certs {
            store.certs.insert(take_fixed::<32>(bytes, &mut off)?);
        }
        let n_pkgs = take_u16(bytes, &mut off)? as usize;
        for _ in 0..n_pkgs {
            store.packages.insert(take_fixed::<32>(bytes, &mut off)?);
        }
        let n_issuers = take_u16(bytes, &mut off)? as usize;
        for _ in 0..n_issuers {
            store.issuers.insert(take_fixed::<32>(bytes, &mut off)?);
        }
        let n_gens = take_u16(bytes, &mut off)? as usize;
        for _ in 0..n_gens {
            store.generations.insert(take_u64(bytes, &mut off)?);
        }
        if off != bytes.len() {
            return Err(PhaseStackError::CanonicalMismatch);
        }
        Ok(store)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revoked_package_rejected() {
        let mut s = PackageRevocationStore::new();
        s.revoke_package([1u8; 32]);
        assert_eq!(
            s.check_package([1u8; 32], [0u8; 32], [0u8; 32], 0),
            Err(PhaseStackError::Revoked)
        );
    }

    #[test]
    fn leaf_change_changes_root() {
        let mut s = PackageRevocationStore::new();
        let a = s.merkle_root();
        s.revoke_certificate([9u8; 32]);
        assert_ne!(a, s.merkle_root());
    }

    #[test]
    fn encode_decode_round_trip() {
        let mut s = PackageRevocationStore::new();
        s.revoke_package([1u8; 32]);
        s.revoke_certificate([2u8; 32]);
        s.revoke_issuer([3u8; 32]);
        s.revoke_generation(7);
        let back = PackageRevocationStore::decode(&s.encode()).unwrap();
        assert_eq!(s.merkle_root(), back.merkle_root());
        assert!(back.package_revoked([1u8; 32]));
        assert!(back.cert_revoked([2u8; 32]));
        assert!(back.issuer_revoked([3u8; 32]));
        assert!(back.generation_revoked(7));
    }

    #[test]
    fn decode_rejects_bad_magic() {
        assert!(matches!(
            PackageRevocationStore::decode(b"xxxx"),
            Err(PhaseStackError::BadMagic)
        ));
    }
}
