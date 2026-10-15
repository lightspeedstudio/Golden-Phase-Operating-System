//! One-use Sentinel launch permit and structured GPA-path decisions.

use crate::kernel::hash::StateHash;
use crate::kernel::phasestack::ticket::VerifiedLaunchTicket;
use crate::kernel::phasestack::types::{
    AppId, AppState, PHASEVM_ENTRY, PHASEVM_MAX_MEM, MAX_NONCES, MAX_PERMITS, LAUNCH_NONCE_LEN,
};
use crate::platform::Vec;

use super::types::Rights;
use super::PhaseSentinel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SentinelRefusal {
    RegistryMissing,
    NotInstalled,
    PackageRevoked,
    PackageQuarantined,
    SignerUnknown,
    SignerRetired,
    ManifestRootMismatch,
    PayloadRootMismatch,
    RegistryGenerationMismatch,
    InstallSequenceMismatch,
    CapabilityEscalation,
    MemoryEnvelopeInvalid,
    EntryNotDeclared,
    NonceReplayed,
    PermitConsumed,
    KernelStateInvalid,
    ProcessAlreadyRunning,
    BoundExceeded,
}

impl SentinelRefusal {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RegistryMissing => "RegistryMissing",
            Self::NotInstalled => "NotInstalled",
            Self::PackageRevoked => "PackageRevoked",
            Self::PackageQuarantined => "PackageQuarantined",
            Self::SignerUnknown => "SignerUnknown",
            Self::SignerRetired => "SignerRetired",
            Self::ManifestRootMismatch => "ManifestRootMismatch",
            Self::PayloadRootMismatch => "PayloadRootMismatch",
            Self::RegistryGenerationMismatch => "RegistryGenerationMismatch",
            Self::InstallSequenceMismatch => "InstallSequenceMismatch",
            Self::CapabilityEscalation => "CapabilityEscalation",
            Self::MemoryEnvelopeInvalid => "MemoryEnvelopeInvalid",
            Self::EntryNotDeclared => "EntryNotDeclared",
            Self::NonceReplayed => "NonceReplayed",
            Self::PermitConsumed => "PermitConsumed",
            Self::KernelStateInvalid => "KernelStateInvalid",
            Self::ProcessAlreadyRunning => "ProcessAlreadyRunning",
            Self::BoundExceeded => "BoundExceeded",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SentinelIsolation {
    pub reason: SentinelRefusal,
}

/// One-use permit. Not Clone. Spawn path consumes it.
#[derive(Debug, PartialEq, Eq)]
pub struct SentinelLaunchPermit {
    package_id: AppId,
    payload_root: StateHash,
    registry_generation: u64,
    granted_caps: Rights,
    memory_len: usize,
    launch_nonce: [u8; LAUNCH_NONCE_LEN],
    permit_sequence: u64,
}

impl SentinelLaunchPermit {
    fn mint(
        package_id: AppId,
        payload_root: StateHash,
        registry_generation: u64,
        granted_caps: Rights,
        memory_len: usize,
        launch_nonce: [u8; LAUNCH_NONCE_LEN],
        permit_sequence: u64,
    ) -> Self {
        Self {
            package_id,
            payload_root,
            registry_generation,
            granted_caps,
            memory_len,
            launch_nonce,
            permit_sequence,
        }
    }

    pub fn package_id(&self) -> AppId {
        self.package_id
    }
    pub fn payload_root(&self) -> StateHash {
        self.payload_root
    }
    pub fn registry_generation(&self) -> u64 {
        self.registry_generation
    }
    pub fn granted_caps(&self) -> Rights {
        self.granted_caps
    }
    pub fn memory_len(&self) -> usize {
        self.memory_len
    }
    pub fn launch_nonce(&self) -> [u8; LAUNCH_NONCE_LEN] {
        self.launch_nonce
    }
    pub fn permit_sequence(&self) -> u64 {
        self.permit_sequence
    }
}

pub enum SentinelLaunchDecision {
    Allow(SentinelLaunchPermit),
    Refuse(SentinelRefusal),
    Isolate(SentinelIsolation),
}

impl SentinelLaunchDecision {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Allow(_) => "Allow",
            Self::Refuse(r) => r.as_str(),
            Self::Isolate(i) => i.reason.as_str(),
        }
    }
}

/// Live registry/signer snapshot built by PhaseStack. Not a public ticket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveLaunchView {
    pub present: bool,
    pub state: AppState,
    pub package_id: AppId,
    pub package_version_major: u16,
    pub package_version_minor: u16,
    pub package_version_patch: u16,
    pub signer_key_id: [u8; 32],
    pub developer_cert_hash: StateHash,
    pub manifest_root: StateHash,
    pub payload_root: StateHash,
    pub registry_generation: u64,
    pub install_sequence: u64,
    pub requested_caps: u32,
    pub granted_caps: u32,
    pub mem_bytes: u32,
    pub entry_point: u32,
    pub signer_known: bool,
    pub signer_retired: bool,
    pub signer_revoked: bool,
    pub package_revoked: bool,
    pub policy_caps: u32,
    pub kernel_permits_launch: bool,
}

#[derive(Debug, Default)]
pub struct PermitTable {
    next_sequence: u64,
    consumed_sequences: Vec<u64>,
    consumed_nonces: Vec<[u8; LAUNCH_NONCE_LEN]>,
}

impl PermitTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.consumed_sequences.len()
    }

    pub fn nonce_len(&self) -> usize {
        self.consumed_nonces.len()
    }

    fn nonce_used(&self, nonce: &[u8; LAUNCH_NONCE_LEN]) -> bool {
        self.consumed_nonces.iter().any(|n| n == nonce)
    }

    fn sequence_used(&self, seq: u64) -> bool {
        self.consumed_sequences.iter().any(|s| *s == seq)
    }

    pub fn invalidate_generation(&mut self, _generation: u64) {
        self.consumed_sequences.clear();
        self.consumed_nonces.clear();
    }

    pub fn full(&self) -> bool {
        self.consumed_nonces.len() >= MAX_NONCES || self.consumed_sequences.len() >= MAX_PERMITS
    }
}

impl PhaseSentinel {
    pub fn permit_table(&self) -> &PermitTable {
        &self.permits
    }

    pub fn permit_table_mut(&mut self) -> &mut PermitTable {
        &mut self.permits
    }

    /// Fail-closed GPA launch gate. Never falls through to spawn.
    pub fn authorize_verified_ticket(
        &mut self,
        ticket: &VerifiedLaunchTicket,
        live: &LiveLaunchView,
    ) -> SentinelLaunchDecision {
        if !live.present {
            return SentinelLaunchDecision::Refuse(SentinelRefusal::RegistryMissing);
        }
        if live.package_revoked || live.state == AppState::Revoked {
            return SentinelLaunchDecision::Refuse(SentinelRefusal::PackageRevoked);
        }
        if live.state == AppState::Quarantined {
            return SentinelLaunchDecision::Refuse(SentinelRefusal::PackageQuarantined);
        }
        if live.state == AppState::Running {
            return SentinelLaunchDecision::Refuse(SentinelRefusal::ProcessAlreadyRunning);
        }
        if live.state != AppState::Installed && live.state != AppState::Stopped {
            return SentinelLaunchDecision::Refuse(SentinelRefusal::NotInstalled);
        }
        if live.signer_retired {
            return SentinelLaunchDecision::Refuse(SentinelRefusal::SignerRetired);
        }
        if live.signer_revoked {
            return SentinelLaunchDecision::Refuse(SentinelRefusal::SignerRetired);
        }
        if !live.signer_known {
            return SentinelLaunchDecision::Refuse(SentinelRefusal::SignerUnknown);
        }
        if ticket.package_id() != live.package_id
            || ticket.package_version().major != live.package_version_major
            || ticket.package_version().minor != live.package_version_minor
            || ticket.package_version().patch != live.package_version_patch
        {
            return SentinelLaunchDecision::Refuse(SentinelRefusal::NotInstalled);
        }
        if ticket.signer_key_id() != live.signer_key_id {
            return SentinelLaunchDecision::Refuse(SentinelRefusal::SignerUnknown);
        }
        if ticket.manifest_root() != live.manifest_root {
            return SentinelLaunchDecision::Refuse(SentinelRefusal::ManifestRootMismatch);
        }
        if ticket.payload_root() != live.payload_root {
            return SentinelLaunchDecision::Refuse(SentinelRefusal::PayloadRootMismatch);
        }
        if ticket.registry_generation() != live.registry_generation {
            return SentinelLaunchDecision::Refuse(SentinelRefusal::RegistryGenerationMismatch);
        }
        if ticket.install_sequence() != live.install_sequence {
            return SentinelLaunchDecision::Refuse(SentinelRefusal::InstallSequenceMismatch);
        }
        if ticket.requested_caps() != live.requested_caps {
            return SentinelLaunchDecision::Refuse(SentinelRefusal::CapabilityEscalation);
        }
        let granted = ticket.granted_caps();
        let requested = ticket.requested_caps();
        let policy = live.policy_caps;
        if granted & !requested != 0 || granted & !policy != 0 {
            return SentinelLaunchDecision::Refuse(SentinelRefusal::CapabilityEscalation);
        }
        if ticket.memory_len() == 0
            || ticket.memory_len() > PHASEVM_MAX_MEM as usize
            || live.mem_bytes == 0
            || live.mem_bytes > PHASEVM_MAX_MEM
            || ticket.memory_len() != live.mem_bytes as usize
        {
            return SentinelLaunchDecision::Refuse(SentinelRefusal::MemoryEnvelopeInvalid);
        }
        if ticket.entry_id() != PHASEVM_ENTRY || live.entry_point != PHASEVM_ENTRY {
            return SentinelLaunchDecision::Refuse(SentinelRefusal::EntryNotDeclared);
        }
        if self.permits.nonce_used(&ticket.launch_nonce()) {
            return SentinelLaunchDecision::Refuse(SentinelRefusal::NonceReplayed);
        }
        if !live.kernel_permits_launch {
            return SentinelLaunchDecision::Refuse(SentinelRefusal::KernelStateInvalid);
        }
        if self.permits.full() {
            return SentinelLaunchDecision::Isolate(SentinelIsolation {
                reason: SentinelRefusal::BoundExceeded,
            });
        }
        self.permits.next_sequence = self.permits.next_sequence.saturating_add(1);
        let permit = SentinelLaunchPermit::mint(
            ticket.package_id(),
            ticket.payload_root(),
            ticket.registry_generation(),
            granted,
            ticket.memory_len(),
            ticket.launch_nonce(),
            self.permits.next_sequence,
        );
        SentinelLaunchDecision::Allow(permit)
    }

    /// Consume a permit at spawn. One-use.
    pub fn consume_permit(
        &mut self,
        permit: SentinelLaunchPermit,
        live_generation: u64,
        revoked: bool,
    ) -> Result<SentinelLaunchPermit, SentinelRefusal> {
        if revoked {
            return Err(SentinelRefusal::PackageRevoked);
        }
        if permit.registry_generation != live_generation {
            return Err(SentinelRefusal::RegistryGenerationMismatch);
        }
        if self.permits.sequence_used(permit.permit_sequence) {
            return Err(SentinelRefusal::PermitConsumed);
        }
        if self.permits.nonce_used(&permit.launch_nonce) {
            return Err(SentinelRefusal::NonceReplayed);
        }
        if self.permits.consumed_sequences.len() >= MAX_PERMITS
            || self.permits.consumed_nonces.len() >= MAX_NONCES
        {
            return Err(SentinelRefusal::BoundExceeded);
        }
        self.permits.consumed_sequences.push(permit.permit_sequence);
        self.permits.consumed_nonces.push(permit.launch_nonce);
        Ok(permit)
    }
}
