//! Bounded proof queue.

use crate::platform::Vec;

pub const PROOF_QUEUE_CAPACITY: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofWorkItem {
    pub object_id: u32,
    pub sequence_hint: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ProofPipeline {
    pub queue: Vec<ProofWorkItem>,
    pub accepted: u64,
    pub dropped: u64,
}

impl ProofPipeline {
    pub fn push(&mut self, item: ProofWorkItem) {
        if self.queue.len() >= PROOF_QUEUE_CAPACITY {
            self.queue.remove(0);
            self.dropped += 1;
        }
        self.queue.push(item);
        self.accepted += 1;
    }
}
