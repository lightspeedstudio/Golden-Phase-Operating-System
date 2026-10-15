//! State change proofs and virtual state instances.

use crate::core::gphase_core::PhaseState;
use crate::kernel::authority::StateHash;
use crate::kernel::state_domain::domain::{DomainId, DomainStateId, StateDomain};
use crate::kernel::state_domain::encoding::PhaseWord;
use crate::platform::String;
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::platform::Vec;
use crate::stateclock::StateClockSnapshot;

#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::hash::tagged_hash;
#[cfg(any(feature = "std", feature = "baremetal"))]
use crate::kernel::state_proof::ObjectKind;

pub const STATE_CHANGE_PROOF_TAG: &str = "KAIROS_STATE_CHANGE_PROOF_V1";
pub const STATEDOMAIN_STATECHAIN_OBJECT_ID: u32 = 3;
/// First dense driver-head slot available for per-object domain chains.
pub const STATEDOMAIN_FIRST_OBJECT_SLOT: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateChangeMode {
    Root4,
    DomainExpanded,
    Adaptive,
}

impl StateChangeMode {
    pub fn label(&self) -> &'static str {
        match self {
            StateChangeMode::Root4 => "root4",
            StateChangeMode::DomainExpanded => "expanded",
            StateChangeMode::Adaptive => "adaptive",
        }
    }

    pub fn parse(token: &str) -> Option<Self> {
        match token.to_ascii_lowercase().as_str() {
            "root4" | "root" => Some(StateChangeMode::Root4),
            "expanded" | "domain" => Some(StateChangeMode::DomainExpanded),
            "adaptive" => Some(StateChangeMode::Adaptive),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VirtualStateInstance {
    pub object_key: String,
    pub driver_chain_id: u32,
    pub domain_id: DomainId,
    pub virtual_state_id: DomainStateId,
    pub virtual_state_label: String,
    pub phase_word: PhaseWord,
    pub current_root_phase: PhaseState,
    pub last_proof_hash: StateHash,
    pub last_event_hash: StateHash,
}

#[derive(Debug, Clone)]
pub struct StateChangeProof {
    pub equation_root: StateHash,
    pub domain_root: StateHash,
    pub object_kind: u8,
    pub object_id: u32,
    pub old_virtual_state: String,
    pub old_phase_word: PhaseWord,
    pub new_virtual_state: String,
    pub new_phase_word: PhaseWord,
    pub operation: String,
    pub stateclock_snapshot: StateClockSnapshot,
    pub previous_event_hash: StateHash,
    pub proof_hash: StateHash,
}

#[cfg(any(feature = "std", feature = "baremetal"))]
#[allow(clippy::too_many_arguments)]
pub fn compute_state_change_proof_hash(
    equation_root: &StateHash,
    domain_root: &StateHash,
    object_kind: ObjectKind,
    object_id: u32,
    old_virtual_state: &str,
    old_phase_word: &[PhaseState],
    operation: &str,
    new_virtual_state: &str,
    new_phase_word: &[PhaseState],
    snapshot: &StateClockSnapshot,
    previous_event_hash: &StateHash,
) -> StateHash {
    let mut buf = Vec::new();
    buf.extend_from_slice(equation_root);
    buf.extend_from_slice(domain_root);
    buf.push(object_kind as u8);
    buf.extend_from_slice(&object_id.to_le_bytes());
    buf.extend_from_slice(old_virtual_state.as_bytes());
    for p in old_phase_word {
        buf.push(p.as_u8());
    }
    buf.extend_from_slice(operation.as_bytes());
    buf.extend_from_slice(new_virtual_state.as_bytes());
    for p in new_phase_word {
        buf.push(p.as_u8());
    }
    buf.extend_from_slice(&snapshot.proper_tick.to_le_bytes());
    buf.extend_from_slice(&snapshot.kairos_tick.to_le_bytes());
    buf.push(snapshot.sector_id);
    buf.extend_from_slice(&snapshot.x.to_le_bytes());
    buf.extend_from_slice(previous_event_hash);
    tagged_hash(STATE_CHANGE_PROOF_TAG, &[&buf])
}

#[cfg(any(feature = "std", feature = "baremetal"))]
pub fn verify_state_change_proof(proof: &StateChangeProof, object_kind: ObjectKind) -> bool {
    let expected = compute_state_change_proof_hash(
        &proof.equation_root,
        &proof.domain_root,
        object_kind,
        proof.object_id,
        &proof.old_virtual_state,
        &proof.old_phase_word,
        &proof.operation,
        &proof.new_virtual_state,
        &proof.new_phase_word,
        &proof.stateclock_snapshot,
        &proof.previous_event_hash,
    );
    expected == proof.proof_hash
}

#[cfg(any(feature = "std", feature = "baremetal"))]
#[allow(clippy::too_many_arguments)]
pub fn build_state_change_proof(
    equation_root: &StateHash,
    domain: &StateDomain,
    object_id: u32,
    old_label: &str,
    old_word: &[PhaseState],
    new_label: &str,
    new_word: &[PhaseState],
    operation: &str,
    snapshot: &StateClockSnapshot,
    previous_event_hash: &StateHash,
) -> StateChangeProof {
    let proof_hash = compute_state_change_proof_hash(
        equation_root,
        &domain.domain_root,
        ObjectKind::Driver,
        object_id,
        old_label,
        old_word,
        operation,
        new_label,
        new_word,
        snapshot,
        previous_event_hash,
    );
    StateChangeProof {
        equation_root: *equation_root,
        domain_root: domain.domain_root,
        object_kind: ObjectKind::Driver as u8,
        object_id,
        old_virtual_state: old_label.into(),
        old_phase_word: old_word.to_vec(),
        new_virtual_state: new_label.into(),
        new_phase_word: new_word.to_vec(),
        operation: operation.into(),
        stateclock_snapshot: *snapshot,
        previous_event_hash: *previous_event_hash,
        proof_hash,
    }
}

pub fn object_id_from_key(object_key: &str) -> u32 {
    #[cfg(feature = "std")]
    let hash = crate::kernel::state_proof::hash_payload(object_key.as_bytes());
    #[cfg(not(feature = "std"))]
    let hash = [0u8; 32];
    u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]])
}

pub fn infer_domain_from_object(object_key: &str) -> Option<DomainId> {
    let prefix = object_key.split('.').next()?;
    match prefix {
        "chem" | "chemistry" => Some(DomainId::Chemistry),
        "craft" => Some(DomainId::Craft),
        "net" | "network" => Some(DomainId::Network),
        "sec" | "security" => Some(DomainId::Security),
        "usb" => Some(DomainId::Usb),
        #[cfg(feature = "std")]
        other => Some(DomainId::Custom(other.into())),
        #[cfg(not(feature = "std"))]
        _ => None,
    }
}
