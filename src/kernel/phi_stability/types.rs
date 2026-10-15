//! PHI-STABILITY core types.

use crate::core::gphase_core::PhaseState;
use crate::kernel::hash::StateHash;
use crate::platform::{String, Vec};

pub const PHI_STABILITY_STATECHAIN_OBJECT_ID: u32 = 7;
pub const PHI_STABILITY_VERSION: u8 = 1;
pub const PHI_STABILITY_MAX_HISTORY: usize = 32;
pub const PHI_STABILITY_MAX_STEPS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhiTestId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhiTestMode {
    Local,
    DualClock,
    Batch,
    ScalarComparison,
    NetworkSender,
    NetworkReceiver,
    FaultInjection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhiIntegerMode {
    ExactHosted,
    FixedWidthBaremetal { bits: u16 },
    ModularBaremetal,
}

impl PhiIntegerMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ExactHosted => "exact-hosted",
            Self::FixedWidthBaremetal { bits: 1024 } => "fixed-1024",
            Self::FixedWidthBaremetal { .. } => "fixed-width",
            Self::ModularBaremetal => "modular",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopologyRelation {
    Same,
    Reflection,
}

#[derive(Debug, Clone)]
pub struct FibonacciPair<T> {
    pub current: T,
    pub next: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhiKairosEventKind {
    TestCreated,
    OperandsInitialized,
    DoublingStepCompleted,
    CalculationCompleted,
    ResultVerified,
    TrajectorySealed,
    MerkleCommitted,
    NetworkFrameCreated,
    NetworkFrameSent,
    NetworkFrameReceived,
    RemoteResultVerified,
    TestCompleted,
    FaultInjected,
    FrameQuarantined,
    TestFailed,
}

#[derive(Debug, Clone)]
pub struct PhiKairosEvent {
    pub kind: PhiKairosEventKind,
    pub chronos_tick: u64,
    pub kairos_tick: u64,
}

#[derive(Debug, Clone)]
pub struct PhiStepRecord {
    pub test_id: PhiTestId,
    pub step_index: u32,
    pub input_bit: bool,
    pub previous_phase: PhaseState,
    pub next_phase: PhaseState,
    pub topology_relation: TopologyRelation,
    pub chi: i8,
    pub chronos_tick: u64,
    pub kairos_tick: u64,
    pub stateclock_sector: i8,
    pub stateclock_epsilon: i64,
    pub pair_commitment: StateHash,
    pub previous_step_hash: StateHash,
    pub step_hash: StateHash,
}

#[derive(Debug, Clone)]
pub struct PhiStabilityResult {
    pub test_id: PhiTestId,
    pub target_index: u64,
    pub mode: PhiTestMode,
    pub integer_mode: PhiIntegerMode,
    pub recurrence_steps: u32,
    pub digit_count: Option<u64>,
    pub result_preview: Option<String>,
    pub result_hash: StateHash,
    pub pair_hash: StateHash,
    pub trajectory_root: StateHash,
    pub clock_root: StateHash,
    pub merkle_root: StateHash,
    pub test_root: StateHash,
    pub chronos_start: u64,
    pub chronos_finish: u64,
    pub chronos_elapsed: u64,
    pub kairos_start: u64,
    pub kairos_finish: u64,
    pub kairos_events: u64,
    pub phase_transitions: u64,
    pub invalid_transitions: u64,
    pub scalar_match: Option<bool>,
    pub remote_match: Option<bool>,
    pub proof_valid: bool,
    pub chain_valid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhiFaultType {
    ResultBit,
    PairHash,
    TrajectoryHash,
    ChronosTick,
    KairosSkip,
    KairosDuplicate,
    StateClockSector,
    StateClockEpsilon,
    PhaseTransition,
    SequenceNumber,
    PreviousFrameHash,
    MerkleLeaf,
    Replay,
    TruncatedFrame,
}

impl PhiFaultType {
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "result-bit" => Some(Self::ResultBit),
            "pair-hash" => Some(Self::PairHash),
            "trajectory" | "trajectory-hash" => Some(Self::TrajectoryHash),
            "chronos" | "chronos-tick" => Some(Self::ChronosTick),
            "kairos-skip" => Some(Self::KairosSkip),
            "kairos-duplicate" => Some(Self::KairosDuplicate),
            "sector" | "stateclock-sector" => Some(Self::StateClockSector),
            "epsilon" | "stateclock-epsilon" => Some(Self::StateClockEpsilon),
            "phase" | "phase-transition" => Some(Self::PhaseTransition),
            "sequence" | "sequence-number" => Some(Self::SequenceNumber),
            "previous-hash" | "previous-frame-hash" => Some(Self::PreviousFrameHash),
            "merkle" | "merkle-leaf" => Some(Self::MerkleLeaf),
            "replay" => Some(Self::Replay),
            "truncate" | "truncated-frame" => Some(Self::TruncatedFrame),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhiNetworkMessageKind {
    TestRequest = 1,
    TestResult = 2,
    VerificationResult = 3,
    FaultResult = 4,
}

#[derive(Debug, Clone)]
pub struct PhiStabilityNetworkMessage {
    pub version: u8,
    pub message_kind: PhiNetworkMessageKind,
    pub source_node: StateHash,
    pub destination_node: StateHash,
    pub equation_root: StateHash,
    pub boot_session_root: StateHash,
    pub test_id: PhiTestId,
    pub target_index: u64,
    pub integer_mode: PhiIntegerMode,
    pub result_hash: StateHash,
    pub pair_hash: StateHash,
    pub trajectory_root: StateHash,
    pub clock_root: StateHash,
    pub test_root: StateHash,
    pub recurrence_steps: u32,
    pub chronos_elapsed: u64,
    pub kairos_events: u64,
    pub sequence_number: u64,
    pub previous_frame_hash: StateHash,
    pub frame_hash: StateHash,
}

#[derive(Debug, Clone)]
pub struct PhiStateEvent {
    pub operation: String,
    pub test_id: PhiTestId,
    pub target_index: u64,
    pub chronos_tick: u64,
    pub kairos_tick: u64,
    pub event_hash: StateHash,
    pub previous_event_hash: StateHash,
}

#[derive(Debug)]
pub struct PhiStabilityController {
    pub next_test_id: u64,
    pub current_phase: PhaseState,
    pub tests_completed: u64,
    pub tests_failed: u64,
    pub faults_detected: u64,
    pub faults_missed: u64,
    pub network_tests: u64,
    pub remote_matches: u64,
    pub replays_quarantined: u64,
    pub invalid_transitions: u64,
    pub last_target: u64,
    pub last_result_hash: StateHash,
    pub last_trajectory_root: StateHash,
    pub last_chronos_elapsed: u64,
    pub last_kairos_events: u64,
    pub pending_fault: Option<PhiFaultType>,
    pub last_network_message: Option<PhiStabilityNetworkMessage>,
    pub history: Vec<PhiStabilityResult>,
    pub kairos_log: Vec<PhiKairosEvent>,
    pub state_events: Vec<PhiStateEvent>,
    pub integer_mode: PhiIntegerMode,
}

impl Default for PhiStabilityController {
    fn default() -> Self {
        Self::new()
    }
}

impl PhiStabilityController {
    pub fn new() -> Self {
        Self {
            next_test_id: 0,
            current_phase: PhaseState::Phi0,
            tests_completed: 0,
            tests_failed: 0,
            faults_detected: 0,
            faults_missed: 0,
            network_tests: 0,
            remote_matches: 0,
            replays_quarantined: 0,
            invalid_transitions: 0,
            last_target: 0,
            last_result_hash: [0u8; 32],
            last_trajectory_root: [0u8; 32],
            last_chronos_elapsed: 0,
            last_kairos_events: 0,
            pending_fault: None,
            last_network_message: None,
            history: Vec::new(),
            kairos_log: Vec::new(),
            state_events: Vec::new(),
            integer_mode: default_integer_mode(),
        }
    }

    pub fn allocate_test_id(&mut self) -> PhiTestId {
        self.next_test_id += 1;
        PhiTestId(self.next_test_id)
    }

    pub fn push_history(&mut self, result: PhiStabilityResult) {
        if self.history.len() >= PHI_STABILITY_MAX_HISTORY {
            self.history.remove(0);
        }
        self.history.push(result);
    }
}

pub fn default_integer_mode() -> PhiIntegerMode {
    #[cfg(feature = "std")]
    {
        PhiIntegerMode::ExactHosted
    }
    #[cfg(all(feature = "baremetal", not(feature = "std")))]
    {
        PhiIntegerMode::FixedWidthBaremetal { bits: 1024 }
    }
    #[cfg(not(any(feature = "std", feature = "baremetal")))]
    {
        PhiIntegerMode::ExactHosted
    }
}
