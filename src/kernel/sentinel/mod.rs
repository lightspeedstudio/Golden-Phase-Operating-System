//! PhaseSentinel (`feature = "phasesentinel"`).
//!
//! Opt-in, fail-closed launch and observe path. Dedicated signer table
//! (not `identity::CertificateRegistry`). Ed25519 via `kernel::crypto::sign`.
//! `R_equation` / `R_kernel` are MEASURED_ROOT. `K_authority` is AUTHENTICATED
//! after Ed25519 verify against a pinned Sentinel trust-anchor public key.
//! Nothing here is HARDWARE_ROOTED.
//!
//! Quarantine/isolate deschedule, revoke, close, deny relaunch, and keep
//! evidence. They do **not** claim address-space containment.

pub mod checkpoint;
pub mod decision;
pub mod event;
pub mod evidence;
pub mod launch;
pub mod manifest;
#[cfg(feature = "phasestack")]
pub mod permit;
pub mod policy;
pub mod root;
pub mod score;
pub mod signer;
pub mod types;

pub use checkpoint::SentinelCheckpoint;
pub use decision::SentinelDecision;
pub use event::{EventKind, SentinelEvent};
pub use evidence::{EvidenceRecord, MAX_EVIDENCE};
pub use manifest::ProgramManifest;
#[cfg(feature = "phasestack")]
pub use permit::{
    LiveLaunchView, PermitTable, SentinelIsolation, SentinelLaunchDecision, SentinelLaunchPermit,
    SentinelRefusal,
};
pub use policy::SentinelPolicy;
pub use signer::SignerTable;
pub use types::{project_s, Rights, SchedulerSnapshot, SentinelState};

use crate::core::gphase_core::PhaseState;
use crate::kernel::event_log::ProcessId;
use crate::kernel::hash::StateHash;
use crate::platform::{BTreeMap, Vec};

pub const SENTINEL_EVIDENCE_OBJECT_ID: u32 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SentinelError {
    Unauthorized,
    Unsigned,
    BadSignature,
    Revoked,
    Expired,
    Generation,
    RightsEscalation,
    Quarantined,
    Isolated,
    NoAnchor,
    NoCheckpoint,
    CheckpointInvalid,
    MmioDenied,
    PhaseIllegal,
}

impl SentinelError {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unauthorized => "sentinel: unauthorized",
            Self::Unsigned => "sentinel: unsigned program",
            Self::BadSignature => "sentinel: bad signature",
            Self::Revoked => "sentinel: signer revoked",
            Self::Expired => "sentinel: expired",
            Self::Generation => "sentinel: generation rejected",
            Self::RightsEscalation => "sentinel: rights escalation",
            Self::Quarantined => "sentinel: quarantined",
            Self::Isolated => "sentinel: isolated",
            Self::NoAnchor => "sentinel: no trust anchor",
            Self::NoCheckpoint => "sentinel: no verified checkpoint",
            Self::CheckpointInvalid => "sentinel: checkpoint invalid",
            Self::MmioDenied => "sentinel: mmio denied",
            Self::PhaseIllegal => "sentinel: illegal phase transition",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskDisposition {
    None,
    Quarantined,
    Isolated,
}

pub struct CachedLaunch {
    pub content_hash: StateHash,
    pub signer: [u8; 32],
    pub generation: u64,
}

pub struct PhaseSentinel {
    pub signers: SignerTable,
    pub policy: SentinelPolicy,
    pub evidence: evidence::EvidenceRing,
    pub generation: u64,
    local_epoch: u64,
    observe_count: u64,
    dispositions: BTreeMap<ProcessId, TaskDisposition>,
    blocked_content: BTreeMap<StateHash, TaskDisposition>,
    cached: BTreeMap<ProcessId, CachedLaunch>,
    checkpoints: Vec<SentinelCheckpoint>,
    score: score::SoftScore,
    manifests: BTreeMap<StateHash, ProgramManifest>,
    #[cfg(feature = "phasestack")]
    permits: PermitTable,
}

impl PhaseSentinel {
    pub fn new() -> Self {
        Self {
            signers: SignerTable::new(),
            policy: SentinelPolicy::deny_all(),
            evidence: evidence::EvidenceRing::new(),
            generation: 0,
            local_epoch: 0,
            observe_count: 0,
            dispositions: BTreeMap::new(),
            blocked_content: BTreeMap::new(),
            cached: BTreeMap::new(),
            checkpoints: Vec::new(),
            score: score::SoftScore::new(),
            manifests: BTreeMap::new(),
            #[cfg(feature = "phasestack")]
            permits: PermitTable::new(),
        }
    }

    pub fn observe_count(&self) -> u64 {
        self.observe_count
    }

    pub fn local_epoch(&self) -> u64 {
        self.local_epoch
    }

    pub fn bump_local_epoch(&mut self) {
        self.local_epoch = self.local_epoch.wrapping_add(1);
    }

    pub fn scheduler_snapshot(&self, sched2_elapsed: Option<u64>) -> SchedulerSnapshot {
        SchedulerSnapshot {
            epoch: sched2_elapsed.unwrap_or(self.local_epoch),
        }
    }

    pub fn q_t(&self, s_t: PhaseState, m_t: SchedulerSnapshot, v_t: Rights) -> SentinelState {
        SentinelState { s_t, m_t, v_t }
    }

    pub fn disposition(&self, pid: ProcessId) -> TaskDisposition {
        self.dispositions
            .get(&pid)
            .copied()
            .unwrap_or(TaskDisposition::None)
    }

    pub fn content_disposition(&self, content: &StateHash) -> TaskDisposition {
        self.blocked_content
            .get(content)
            .copied()
            .unwrap_or(TaskDisposition::None)
    }

    pub fn set_disposition(&mut self, pid: ProcessId, d: TaskDisposition) {
        match d {
            TaskDisposition::None => {
                self.dispositions.remove(&pid);
            }
            _ => {
                self.dispositions.insert(pid, d);
            }
        }
    }

    pub fn block_content(&mut self, content: StateHash, d: TaskDisposition) {
        self.blocked_content.insert(content, d);
    }

    pub fn clear_content_block(&mut self, content: &StateHash) {
        self.blocked_content.remove(content);
    }

    pub fn cache_launch(&mut self, pid: ProcessId, cached: CachedLaunch) {
        self.cached.insert(pid, cached);
    }

    pub fn cached_launch(&self, pid: ProcessId) -> Option<&CachedLaunch> {
        self.cached.get(&pid)
    }

    pub fn drop_cache(&mut self, pid: ProcessId) {
        self.cached.remove(&pid);
    }

    pub fn record_observe(&mut self, event: SentinelEvent) {
        self.observe_count = self.observe_count.saturating_add(1);
        self.evidence.push(EvidenceRecord {
            event,
            decision: SentinelDecision::Allow,
        });
    }

    pub fn record_decision(&mut self, event: SentinelEvent, decision: SentinelDecision) {
        self.observe_count = self.observe_count.saturating_add(1);
        self.evidence.push(EvidenceRecord { event, decision });
    }

    pub fn push_checkpoint(&mut self, ck: SentinelCheckpoint) {
        self.checkpoints.push(ck);
    }

    pub fn checkpoints(&self) -> &[SentinelCheckpoint] {
        &self.checkpoints
    }

    pub fn latest_checkpoint(&self) -> Option<&SentinelCheckpoint> {
        self.checkpoints.last()
    }

    pub fn score_mut(&mut self) -> &mut score::SoftScore {
        &mut self.score
    }

    pub fn score(&self) -> &score::SoftScore {
        &self.score
    }

    pub fn register_manifest(&mut self, manifest: ProgramManifest) {
        self.manifests.insert(manifest.content_hash, manifest);
    }

    pub fn manifest_for(&self, content: &StateHash) -> Option<&ProgramManifest> {
        self.manifests.get(content)
    }
}

impl Default for PhaseSentinel {
    fn default() -> Self {
        Self::new()
    }
}

/// Non-kernel MMIO is deny-by-default. No per-process allow-list exists.
pub fn authorize_mmio(_pid: ProcessId) -> Result<(), SentinelError> {
    Err(SentinelError::MmioDenied)
}
