//! PHI-STABILITY fault injection.

use crate::kernel::hash::StateHash;
use crate::kernel::phi_stability::error::PhiStabilityError;
use crate::kernel::phi_stability::types::{PhiFaultType, PhiStabilityNetworkMessage};

pub fn flip_result_bit(hash: &mut StateHash) {
    hash[0] ^= 0x01;
}

pub fn apply_fault_to_message(
    message: &mut PhiStabilityNetworkMessage,
    fault: PhiFaultType,
) -> Result<(), PhiStabilityError> {
    match fault {
        PhiFaultType::ResultBit => flip_result_bit(&mut message.result_hash),
        PhiFaultType::PairHash => flip_result_bit(&mut message.pair_hash),
        PhiFaultType::TrajectoryHash => flip_result_bit(&mut message.trajectory_root),
        PhiFaultType::PreviousFrameHash => flip_result_bit(&mut message.previous_frame_hash),
        PhiFaultType::MerkleLeaf => flip_result_bit(&mut message.test_root),
        PhiFaultType::SequenceNumber => {
            message.sequence_number = message.sequence_number.wrapping_add(1)
        }
        PhiFaultType::Replay
        | PhiFaultType::TruncatedFrame
        | PhiFaultType::ChronosTick
        | PhiFaultType::KairosSkip
        | PhiFaultType::KairosDuplicate
        | PhiFaultType::StateClockSector
        | PhiFaultType::StateClockEpsilon
        | PhiFaultType::PhaseTransition => {}
    }
    Ok(())
}

pub fn fault_label(fault: PhiFaultType) -> &'static str {
    match fault {
        PhiFaultType::ResultBit => "result-bit",
        PhiFaultType::PairHash => "pair-hash",
        PhiFaultType::TrajectoryHash => "trajectory-hash",
        PhiFaultType::ChronosTick => "chronos-tick",
        PhiFaultType::KairosSkip => "kairos-skip",
        PhiFaultType::KairosDuplicate => "kairos-duplicate",
        PhiFaultType::StateClockSector => "stateclock-sector",
        PhiFaultType::StateClockEpsilon => "stateclock-epsilon",
        PhiFaultType::PhaseTransition => "phase-transition",
        PhiFaultType::SequenceNumber => "sequence-number",
        PhiFaultType::PreviousFrameHash => "previous-frame-hash",
        PhiFaultType::MerkleLeaf => "merkle-leaf",
        PhiFaultType::Replay => "replay",
        PhiFaultType::TruncatedFrame => "truncated-frame",
    }
}
