//! Batch interface and lanes.

use crate::core::gphase_core::PhaseState;
use crate::platform::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchLane {
    Immediate,
    Trajectory,
    Proofs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseOp {
    pub left: PhaseState,
    pub right: PhaseState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopologyOp {
    pub a: PhaseState,
    pub b: PhaseState,
    pub c: PhaseState,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PhaseBatchInput {
    pub lane: Option<BatchLane>,
    pub phase_ops: Vec<PhaseOp>,
    pub topology_ops: Vec<TopologyOp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PhaseBatchOutput {
    pub phase_results: Vec<PhaseState>,
    pub topology_results: Vec<u8>,
}

impl PhaseBatchInput {
    pub fn from_pairs(ops: &[PhaseOp], lane: BatchLane) -> Self {
        Self {
            lane: Some(lane),
            phase_ops: ops.to_vec(),
            topology_ops: Vec::new(),
        }
    }
}
