//! Bounded Merkle leaf queue.

use crate::platform::Vec;

pub const MERKLE_QUEUE_CAPACITY: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MerkleLeafWorkItem {
    pub object_id: u32,
}

#[derive(Debug, Clone, Default)]
pub struct MerklePipeline {
    pub queue: Vec<MerkleLeafWorkItem>,
    pub accepted: u64,
    pub dropped: u64,
}

impl MerklePipeline {
    pub fn push(&mut self, item: MerkleLeafWorkItem) {
        if self.queue.len() >= MERKLE_QUEUE_CAPACITY {
            self.queue.remove(0);
            self.dropped += 1;
        }
        self.queue.push(item);
        self.accepted += 1;
    }
}
