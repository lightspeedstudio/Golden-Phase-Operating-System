//! Immutable launch ticket. Only PhaseStack's live install/registry path may mint.

use crate::kernel::hash::StateHash;
use crate::kernel::phasestack::types::{
    AppId, AppVersion, ExecEnvelope, VerifiedInstallation, LAUNCH_NONCE_LEN,
};

/// Verified launch ticket. Fields private. No public constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedLaunchTicket {
    package_id: AppId,
    package_version: AppVersion,
    signer_key_id: [u8; 32],
    developer_cert_hash: StateHash,
    manifest_root: StateHash,
    payload_root: StateHash,
    registry_generation: u64,
    install_sequence: u64,
    requested_caps: u32,
    granted_caps: u32,
    memory_base: usize,
    memory_len: usize,
    entry_id: u32,
    launch_nonce: [u8; LAUNCH_NONCE_LEN],
}

impl VerifiedLaunchTicket {
    pub(super) fn from_installation(
        inst: &VerifiedInstallation,
        launch_nonce: [u8; LAUNCH_NONCE_LEN],
    ) -> Self {
        let env = inst.package().envelope();
        Self {
            package_id: inst.package().app_id(),
            package_version: inst.package().version(),
            signer_key_id: inst.package().publisher_key_id(),
            developer_cert_hash: inst.package().developer_cert_hash(),
            manifest_root: inst.package().manifest_hash(),
            payload_root: inst.package().code_hash(),
            registry_generation: inst.registry_generation(),
            install_sequence: inst.install_sequence(),
            requested_caps: inst.package().requested_caps(),
            granted_caps: inst.granted_caps(),
            memory_base: 0,
            memory_len: env.mem_bytes as usize,
            entry_id: env.entry_point,
            launch_nonce,
        }
    }

    pub fn package_id(&self) -> AppId {
        self.package_id
    }
    pub fn package_version(&self) -> AppVersion {
        self.package_version
    }
    pub fn signer_key_id(&self) -> [u8; 32] {
        self.signer_key_id
    }
    pub fn developer_cert_hash(&self) -> StateHash {
        self.developer_cert_hash
    }
    pub fn manifest_root(&self) -> StateHash {
        self.manifest_root
    }
    pub fn payload_root(&self) -> StateHash {
        self.payload_root
    }
    pub fn registry_generation(&self) -> u64 {
        self.registry_generation
    }
    pub fn install_sequence(&self) -> u64 {
        self.install_sequence
    }
    pub fn requested_caps(&self) -> u32 {
        self.requested_caps
    }
    pub fn granted_caps(&self) -> u32 {
        self.granted_caps
    }
    pub fn memory_base(&self) -> usize {
        self.memory_base
    }
    pub fn memory_len(&self) -> usize {
        self.memory_len
    }
    pub fn entry_id(&self) -> u32 {
        self.entry_id
    }
    pub fn launch_nonce(&self) -> [u8; LAUNCH_NONCE_LEN] {
        self.launch_nonce
    }
    pub fn envelope(&self) -> ExecEnvelope {
        let mut env = ExecEnvelope::default();
        env.entry_point = self.entry_id;
        env.mem_bytes = self.memory_len as u32;
        env.publisher_key_id = self.signer_key_id;
        env
    }

    #[cfg(test)]
    pub(super) fn with_payload_root(mut self, payload_root: StateHash) -> Self {
        self.payload_root = payload_root;
        self
    }

    #[cfg(test)]
    pub(super) fn with_manifest_root(mut self, manifest_root: StateHash) -> Self {
        self.manifest_root = manifest_root;
        self
    }

    #[cfg(test)]
    pub(super) fn with_registry_generation(mut self, generation: u64) -> Self {
        self.registry_generation = generation;
        self
    }

    #[cfg(test)]
    pub(super) fn with_install_sequence(mut self, sequence: u64) -> Self {
        self.install_sequence = sequence;
        self
    }

    #[cfg(test)]
    pub(super) fn with_granted_caps(mut self, granted_caps: u32) -> Self {
        self.granted_caps = granted_caps;
        self
    }

    #[cfg(test)]
    pub(super) fn with_signer_key_id(mut self, signer_key_id: [u8; 32]) -> Self {
        self.signer_key_id = signer_key_id;
        self
    }

    #[cfg(test)]
    pub(super) fn with_launch_nonce(mut self, launch_nonce: [u8; LAUNCH_NONCE_LEN]) -> Self {
        self.launch_nonce = launch_nonce;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::phasestack::capability::GrantSet;
    use crate::kernel::phasestack::types::{
        AppExecutionKind, AppVersion, ABI_VERSION_V1, InstallScope, PKG_TYPE_APP, VerifiedPackage,
    };
    use crate::kernel::phasestack::verifier::commit_installation;

    #[test]
    fn ticket_has_no_public_constructor() {
        let pkg = VerifiedPackage::from_parts(
            [1u8; 32],
            AppVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            [2u8; 32],
            [3u8; 32],
            [4u8; 32],
            [5u8; 32],
            [6u8; 32],
            [7u8; 32],
            AppExecutionKind::PhaseVm,
            0,
            0,
            ABI_VERSION_V1,
            "t".into(),
            crate::platform::Vec::new(),
            PKG_TYPE_APP,
            ExecEnvelope::default(),
        );
        let inst = commit_installation(
            pkg,
            GrantSet::empty(),
            InstallScope::User,
            "/x".into(),
        )
        .unwrap();
        let t = VerifiedLaunchTicket::from_installation(&inst, [9u8; 16]);
        assert_eq!(t.package_id(), [1u8; 32]);
        assert_eq!(t.payload_root(), [3u8; 32]);
        assert_eq!(t.manifest_root(), [4u8; 32]);
    }
}
