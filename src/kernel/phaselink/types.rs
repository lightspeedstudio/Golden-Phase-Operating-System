//! PhaseLink core types.

use crate::core::gphase_core::PhaseState;
use crate::kernel::authority::StateHash;
use crate::kernel::state_proof::{ObjectKind, StateTransitionProof};
use crate::platform::{String, Vec};

pub type NodeId = StateHash;
pub type Hash = StateHash;
pub type OperationId = String;
pub type ObjectId = String;

pub const PHASELINK_MAGIC: [u8; 4] = *b"KPL1";
pub const PHASELINK_VERSION: u8 = 1;
pub const PHASELINK_STATECHAIN_OBJECT_ID: u32 = 6;
pub const PHASELINK_MAX_FRAME_SIZE: usize = 1024 * 1024;
pub const PHASELINK_MAX_PAYLOAD_SIZE: usize = 256 * 1024;
pub const PHASELINK_MAX_OBJECT_ID_LEN: usize = 256;
pub const PHASELINK_MAX_PROOF_LEN: usize = 64 * 1024;
pub const PHASELINK_MAX_QUARANTINE: usize = 256;
pub const PHASELINK_MAX_QUEUE: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PhaseSymbol {
    Phi0 = 0,
    Phi1 = 1,
    Phi2 = 2,
    Phi3 = 3,
}

impl PhaseSymbol {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Phi0),
            1 => Some(Self::Phi1),
            2 => Some(Self::Phi2),
            3 => Some(Self::Phi3),
            _ => None,
        }
    }

    pub fn as_phase_state(self) -> PhaseState {
        PhaseState::from_u8(self as u8).expect("valid phase symbol")
    }
}

impl From<PhaseState> for PhaseSymbol {
    fn from(value: PhaseState) -> Self {
        match value {
            PhaseState::Phi0 => Self::Phi0,
            PhaseState::Phi1 => Self::Phi1,
            PhaseState::Phi2 => Self::Phi2,
            PhaseState::Phi3 => Self::Phi3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PhaseStream {
    pub symbols: Vec<PhaseSymbol>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PhaseLinkFlags {
    pub anom_public: bool,
    pub acknowledgement: bool,
    pub quarantine_notice: bool,
    pub proof_required: bool,
    pub loopback: bool,
    pub simulation: bool,
}

impl PhaseLinkFlags {
    pub fn to_bits(self) -> u8 {
        (self.anom_public as u8)
            | ((self.acknowledgement as u8) << 1)
            | ((self.quarantine_notice as u8) << 2)
            | ((self.proof_required as u8) << 3)
            | ((self.loopback as u8) << 4)
            | ((self.simulation as u8) << 5)
    }

    pub fn from_bits(bits: u8) -> Option<Self> {
        if bits & 0b1100_0000 != 0 {
            return None;
        }
        Some(Self {
            anom_public: bits & 1 != 0,
            acknowledgement: bits & (1 << 1) != 0,
            quarantine_notice: bits & (1 << 2) != 0,
            proof_required: bits & (1 << 3) != 0,
            loopback: bits & (1 << 4) != 0,
            simulation: bits & (1 << 5) != 0,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PayloadEncoding {
    RawBinary = 1,
    PhaseStream = 2,
    StateEvent = 3,
    PublicCommitment = 4,
}

impl PayloadEncoding {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::RawBinary),
            2 => Some(Self::PhaseStream),
            3 => Some(Self::StateEvent),
            4 => Some(Self::PublicCommitment),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EquationRootPolicy {
    ExactMatch = 1,
    AllowListed = 2,
    QuarantineUnknown = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SequenceGapPolicy {
    Reject = 1,
    Quarantine = 2,
    AcceptAndRecordGap = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GatewayState {
    Disabled = 0,
    RootAuthorized = 1,
    Active = 2,
    Quarantined = 3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseGatewayPolicy {
    pub state: GatewayState,
    pub allow_binary_input: bool,
    pub allow_binary_output: bool,
    pub require_root_authority: bool,
    pub allow_anom_decryption: bool,
}

impl Default for PhaseGatewayPolicy {
    fn default() -> Self {
        Self {
            state: GatewayState::Disabled,
            allow_binary_input: false,
            allow_binary_output: false,
            require_root_authority: true,
            allow_anom_decryption: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSequenceState {
    pub source_node: NodeId,
    pub boot_session_root: Hash,
    pub last_sequence_number: u64,
    pub last_frame_hash: Hash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseLinkObjectKind {
    Process,
    MemoryPage,
    File,
    Directory,
    Block,
    Syscall,
    Vm,
    Driver,
    Craft,
}

impl From<ObjectKind> for PhaseLinkObjectKind {
    fn from(value: ObjectKind) -> Self {
        match value {
            ObjectKind::Process => Self::Process,
            ObjectKind::MemoryPage => Self::MemoryPage,
            ObjectKind::File => Self::File,
            ObjectKind::Directory => Self::Directory,
            ObjectKind::Block => Self::Block,
            ObjectKind::Syscall => Self::Syscall,
            ObjectKind::Vm => Self::Vm,
            ObjectKind::Driver => Self::Driver,
            ObjectKind::Craft => Self::Craft,
            #[cfg(feature = "phasestack")]
            ObjectKind::App => Self::Craft,
        }
    }
}

impl From<PhaseLinkObjectKind> for ObjectKind {
    fn from(value: PhaseLinkObjectKind) -> Self {
        match value {
            PhaseLinkObjectKind::Process => Self::Process,
            PhaseLinkObjectKind::MemoryPage => Self::MemoryPage,
            PhaseLinkObjectKind::File => Self::File,
            PhaseLinkObjectKind::Directory => Self::Directory,
            PhaseLinkObjectKind::Block => Self::Block,
            PhaseLinkObjectKind::Syscall => Self::Syscall,
            PhaseLinkObjectKind::Vm => Self::Vm,
            PhaseLinkObjectKind::Driver => Self::Driver,
            PhaseLinkObjectKind::Craft => Self::Craft,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseLinkFrame {
    pub version: u8,
    pub flags: PhaseLinkFlags,
    pub source_node: NodeId,
    pub destination_node: NodeId,
    pub equation_root: Hash,
    pub boot_session_root: Hash,
    pub sequence_number: u64,
    pub chronos_tick: u64,
    pub kairos_tick: u64,
    pub stateclock_sector: i8,
    pub object_kind: PhaseLinkObjectKind,
    pub object_id: ObjectId,
    pub operation: OperationId,
    pub previous_phase: PhaseState,
    pub next_phase: PhaseState,
    pub previous_frame_hash: Hash,
    pub transition_proof: StateTransitionProof,
    pub payload_encoding: PayloadEncoding,
    pub payload: Vec<u8>,
    pub payload_hash: Hash,
    pub public_commitment: Hash,
    pub frame_hash: Hash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseLinkContext {
    pub source_node: NodeId,
    pub destination_node: NodeId,
    pub equation_root: Hash,
    pub boot_session_root: Hash,
    pub sequence_number: u64,
    pub chronos_tick: u64,
    pub kairos_tick: u64,
    pub stateclock_sector: i8,
    pub previous_frame_hash: Hash,
    pub payload_encoding: PayloadEncoding,
    pub flags: PhaseLinkFlags,
}
