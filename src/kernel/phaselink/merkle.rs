//! PhaseLink Merkle root.

use crate::kernel::hash::{hash_pair, tagged_hash, StateHash};
use crate::kernel::phaselink::types::NodeId;
use crate::platform::Vec;

const MERKLE_TAG: &str = "KAIROS_MERKLE_NODE_V1";

pub const PHASELINK_LEAF_TAG: &str = "KAIROS_PHASELINK_LEAF_V1";

pub fn leaf_hash(
    source_node: NodeId,
    destination_node: NodeId,
    sequence_number: u64,
    frame_hash: StateHash,
    chain_head: StateHash,
    status: &str,
) -> StateHash {
    tagged_hash(
        PHASELINK_LEAF_TAG,
        &[
            &source_node,
            &destination_node,
            &sequence_number.to_be_bytes(),
            &frame_hash,
            &chain_head,
            status.as_bytes(),
        ],
    )
}

pub fn phaselink_root(leaves: &[StateHash]) -> StateHash {
    merkle_root(leaves)
}

fn merkle_root(leaves: &[StateHash]) -> StateHash {
    if leaves.is_empty() {
        return tagged_hash(MERKLE_TAG, &[&[0u8; 32]]);
    }
    let mut level: Vec<StateHash> = leaves.to_vec();
    while level.len() > 1 {
        let mut next = Vec::new();
        let mut i = 0;
        while i < level.len() {
            let left = level[i];
            let right = if i + 1 < level.len() {
                level[i + 1]
            } else {
                level[i]
            };
            next.push(hash_pair(MERKLE_TAG, &left, &right));
            i += 2;
        }
        level = next;
    }
    level[0]
}

pub fn short_root(hash: &StateHash) -> crate::platform::String {
    format!(
        "{:02X}{:02X}:{:02X}{:02X}",
        hash[0], hash[1], hash[2], hash[3]
    )
}

pub fn recompute_from_events(leaves: &[StateHash]) -> StateHash {
    let filtered = leaves
        .iter()
        .copied()
        .filter(|h| *h != [0u8; 32])
        .collect::<Vec<_>>();
    phaselink_root(&filtered)
}
