//! Bounded evidence ring. Drops are counted, never silent.

use crate::kernel::crypto::phase_hash::ph_sentinel_evidence;
use crate::kernel::hash::StateHash;
use crate::platform::Vec;

use super::decision::SentinelDecision;
use super::event::SentinelEvent;

pub const MAX_EVIDENCE: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceRecord {
    pub event: SentinelEvent,
    pub decision: SentinelDecision,
}

pub struct EvidenceRing {
    records: Vec<EvidenceRecord>,
    drop_count: u64,
    sequence: u64,
}

impl EvidenceRing {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            drop_count: 0,
            sequence: 0,
        }
    }

    pub fn push(&mut self, record: EvidenceRecord) {
        self.sequence = self.sequence.saturating_add(1);
        if self.records.len() >= MAX_EVIDENCE {
            self.records.remove(0);
            self.drop_count = self.drop_count.saturating_add(1);
        }
        self.records.push(record);
    }

    pub fn drop_count(&self) -> u64 {
        self.drop_count
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn records(&self) -> &[EvidenceRecord] {
        &self.records
    }

    pub fn head_hash(&self) -> StateHash {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.sequence.to_le_bytes());
        buf.extend_from_slice(&self.drop_count.to_le_bytes());
        if let Some(last) = self.records.last() {
            buf.push(last.decision.as_u8());
            buf.push(last.event.kind.as_u8());
            buf.extend_from_slice(&last.event.pid.to_le_bytes());
            buf.extend_from_slice(&last.event.nr.to_le_bytes());
        }
        ph_sentinel_evidence(&buf)
    }
}

impl Default for EvidenceRing {
    fn default() -> Self {
        Self::new()
    }
}
