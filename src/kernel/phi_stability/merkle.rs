//! PHI-STABILITY Merkle roots.

use crate::kernel::hash::{hash_pair, tagged_hash, StateHash};
use crate::platform::Vec;

const MERKLE_TAG: &str = "KAIROS_MERKLE_NODE_V1";

pub fn merkle_root(leaves: &[StateHash]) -> StateHash {
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

pub fn short_hash(hash: &StateHash) -> crate::platform::String {
    format!(
        "{:02X}{:02X}:{:02X}{:02X}",
        hash[0], hash[1], hash[2], hash[3]
    )
}
