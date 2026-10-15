//! State transition proofs and event hashing.

use crate::core::gphase_core::{compose, topology_status, PhaseState, TopologyKind};
use crate::kernel::hash::{tagged_hash, StateHash};
use crate::platform::String;

pub const EVENT_TAG: &str = "KAIROS_STATE_EVENT_V1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectKind {
    Process,
    MemoryPage,
    File,
    Directory,
    Block,
    Syscall,
    Vm,
    Driver,
    /// Craft Mainframe objects (appended; preserves 0..=7 discriminants).
    Craft,
    /// PhaseStack verified apps. Present only with `feature = "phasestack"`
    /// so sealed-off builds do not grow this enum.
    #[cfg(feature = "phasestack")]
    App,
}

impl ObjectKind {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Process => 0,
            Self::MemoryPage => 1,
            Self::File => 2,
            Self::Directory => 3,
            Self::Block => 4,
            Self::Syscall => 5,
            Self::Vm => 6,
            Self::Driver => 7,
            Self::Craft => 8,
            #[cfg(feature = "phasestack")]
            Self::App => 9,
        }
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Process),
            1 => Some(Self::MemoryPage),
            2 => Some(Self::File),
            3 => Some(Self::Directory),
            4 => Some(Self::Block),
            5 => Some(Self::Syscall),
            6 => Some(Self::Vm),
            7 => Some(Self::Driver),
            8 => Some(Self::Craft),
            #[cfg(feature = "phasestack")]
            9 => Some(Self::App),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopologyProofStatus {
    None,
    Same,
    Reflection,
}

impl TopologyProofStatus {
    pub fn from_triple(a: PhaseState, b: PhaseState, c: PhaseState) -> Self {
        if a == b && b == c {
            return TopologyProofStatus::None;
        }
        match topology_status(a, b, c).kind {
            TopologyKind::Same => TopologyProofStatus::Same,
            TopologyKind::Reflection => TopologyProofStatus::Reflection,
        }
    }

    pub fn as_u8(self) -> u8 {
        match self {
            TopologyProofStatus::None => 0,
            TopologyProofStatus::Same => 1,
            TopologyProofStatus::Reflection => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateTransitionProof {
    pub object_kind: ObjectKind,
    pub object_id: u32,
    pub sequence_number: u64,
    pub previous_state: PhaseState,
    pub requested_state: PhaseState,
    pub operation_name: String,
    pub compose_result: Option<PhaseState>,
    pub topology_status: TopologyProofStatus,
    pub payload_hash: StateHash,
    pub previous_event_hash: StateHash,
    pub equation_root: StateHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofError {
    StateMismatch,
    ComposeMismatch,
    TopologyMismatch,
    EquationRootMismatch,
    PreviousHashMismatch,
}

pub fn hash_payload(data: &[u8]) -> StateHash {
    tagged_hash("KAIROS_PAYLOAD_V1", &[data])
}

pub fn compute_event_hash(proof: &StateTransitionProof, assoc_parity: u8) -> StateHash {
    let compose_byte = [proof.compose_result.map(|s| s.as_u8()).unwrap_or(255)];
    tagged_hash(
        EVENT_TAG,
        &[
            &proof.equation_root,
            &proof.previous_event_hash,
            &[proof.object_kind.as_u8()],
            &proof.object_id.to_le_bytes(),
            &proof.sequence_number.to_le_bytes(),
            &[proof.previous_state.as_u8()],
            &[proof.requested_state.as_u8()],
            proof.operation_name.as_bytes(),
            &compose_byte,
            &[proof.topology_status.as_u8()],
            &[assoc_parity],
            &proof.payload_hash,
        ],
    )
}

pub fn verify_proof(
    proof: &StateTransitionProof,
    expected_prev_hash: &StateHash,
) -> Result<(), ProofError> {
    if proof.previous_event_hash != *expected_prev_hash {
        return Err(ProofError::PreviousHashMismatch);
    }
    if proof.equation_root == [0u8; 32] {
        return Err(ProofError::EquationRootMismatch);
    }
    Ok(())
}

pub fn verify_compose_result(
    proof: &StateTransitionProof,
    left: PhaseState,
    right: PhaseState,
) -> Result<(), ProofError> {
    let Some(result) = proof.compose_result else {
        return Ok(());
    };
    if result != compose(left, right) {
        return Err(ProofError::ComposeMismatch);
    }
    Ok(())
}

pub fn verify_event_hash(
    proof: &StateTransitionProof,
    assoc_parity: u8,
    event_hash: &StateHash,
) -> bool {
    &compute_event_hash(proof, assoc_parity) == event_hash
}
