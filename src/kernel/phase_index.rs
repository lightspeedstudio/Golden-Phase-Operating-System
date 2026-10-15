//! PhaseIndex — additive multidimensional index over `PhaseFs` inodes.
//!
//! Partitions inodes by `(state, Kairos-sector, Chronos-bucket)` so lookups
//! like "every Phi1 file written in Kairos-sector 2 during Chronos ticks
//! 100..200" are a handful of `BTreeMap` range lookups instead of a full
//! filesystem walk. `PhaseIndex` is purely additive bookkeeping: `PhaseFs`
//! remains the source of truth for inode contents, and every mutation that
//! changes an inode's `(tag, kairos_sector, chronos)` must call `insert`,
//! `remove`, or `relocate` here to stay consistent — see
//! `kernel::syscall::FS_MKDIR`/`FS_WRITE_FILE`/`FS_RM`/`FS_LOCK`/`FS_UNLOCK`.

use crate::core::gphase_core::PhaseState;
use crate::fs::InodeId;
use crate::platform::{BTreeMap, Vec};

/// Width of one Chronos bucket, in raw `chronos_tick` units. Chosen so a
/// `query_range` spanning a handful of buckets stays a handful of BTreeMap
/// lookups even over a long-running Chronos horizon.
pub const CHRONOS_BUCKET_WIDTH: u64 = 16;

fn chronos_bucket(chronos: u64) -> u64 {
    chronos / CHRONOS_BUCKET_WIDTH
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct IndexKey {
    state: u8,
    kairos_sector: u8,
    chronos_bucket: u64,
}

impl IndexKey {
    fn new(state: PhaseState, kairos_sector: u8, chronos: u64) -> Self {
        Self {
            state: state.as_u8(),
            kairos_sector,
            chronos_bucket: chronos_bucket(chronos),
        }
    }
}

/// Partitioned index: `(state, Kairos-sector, Chronos-bucket) -> [InodeId]`,
/// plus a reverse map so `remove`/`relocate` never need a linear scan.
#[derive(Debug, Default)]
pub struct PhaseIndexController {
    buckets: BTreeMap<IndexKey, Vec<InodeId>>,
    located: BTreeMap<InodeId, IndexKey>,
}

impl PhaseIndexController {
    pub fn new() -> Self {
        Self {
            buckets: BTreeMap::new(),
            located: BTreeMap::new(),
        }
    }

    /// Index `inode` under `(state, kairos_sector, chronos)`. If `inode` is
    /// already indexed, its old entry is removed first (idempotent re-insert).
    pub fn insert(&mut self, inode: InodeId, state: PhaseState, kairos_sector: u8, chronos: u64) {
        self.remove(inode);
        let key = IndexKey::new(state, kairos_sector, chronos);
        self.buckets.entry(key).or_default().push(inode);
        self.located.insert(inode, key);
    }

    /// Drop `inode` from the index. Returns `true` if it was present.
    pub fn remove(&mut self, inode: InodeId) -> bool {
        let Some(key) = self.located.remove(&inode) else {
            return false;
        };
        if let Some(bucket) = self.buckets.get_mut(&key) {
            bucket.retain(|&id| id != inode);
            if bucket.is_empty() {
                self.buckets.remove(&key);
            }
        }
        true
    }

    /// Move `inode` to a new `(state, kairos_sector, chronos)` partition.
    /// Equivalent to `remove` + `insert`, exposed separately because a
    /// relocate is the common case (lock/unlock, re-tag, re-stamp).
    pub fn relocate(&mut self, inode: InodeId, state: PhaseState, kairos_sector: u8, chronos: u64) {
        self.insert(inode, state, kairos_sector, chronos);
    }

    /// All inodes tagged `state`, in Kairos-sector `kairos_sector`, whose
    /// Chronos tick falls in `[chronos_start, chronos_end]` inclusive.
    ///
    /// Walks only buckets that actually exist via `BTreeMap::range` — cost is
    /// `O(log n + k)` in the number of *populated* buckets touched, not in
    /// `chronos_end - chronos_start` (which callers may legitimately pass as
    /// something as wide as `0..=u64::MAX` for "everything in this sector").
    pub fn query_range(
        &self,
        state: PhaseState,
        kairos_sector: u8,
        chronos_start: u64,
        chronos_end: u64,
    ) -> Vec<InodeId> {
        let start_key = IndexKey {
            state: state.as_u8(),
            kairos_sector,
            chronos_bucket: chronos_bucket(chronos_start),
        };
        let end_key = IndexKey {
            state: state.as_u8(),
            kairos_sector,
            chronos_bucket: chronos_bucket(chronos_end),
        };
        let mut out = Vec::new();
        for (_, ids) in self.buckets.range(start_key..=end_key) {
            out.extend(ids.iter().copied());
        }
        out
    }

    /// Number of currently-indexed inodes.
    pub fn len(&self) -> usize {
        self.located.len()
    }

    pub fn is_empty(&self) -> bool {
        self.located.is_empty()
    }

    /// Whether `inode` is currently indexed.
    pub fn contains(&self, inode: InodeId) -> bool {
        self.located.contains_key(&inode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_then_query_range_finds_the_bucket() {
        let mut idx = PhaseIndexController::new();
        idx.insert(1, PhaseState::Phi1, 2, 50);
        let hits = idx.query_range(PhaseState::Phi1, 2, 0, 100);
        assert_eq!(hits, Vec::from([1]));
    }

    #[test]
    fn query_range_excludes_other_state_sector_or_bucket() {
        let mut idx = PhaseIndexController::new();
        idx.insert(1, PhaseState::Phi1, 2, 50);
        assert!(idx.query_range(PhaseState::Phi2, 2, 0, 100).is_empty());
        assert!(idx.query_range(PhaseState::Phi1, 3, 0, 100).is_empty());
        assert!(idx
            .query_range(PhaseState::Phi1, 2, 0, chronos_bucket_end_before(50))
            .is_empty());
    }

    fn chronos_bucket_end_before(chronos: u64) -> u64 {
        // Largest chronos value that still falls one bucket short of `chronos`.
        chronos_bucket(chronos) * CHRONOS_BUCKET_WIDTH - 1
    }

    #[test]
    fn remove_drops_from_bucket_and_reverse_map() {
        let mut idx = PhaseIndexController::new();
        idx.insert(1, PhaseState::Phi1, 0, 0);
        assert!(idx.contains(1));
        assert!(idx.remove(1));
        assert!(!idx.contains(1));
        assert!(idx.query_range(PhaseState::Phi1, 0, 0, 0).is_empty());
        assert!(idx.is_empty());
    }

    #[test]
    fn remove_missing_inode_returns_false() {
        let mut idx = PhaseIndexController::new();
        assert!(!idx.remove(999));
    }

    #[test]
    fn relocate_moves_inode_between_buckets() {
        let mut idx = PhaseIndexController::new();
        idx.insert(1, PhaseState::Phi1, 0, 0);
        idx.relocate(1, PhaseState::Phi3, 5, 200);
        assert!(idx.query_range(PhaseState::Phi1, 0, 0, 0).is_empty());
        assert_eq!(
            idx.query_range(PhaseState::Phi3, 5, 200, 200),
            Vec::from([1])
        );
        assert_eq!(idx.len(), 1);
    }

    #[test]
    fn multiple_inodes_share_a_bucket() {
        let mut idx = PhaseIndexController::new();
        idx.insert(1, PhaseState::Phi1, 0, 10);
        idx.insert(2, PhaseState::Phi1, 0, 11);
        let mut hits = idx.query_range(PhaseState::Phi1, 0, 0, 20);
        hits.sort_unstable();
        assert_eq!(hits, Vec::from([1, 2]));
        assert_eq!(idx.len(), 2);
    }

    #[test]
    fn reinserting_an_inode_moves_it_not_duplicates_it() {
        let mut idx = PhaseIndexController::new();
        idx.insert(1, PhaseState::Phi1, 0, 0);
        idx.insert(1, PhaseState::Phi1, 0, 100);
        assert_eq!(idx.len(), 1);
        assert!(idx.query_range(PhaseState::Phi1, 0, 0, 0).is_empty());
        assert_eq!(
            idx.query_range(PhaseState::Phi1, 0, 100, 100),
            Vec::from([1])
        );
    }

    #[test]
    fn query_range_spans_multiple_buckets() {
        let mut idx = PhaseIndexController::new();
        idx.insert(1, PhaseState::Phi0, 1, 0);
        idx.insert(2, PhaseState::Phi0, 1, CHRONOS_BUCKET_WIDTH * 3);
        let mut hits = idx.query_range(PhaseState::Phi0, 1, 0, CHRONOS_BUCKET_WIDTH * 3);
        hits.sort_unstable();
        assert_eq!(hits, Vec::from([1, 2]));
    }
}
