//! Trajectory and topology batch helpers.

use crate::core::gphase_core::{topology_status, TopologyKind};
use crate::kernel::phase_math::batch::TopologyOp;
use crate::platform::Vec;

pub fn process_trajectory(batch: &[TopologyOp]) -> Vec<u8> {
    batch
        .iter()
        .map(|op| match topology_status(op.a, op.b, op.c).kind {
            TopologyKind::Same => 0,
            TopologyKind::Reflection => 1,
        })
        .collect()
}
