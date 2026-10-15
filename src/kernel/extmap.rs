//! Storage allocation and extent map (`feature = "extmap"`), subsystem 6 of
//! the eight kernel-subsystems build program — see
//! `fpga/zynq/evidence/KERNEL_MATH_CONTRACT_2026-08-12.md` §0.7/§9 and
//! `fpga/zynq/evidence/EXTMAP_STATUS_<date>.md`.
//!
//! Gives RAM-backed `PhaseFs` a real, reusable, fixed-size block allocator
//! and multi-block extent mapping, without claiming persistent SD storage
//! (this pool is RAM-only, like `captable`/`irqreg`/`sched2`/`ipc`).
//!
//! ## Mathematics (per the build contract's Part V §6)
//! - Block state partition: `B = F ∪ R ∪ W ∪ L ∪ S ∪ D` (Free, Reserved,
//!   Writing, Live, Stale, Damaged) — [`BlockState`], disjoint by
//!   construction: every block index has exactly one state.
//! - An extent is `e = (l, p, n, h, g)`: logical start, physical start,
//!   block count, content root, generation — [`Extent`].
//! - Logical-to-physical mapping inside an extent: `physical(x) = p + (x -
//!   l)` for `l <= x < l+n` — [`Extmap::read_extents`] walks exactly this.
//! - Non-overlap for live extents: two live extents' physical ranges never
//!   intersect, because every allocation ([`Extmap::alloc_extent`], and the
//!   internal loop in [`Extmap::write_inode`]) only ever claims blocks
//!   currently `Free`, and a block leaves `Free` for the whole time any
//!   extent references it (see "Required proof" test coverage in this
//!   module for the standing invariant check).
//! - Placement: standard first-fit over the fixed block array (see module
//!   docs "Non-goals" below for why the full `Q(e)` scoring function is not
//!   implemented) — the contract's own text permits this: "begin with
//!   standard first-fit/best-fit unless measurements justify extra terms."
//! - Content identity: `h_b = PH-C(length|bytes)` via
//!   [`crate::kernel::crypto::phase_hash::ph_content`] (which already
//!   length-prefixes its input internally, per `tuple_hash`'s
//!   `write_len_prefixed` — no separate length encoding needed here).
//! - Reference-count safety: `refcount(b) > 0 => b not in Free` — enforced
//!   by [`Extmap::decref`], the only path that ever returns a block to
//!   `Free`.
//!
//! ## Phase/kernel integration
//! - `PhaseFs` remains authoritative for namespace/inodes (`mkdir`,
//!   `create_file`, `rm`, path resolution): this module never creates,
//!   renames, or deletes an inode. It only owns the *content bytes* for
//!   `InodeId`s `PhaseFs` already assigned, keyed by that same
//!   [`crate::fs::InodeId`] (reused, not reinvented — math contract §0.4).
//! - `kernel::syscall`'s `FS_WRITE_FILE`/`FS_READ_FILE`/`FS_RM` handlers
//!   route through [`Extmap::write_inode`]/[`Extmap::read_inode`]/
//!   [`Extmap::delete_inode`] when this feature is enabled, additively:
//!   `PhaseFs`'s own single-legacy-block write/read still runs unchanged
//!   underneath (so behavior with the feature off is byte-for-byte
//!   identical to before this module existed), and `PhaseIndex`/
//!   `PhaseCache` invalidation already wired into those handlers is
//!   untouched.
//! - PH-R ([`crate::kernel::crypto::phase_hash::ph_root`]) anchors each
//!   extent's `content_root` over its constituent blocks' PH-C hashes, at
//!   the write/split/coalesce/resize boundary only (cold path, never a
//!   per-tick hash) — matching the math contract's own row for `extmap`:
//!   "PH-C for block/extent content identity; PH-R may anchor ordered
//!   extent roots at audit points only. No G64 operation."
//! - [`Extmap::mark_damaged`] is the integration point a `PhaseGuard`
//!   observer *could* call to quarantine a suspect block; nothing in this
//!   module invokes it automatically (no automatic corruption-detection
//!   path exists to trigger it), matching `pmm::quarantine`'s identical
//!   "available, not auto-invoked" precedent.
//!
//! ## Complexity targets (per the build contract)
//! - Single-block status query ([`Extmap::block_state`]): `O(1)`.
//! - Per-file extent lookup: a file's own extent list
//!   ([`Extmap::read_inode`]/[`Extmap::write_inode`]) is a small `Vec`
//!   bounded by that file's own block count, walked directly — no unbounded
//!   scan over a numeric ID space is ever performed (the forbidden "loop
//!   from a numeric start bucket to `u64::MAX`" antipattern the contract
//!   warns against does not apply here: the block pool itself is a small,
//!   fixed-size array, not a sparse ID space).
//!
//! ## Non-goals (explicit — see also `EXTMAP_STATUS_<date>.md`)
//! - **No `Q(e)` placement-scoring function.** First-fit over the fixed
//!   block array is used unconditionally; the contract explicitly permits
//!   deferring `fragmentation`/`distance`/`wear`/`phaseMismatch` scoring
//!   "unless measurements justify extra terms," and no such measurement
//!   exists yet.
//! - **Content-hash deduplication is single-block-file only.** Sharing a
//!   block across two *different* multi-block files' extents would break
//!   the "extent = one contiguous physical run" invariant this module
//!   otherwise guarantees (a shared block in the middle of an extent is no
//!   longer contiguous with its neighbors under that file's own ownership).
//!   Deduplication is therefore scoped to whole files that fit in exactly
//!   one block — still a real, tested, math-contract-satisfying feature
//!   ("hash equality selects a dedup candidate; verify length and bytes
//!   before sharing"), just not applied where it would corrupt the extent
//!   model.
//! - Extmap is not its own WAL. The block pool is RAM. After PhaseFs
//!   journal recover, `Kernel::journal_post_recover` rebuilds extents from
//!   recovered file bytes so namespace and extents match. Files larger than
//!   the PhaseFs journal record are not durable here.
//! - `Damaged` is absorbing (no "un-damage" API), matching
//!   `pmm::quarantine`'s identical precedent — no hardware revalidation
//!   procedure is defined for either subsystem.
//! - This subsystem consumes no G64 operation and no `PhaseState`
//!   reflection quotient beyond PH-C/PH-R above — block allocation is not
//!   phase mathematics. Explicitly stated per Gate 0's instruction to
//!   declare this for every subsystem.

use core::fmt;

use crate::fs::InodeId;
use crate::kernel::crypto::phase_hash::{ph_content, ph_root};
use crate::kernel::hash::StateHash;
use crate::platform::{HashMap, Vec};

/// Fixed logical block size. Chosen (not mandated by the math contract,
/// which specifies no concrete size) to be small enough that host tests can
/// exercise real multi-block files without large buffers, while still being
/// a realistic filesystem block size.
pub const EXTMAP_BLOCK_SIZE: usize = 512;

/// Default fixed pool size for [`Extmap::default`] / `Kernel::new` (512 KiB
/// total). A real, bounded capacity — not unbounded growth — per the
/// contract's "give PhaseFS a real reusable block allocator" goal.
pub const DEFAULT_TOTAL_BLOCKS: u32 = 1024;

/// `B = F ∪ R ∪ W ∪ L ∪ S ∪ D`. Disjoint by construction: [`BlockSlot`]
/// carries exactly one of these at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockState {
    /// Unallocated, available to [`Extmap::alloc_extent`]/`write_inode`.
    Free,
    /// Claimed by an in-progress allocation, before its content is written.
    /// Never selected by the allocator (same disjointness as `Free`).
    Reserved,
    /// Content is being written (transient — [`Extmap::write_block_content`]
    /// moves a block through this state and out of it in the same call).
    Writing,
    /// Committed content, `refcount() > 0`.
    Live,
    /// `refcount()` reached zero; reclaimed to `Free` in the same call that
    /// dropped the last reference (see module docs "Non-goals": immediate
    /// reclaim, not a deferred garbage-collection pass).
    Stale,
    /// Quarantined by [`Extmap::mark_damaged`]. Absorbing — never
    /// reallocated.
    Damaged,
}

/// `e = (l, p, n, h, g)`: logical start, physical start, block count,
/// content root, generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extent {
    pub logical_start: u32,
    pub physical_start: u32,
    pub block_count: u32,
    pub content_root: StateHash,
    pub generation: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtmapError {
    /// Not enough free blocks existed to satisfy the request. No partial
    /// allocation is ever committed on this error.
    OutOfSpace,
    /// A block index, extent shape, or split point was invalid (out of
    /// range, zero-length, or a split point outside the extent's own
    /// logical range).
    InvalidExtent,
    /// `coalesce_extents` was called on two extents that are not
    /// immediately adjacent in both logical and physical space.
    NotAdjacent,
    /// `resize_extent` could not grow in place: the immediately-following
    /// blocks are not all `Free` (or run off the end of the pool). Callers
    /// must allocate a new extent and copy instead.
    CannotGrow,
    /// `read_inode`/`delete_inode` (no-op) found no extent record for this
    /// inode.
    NotFound,
    /// `mark_damaged` was called on a block that is currently `Reserved`,
    /// `Writing`, or `Live` (in active use).
    InUse,
    /// `mark_damaged`/`block_state` referenced a block index outside the
    /// pool.
    InvalidHandle,
}

impl fmt::Display for ExtmapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            ExtmapError::OutOfSpace => "out of space",
            ExtmapError::InvalidExtent => "invalid extent",
            ExtmapError::NotAdjacent => "extents are not adjacent",
            ExtmapError::CannotGrow => "cannot grow extent in place",
            ExtmapError::NotFound => "no extent record for this inode",
            ExtmapError::InUse => "block is in use",
            ExtmapError::InvalidHandle => "invalid block index",
        };
        f.write_str(msg)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ExtmapError {}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ExtmapStats {
    pub blocks_total: u32,
    pub blocks_free: u32,
    pub blocks_reserved: u32,
    pub blocks_writing: u32,
    pub blocks_live: u32,
    pub blocks_stale: u32,
    pub blocks_damaged: u32,
    pub allocations: u64,
    pub extent_writes: u64,
    pub writes: u64,
    pub reads: u64,
    pub deletes: u64,
    pub frees: u64,
    pub splits: u64,
    pub coalesces: u64,
    pub resizes: u64,
    pub dedup_hits: u64,
    pub out_of_space_rejections: u64,
}

#[derive(Debug, Default, Clone, Copy)]
struct ExtmapCounters {
    allocations: u64,
    extent_writes: u64,
    writes: u64,
    reads: u64,
    deletes: u64,
    frees: u64,
    splits: u64,
    coalesces: u64,
    resizes: u64,
    dedup_hits: u64,
    out_of_space_rejections: u64,
}

#[derive(Clone)]
struct BlockSlot {
    state: BlockState,
    data: [u8; EXTMAP_BLOCK_SIZE],
    /// Valid byte length in `data` (`<= EXTMAP_BLOCK_SIZE`; a trailing
    /// block of a file may be shorter).
    len: u16,
    refcount: u32,
    content_root: StateHash,
}

impl BlockSlot {
    fn empty() -> Self {
        Self {
            state: BlockState::Free,
            data: [0u8; EXTMAP_BLOCK_SIZE],
            len: 0,
            refcount: 0,
            content_root: StateHash::default(),
        }
    }
}

/// One `InodeId`'s stored content: its ordered extent list and the real
/// original byte length (needed to trim the last block's trailing padding
/// on read).
#[derive(Debug, Clone)]
struct FileExtents {
    extents: Vec<Extent>,
    total_len: usize,
}

fn blocks_needed(len: usize) -> u32 {
    (len.div_ceil(EXTMAP_BLOCK_SIZE)) as u32
}

/// Storage allocation and extent-map engine. See module docs for the full
/// design, math, and integration points.
pub struct Extmap {
    blocks: Vec<BlockSlot>,
    /// Content hash -> block index, for single-block dedup only (see module
    /// docs "Non-goals"). Bounded by `blocks.len()`: at most one entry per
    /// physical block ever exists.
    dedup_index: HashMap<StateHash, u32>,
    files: HashMap<InodeId, FileExtents>,
    next_generation: u32,
    counters: ExtmapCounters,
}

impl Default for Extmap {
    fn default() -> Self {
        Self::new(DEFAULT_TOTAL_BLOCKS)
    }
}

impl Extmap {
    pub fn new(total_blocks: u32) -> Self {
        let blocks = (0..total_blocks).map(|_| BlockSlot::empty()).collect();
        Self {
            blocks,
            dedup_index: HashMap::new(),
            files: HashMap::new(),
            next_generation: 0,
            counters: ExtmapCounters::default(),
        }
    }

    pub fn total_blocks(&self) -> u32 {
        self.blocks.len() as u32
    }

    /// `O(1)` single-block status query.
    pub fn block_state(&self, index: u32) -> Option<BlockState> {
        self.blocks.get(index as usize).map(|b| b.state)
    }

    pub fn has_inode(&self, inode_id: InodeId) -> bool {
        self.files.contains_key(&inode_id)
    }

    pub fn stats(&self) -> ExtmapStats {
        let mut s = ExtmapStats {
            blocks_total: self.blocks.len() as u32,
            allocations: self.counters.allocations,
            extent_writes: self.counters.extent_writes,
            writes: self.counters.writes,
            reads: self.counters.reads,
            deletes: self.counters.deletes,
            frees: self.counters.frees,
            splits: self.counters.splits,
            coalesces: self.counters.coalesces,
            resizes: self.counters.resizes,
            dedup_hits: self.counters.dedup_hits,
            out_of_space_rejections: self.counters.out_of_space_rejections,
            ..Default::default()
        };
        for b in &self.blocks {
            match b.state {
                BlockState::Free => s.blocks_free += 1,
                BlockState::Reserved => s.blocks_reserved += 1,
                BlockState::Writing => s.blocks_writing += 1,
                BlockState::Live => s.blocks_live += 1,
                BlockState::Stale => s.blocks_stale += 1,
                BlockState::Damaged => s.blocks_damaged += 1,
            }
        }
        s
    }

    /// PhaseGuard integration point (see module docs): quarantine a block
    /// that is not currently in active use. Absorbing — no un-damage API.
    pub fn mark_damaged(&mut self, index: u32) -> Result<(), ExtmapError> {
        let slot = self
            .blocks
            .get_mut(index as usize)
            .ok_or(ExtmapError::InvalidHandle)?;
        match slot.state {
            BlockState::Free | BlockState::Stale => {
                slot.state = BlockState::Damaged;
                slot.content_root = StateHash::default();
                slot.len = 0;
                Ok(())
            }
            _ => Err(ExtmapError::InUse),
        }
    }

    fn bump_generation(&mut self) -> u32 {
        let g = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1);
        g
    }

    /// First-fit: the start of the first run of `Free` blocks whose length
    /// is `>= needed`. Only ever considers `Free` blocks — `Reserved`/
    /// `Damaged`/every other state is skipped, satisfying "Reserved/damaged
    /// blocks never allocate" by construction.
    fn find_contiguous_free_run(&self, needed: u32) -> Option<u32> {
        if needed == 0 {
            return None;
        }
        let mut run_start: Option<u32> = None;
        let mut run_len: u32 = 0;
        for i in 0..self.blocks.len() as u32 {
            if self.blocks[i as usize].state == BlockState::Free {
                if run_start.is_none() {
                    run_start = Some(i);
                }
                run_len += 1;
                if run_len >= needed {
                    return run_start;
                }
            } else {
                run_start = None;
                run_len = 0;
            }
        }
        None
    }

    /// The largest contiguous run of `Free` blocks anywhere in the pool
    /// (used by `write_inode`'s multi-extent fallback when no single run
    /// covers the whole file).
    fn find_largest_free_run(&self) -> Option<(u32, u32)> {
        let mut best: Option<(u32, u32)> = None;
        let mut run_start: Option<u32> = None;
        let mut run_len: u32 = 0;
        let total = self.blocks.len() as u32;
        for i in 0..=total {
            let is_free = i < total && self.blocks[i as usize].state == BlockState::Free;
            if is_free {
                if run_start.is_none() {
                    run_start = Some(i);
                }
                run_len += 1;
            } else {
                if run_len > 0 && best.map(|(_, l)| run_len > l).unwrap_or(true) {
                    best = Some((
                        run_start.expect("run_len > 0 implies run_start is set"),
                        run_len,
                    ));
                }
                run_start = None;
                run_len = 0;
            }
        }
        best
    }

    /// Allocate one new, standalone, contiguous extent of exactly
    /// `block_count` blocks (`logical_start` always `0` — callers that need
    /// a different logical placement adjust the returned value directly).
    /// Marks the claimed blocks `Reserved`; call [`Extmap::write_extent_data`]
    /// to commit real content and move them to `Live`.
    pub fn alloc_extent(&mut self, block_count: u32) -> Result<Extent, ExtmapError> {
        if block_count == 0 {
            return Err(ExtmapError::InvalidExtent);
        }
        let start = self
            .find_contiguous_free_run(block_count)
            .ok_or(ExtmapError::OutOfSpace)?;
        for b in start..start + block_count {
            self.blocks[b as usize].state = BlockState::Reserved;
        }
        let generation = self.bump_generation();
        self.counters.allocations += 1;
        Ok(Extent {
            logical_start: 0,
            physical_start: start,
            block_count,
            content_root: StateHash::default(),
            generation,
        })
    }

    /// `Reserved -> Writing -> Live`, in one call: hashes `chunk` (PH-C),
    /// stores it, sets `refcount = 1`. Overwrites whatever was previously
    /// in this slot unconditionally — callers are responsible for having
    /// already `decref`'d any prior occupant.
    fn write_block_content(&mut self, index: u32, chunk: &[u8]) {
        let slot = &mut self.blocks[index as usize];
        slot.state = BlockState::Writing;
        let mut buf = [0u8; EXTMAP_BLOCK_SIZE];
        buf[..chunk.len()].copy_from_slice(chunk);
        slot.data = buf;
        slot.len = chunk.len() as u16;
        slot.content_root = ph_content(chunk);
        slot.refcount = 1;
        slot.state = BlockState::Live;
    }

    /// Write real content into every block of a `Reserved` (or already
    /// `Live`) extent, chunked at `EXTMAP_BLOCK_SIZE` boundaries, then
    /// anchor the extent's own `content_root` with PH-R over its blocks'
    /// PH-C hashes. `data.len()` must not exceed the extent's total byte
    /// capacity.
    pub fn write_extent_data(
        &mut self,
        extent: &mut Extent,
        data: &[u8],
    ) -> Result<(), ExtmapError> {
        let capacity = extent.block_count as usize * EXTMAP_BLOCK_SIZE;
        if data.len() > capacity {
            return Err(ExtmapError::InvalidExtent);
        }
        let mut leaves: Vec<StateHash> = Vec::new();
        let mut offset = 0usize;
        for b_off in 0..extent.block_count {
            let idx = extent.physical_start + b_off;
            let end = core::cmp::min(offset + EXTMAP_BLOCK_SIZE, data.len());
            let chunk = &data[offset..end];
            self.write_block_content(idx, chunk);
            leaves.push(self.blocks[idx as usize].content_root);
            offset = end;
        }
        extent.content_root = ph_root(&leaves);
        self.counters.extent_writes += 1;
        Ok(())
    }

    /// Drop one reference from every block in `extent`. A block reaching
    /// `refcount == 0` is reclaimed `-> Stale -> Free` immediately (see
    /// module docs "Non-goals": no deferred reclaim pass).
    pub fn free_extent(&mut self, extent: &Extent) -> Result<(), ExtmapError> {
        for b in extent.physical_start..extent.physical_start + extent.block_count {
            self.decref(b);
        }
        self.counters.frees += 1;
        Ok(())
    }

    fn decref(&mut self, index: u32) {
        let hash;
        let should_reclaim;
        {
            let slot = &mut self.blocks[index as usize];
            if slot.refcount > 0 {
                slot.refcount -= 1;
            }
            should_reclaim =
                slot.refcount == 0 && matches!(slot.state, BlockState::Live | BlockState::Writing);
            hash = slot.content_root;
        }
        if should_reclaim {
            if self.dedup_index.get(&hash) == Some(&index) {
                self.dedup_index.remove(&hash);
            }
            let slot = &mut self.blocks[index as usize];
            slot.state = BlockState::Stale;
            slot.state = BlockState::Free;
            slot.content_root = StateHash::default();
            slot.len = 0;
            slot.data = [0u8; EXTMAP_BLOCK_SIZE];
        }
    }

    fn incref(&mut self, index: u32) {
        self.blocks[index as usize].refcount += 1;
    }

    /// Split one extent into two at logical offset `at_logical` (must fall
    /// strictly inside the extent). Pure bookkeeping: no block state or
    /// refcount changes, since both halves still describe the exact same
    /// already-`Live` blocks under the same ownership. Each half gets a
    /// fresh generation and its own recomputed PH-R `content_root`.
    pub fn split_extent(
        &mut self,
        extent: &Extent,
        at_logical: u32,
    ) -> Result<(Extent, Extent), ExtmapError> {
        if at_logical <= extent.logical_start
            || at_logical >= extent.logical_start + extent.block_count
        {
            return Err(ExtmapError::InvalidExtent);
        }
        let left_len = at_logical - extent.logical_start;
        let right_len = extent.block_count - left_len;
        let left_leaves: Vec<StateHash> = (0..left_len)
            .map(|i| self.blocks[(extent.physical_start + i) as usize].content_root)
            .collect();
        let right_leaves: Vec<StateHash> = (0..right_len)
            .map(|i| self.blocks[(extent.physical_start + left_len + i) as usize].content_root)
            .collect();
        let left = Extent {
            logical_start: extent.logical_start,
            physical_start: extent.physical_start,
            block_count: left_len,
            content_root: ph_root(&left_leaves),
            generation: self.bump_generation(),
        };
        let right = Extent {
            logical_start: at_logical,
            physical_start: extent.physical_start + left_len,
            block_count: right_len,
            content_root: ph_root(&right_leaves),
            generation: self.bump_generation(),
        };
        self.counters.splits += 1;
        Ok((left, right))
    }

    /// Merge two extents that are immediately adjacent in *both* logical
    /// and physical space into one. Pure bookkeeping, like `split_extent`.
    pub fn coalesce_extents(&mut self, a: &Extent, b: &Extent) -> Result<Extent, ExtmapError> {
        let a_end_logical = a.logical_start + a.block_count;
        let a_end_physical = a.physical_start + a.block_count;
        if a_end_logical != b.logical_start || a_end_physical != b.physical_start {
            return Err(ExtmapError::NotAdjacent);
        }
        let combined_len = a.block_count + b.block_count;
        let leaves: Vec<StateHash> = (0..combined_len)
            .map(|i| self.blocks[(a.physical_start + i) as usize].content_root)
            .collect();
        let merged = Extent {
            logical_start: a.logical_start,
            physical_start: a.physical_start,
            block_count: combined_len,
            content_root: ph_root(&leaves),
            generation: self.bump_generation(),
        };
        self.counters.coalesces += 1;
        Ok(merged)
    }

    /// Grow (only into immediately-following, currently-`Free` blocks — no
    /// move, or else [`ExtmapError::CannotGrow`]; new blocks are committed
    /// `Live` with zero-filled content) or shrink (frees the trailing
    /// blocks via `decref`) an extent in place.
    pub fn resize_extent(
        &mut self,
        extent: &Extent,
        new_block_count: u32,
    ) -> Result<Extent, ExtmapError> {
        if new_block_count == 0 {
            return Err(ExtmapError::InvalidExtent);
        }
        if new_block_count == extent.block_count {
            return Ok(*extent);
        }
        if new_block_count > extent.block_count {
            let grow_by = new_block_count - extent.block_count;
            let grow_start = extent.physical_start + extent.block_count;
            let grow_end = grow_start + grow_by;
            if grow_end as usize > self.blocks.len() {
                return Err(ExtmapError::CannotGrow);
            }
            for b in grow_start..grow_end {
                if self.blocks[b as usize].state != BlockState::Free {
                    return Err(ExtmapError::CannotGrow);
                }
            }
            for b in grow_start..grow_end {
                self.blocks[b as usize].state = BlockState::Reserved;
                self.write_block_content(b, &[]);
            }
        } else {
            let shrink_start = extent.physical_start + new_block_count;
            let shrink_end = extent.physical_start + extent.block_count;
            for b in shrink_start..shrink_end {
                self.decref(b);
            }
        }
        let leaves: Vec<StateHash> = (0..new_block_count)
            .map(|i| self.blocks[(extent.physical_start + i) as usize].content_root)
            .collect();
        let resized = Extent {
            logical_start: extent.logical_start,
            physical_start: extent.physical_start,
            block_count: new_block_count,
            content_root: ph_root(&leaves),
            generation: self.bump_generation(),
        };
        self.counters.resizes += 1;
        Ok(resized)
    }

    /// Reconstruct the exact original bytes described by an ordered extent
    /// list (`physical(x) = p + (x - l)`, walked extent-by-extent,
    /// block-by-block), trimmed to `total_len`.
    pub fn read_extents(&self, extents: &[Extent], total_len: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(total_len);
        for extent in extents {
            for b_off in 0..extent.block_count {
                let idx = (extent.physical_start + b_off) as usize;
                let slot = &self.blocks[idx];
                let take = core::cmp::min(slot.len as usize, total_len.saturating_sub(out.len()));
                out.extend_from_slice(&slot.data[..take]);
            }
        }
        out
    }

    fn free_inode_extents(&mut self, inode_id: InodeId) {
        if let Some(fe) = self.files.remove(&inode_id) {
            for extent in &fe.extents {
                for b in extent.physical_start..extent.physical_start + extent.block_count {
                    self.decref(b);
                }
            }
        }
    }

    /// Store `data` as `inode_id`'s content, replacing whatever it held
    /// before (any previous extents are fully freed first). A `data.len()`
    /// of `0` stores an empty file without allocating any block. Files that
    /// fit in exactly one block are dedup-checked against every other
    /// single-block file's content (see module docs "Non-goals" for why
    /// multi-block files are never deduplicated). `OutOfSpace` is atomic:
    /// no partial reservation is ever left behind on failure.
    pub fn write_inode(&mut self, inode_id: InodeId, data: &[u8]) -> Result<(), ExtmapError> {
        if self.files.contains_key(&inode_id) {
            self.free_inode_extents(inode_id);
        }
        let total_len = data.len();
        if total_len == 0 {
            self.files.insert(
                inode_id,
                FileExtents {
                    extents: Vec::new(),
                    total_len: 0,
                },
            );
            self.counters.writes += 1;
            return Ok(());
        }
        let needed = blocks_needed(total_len);

        if needed == 1 {
            let hash = ph_content(data);
            if let Some(&existing) = self.dedup_index.get(&hash) {
                let matches = {
                    let slot = &self.blocks[existing as usize];
                    slot.state == BlockState::Live
                        && slot.len as usize == data.len()
                        && &slot.data[..data.len()] == data
                };
                if matches {
                    self.incref(existing);
                    let generation = self.bump_generation();
                    let mut extents = Vec::new();
                    extents.push(Extent {
                        logical_start: 0,
                        physical_start: existing,
                        block_count: 1,
                        content_root: hash,
                        generation,
                    });
                    self.files
                        .insert(inode_id, FileExtents { extents, total_len });
                    self.counters.writes += 1;
                    self.counters.dedup_hits += 1;
                    return Ok(());
                }
            }
            let start = match self.find_contiguous_free_run(1) {
                Some(s) => s,
                None => {
                    self.counters.out_of_space_rejections += 1;
                    return Err(ExtmapError::OutOfSpace);
                }
            };
            self.blocks[start as usize].state = BlockState::Reserved;
            self.write_block_content(start, data);
            self.dedup_index.insert(hash, start);
            let generation = self.bump_generation();
            let mut extents = Vec::new();
            extents.push(Extent {
                logical_start: 0,
                physical_start: start,
                block_count: 1,
                content_root: hash,
                generation,
            });
            self.files
                .insert(inode_id, FileExtents { extents, total_len });
            self.counters.writes += 1;
            return Ok(());
        }

        let total_free = self
            .blocks
            .iter()
            .filter(|b| b.state == BlockState::Free)
            .count() as u32;
        if total_free < needed {
            self.counters.out_of_space_rejections += 1;
            return Err(ExtmapError::OutOfSpace);
        }
        let mut extents: Vec<Extent> = Vec::new();
        let mut logical_cursor: u32 = 0;
        let mut remaining = needed;
        while remaining > 0 {
            let (start, len) = match self.find_contiguous_free_run(remaining) {
                Some(s) => (s, remaining),
                None => self
                    .find_largest_free_run()
                    .expect("total_free check above guarantees a run exists"),
            };
            for b in start..start + len {
                self.blocks[b as usize].state = BlockState::Reserved;
            }
            let generation = self.bump_generation();
            extents.push(Extent {
                logical_start: logical_cursor,
                physical_start: start,
                block_count: len,
                content_root: StateHash::default(),
                generation,
            });
            logical_cursor += len;
            remaining -= len;
        }
        let mut offset = 0usize;
        for extent in extents.iter_mut() {
            let mut leaves: Vec<StateHash> = Vec::new();
            for b_off in 0..extent.block_count {
                let idx = extent.physical_start + b_off;
                let end = core::cmp::min(offset + EXTMAP_BLOCK_SIZE, total_len);
                let chunk = &data[offset..end];
                self.write_block_content(idx, chunk);
                leaves.push(self.blocks[idx as usize].content_root);
                offset = end;
            }
            extent.content_root = ph_root(&leaves);
        }
        self.files
            .insert(inode_id, FileExtents { extents, total_len });
        self.counters.writes += 1;
        Ok(())
    }

    /// Reconstruct `inode_id`'s exact stored bytes. `NotFound` if
    /// `write_inode` was never called for this inode (or it was deleted).
    pub fn read_inode(&mut self, inode_id: InodeId) -> Result<Vec<u8>, ExtmapError> {
        let fe = self
            .files
            .get(&inode_id)
            .cloned()
            .ok_or(ExtmapError::NotFound)?;
        let out = self.read_extents(&fe.extents, fe.total_len);
        self.counters.reads += 1;
        Ok(out)
    }

    /// Free every block backing `inode_id`'s content and drop its record.
    /// A safe no-op (matching "recursive deletion" safety for e.g.
    /// directories or never-written files) when no record exists.
    pub fn delete_inode(&mut self, inode_id: InodeId) -> Result<(), ExtmapError> {
        self.free_inode_extents(inode_id);
        self.counters.deletes += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_pool_is_fully_free() {
        let em = Extmap::new(16);
        let s = em.stats();
        assert_eq!(s.blocks_total, 16);
        assert_eq!(s.blocks_free, 16);
        assert_eq!(s.blocks_live, 0);
    }

    #[test]
    fn alloc_extent_reserves_a_contiguous_run_and_is_skipped_by_the_next_alloc() {
        let mut em = Extmap::new(8);
        let e1 = em.alloc_extent(3).unwrap();
        assert_eq!(e1.physical_start, 0);
        assert_eq!(e1.block_count, 3);
        for b in 0..3 {
            assert_eq!(em.block_state(b), Some(BlockState::Reserved));
        }
        let e2 = em.alloc_extent(3).unwrap();
        assert_eq!(e2.physical_start, 3, "must not overlap e1's reserved run");
        assert_eq!(em.stats().blocks_free, 2);
    }

    /// Required proof: multi-block files round-trip exactly.
    #[test]
    fn write_extent_data_round_trips_multi_block_content_exactly() {
        let mut em = Extmap::new(8);
        let mut extent = em.alloc_extent(3).unwrap();
        let data: Vec<u8> = (0u32..(EXTMAP_BLOCK_SIZE as u32 * 2 + 100))
            .map(|i| (i % 251) as u8)
            .collect();
        em.write_extent_data(&mut extent, &data).unwrap();
        for b in 0..3 {
            assert_eq!(
                em.block_state(extent.physical_start + b),
                Some(BlockState::Live)
            );
        }
        let round_trip = em.read_extents(&[extent], data.len());
        assert_eq!(round_trip, data);
    }

    /// Required proof: multi-block files round-trip exactly, through the
    /// higher-level per-inode API `kernel::syscall`'s FS_WRITE_FILE/
    /// FS_READ_FILE handlers actually call.
    #[test]
    fn write_inode_read_inode_round_trip_across_multiple_blocks() {
        let mut em = Extmap::new(16);
        let data: Vec<u8> = (0u32..1400).map(|i| (i % 200) as u8).collect();
        em.write_inode(42, &data).unwrap();
        let back = em.read_inode(42).unwrap();
        assert_eq!(back, data);
        assert_eq!(em.stats().writes, 1);
        assert_eq!(em.stats().reads, 1);
    }

    #[test]
    fn write_inode_zero_length_stores_an_empty_file_without_allocating_any_block() {
        let mut em = Extmap::new(4);
        em.write_inode(1, &[]).unwrap();
        assert_eq!(em.read_inode(1).unwrap(), Vec::<u8>::new());
        assert_eq!(
            em.stats().blocks_free,
            4,
            "an empty file must not consume a block"
        );
    }

    #[test]
    fn write_inode_overwrite_frees_previous_blocks_before_allocating_new_ones() {
        let mut em = Extmap::new(8);
        em.write_inode(1, &[9u8; 1200]).unwrap(); // 3 blocks
        assert_eq!(em.stats().blocks_free, 5);
        em.write_inode(1, b"short").unwrap(); // 1 block, dedup-eligible
        assert_eq!(
            em.stats().blocks_free,
            7,
            "the 3 old blocks must be freed and only 1 new one claimed"
        );
        assert_eq!(em.read_inode(1).unwrap(), b"short");
    }

    /// Required proof: hash equality selects a dedup candidate, verified
    /// byte-for-byte before sharing.
    #[test]
    fn single_block_dedup_hits_share_the_same_physical_block_and_bump_refcount() {
        let mut em = Extmap::new(8);
        em.write_inode(1, b"hello").unwrap();
        em.write_inode(2, b"hello").unwrap();
        let s = em.stats();
        assert_eq!(s.dedup_hits, 1);
        assert_eq!(
            s.blocks_live, 1,
            "only one physical block should back both files"
        );
        assert_eq!(em.read_inode(1).unwrap(), b"hello");
        assert_eq!(em.read_inode(2).unwrap(), b"hello");
    }

    #[test]
    fn deleting_one_dedup_sharing_inode_keeps_the_shared_block_live_for_the_other() {
        let mut em = Extmap::new(8);
        em.write_inode(1, b"shared").unwrap();
        em.write_inode(2, b"shared").unwrap();
        em.delete_inode(1).unwrap();
        assert_eq!(
            em.stats().blocks_live,
            1,
            "block 2's reference keeps it live"
        );
        assert_eq!(em.read_inode(2).unwrap(), b"shared");
        em.delete_inode(2).unwrap();
        assert_eq!(
            em.stats().blocks_free,
            8,
            "the last reference must free the block"
        );
    }

    /// Required proof: split, coalesce, resize, free, and reuse preserve
    /// accounting.
    #[test]
    fn split_extent_preserves_total_blocks_and_content_without_touching_block_state() {
        let mut em = Extmap::new(8);
        let mut extent = em.alloc_extent(4).unwrap();
        let data: Vec<u8> = (0u32..(EXTMAP_BLOCK_SIZE as u32 * 4))
            .map(|i| i as u8)
            .collect();
        em.write_extent_data(&mut extent, &data).unwrap();
        let before = em.stats();

        let (left, right) = em.split_extent(&extent, extent.logical_start + 1).unwrap();
        assert_eq!(left.block_count + right.block_count, extent.block_count);
        assert_eq!(left.physical_start, extent.physical_start);
        assert_eq!(right.physical_start, extent.physical_start + 1);

        let after = em.stats();
        assert_eq!(
            before.blocks_live, after.blocks_live,
            "split must not change any block's state"
        );
        assert_eq!(before.blocks_free, after.blocks_free);

        let mut combined = em.read_extents(&[left], EXTMAP_BLOCK_SIZE);
        combined.extend(em.read_extents(&[right], EXTMAP_BLOCK_SIZE * 3));
        assert_eq!(combined, data);
    }

    #[test]
    fn coalesce_extents_rejects_non_adjacent_and_merges_adjacent_preserving_content() {
        let mut em = Extmap::new(8);
        let mut e1 = em.alloc_extent(2).unwrap();
        let mut e2 = em.alloc_extent(2).unwrap();
        em.write_extent_data(&mut e1, &[1u8; EXTMAP_BLOCK_SIZE * 2])
            .unwrap();
        em.write_extent_data(&mut e2, &[2u8; EXTMAP_BLOCK_SIZE * 2])
            .unwrap();

        // e1 and e2 are physically adjacent (0..2, 2..4) but e2's logical
        // range does not follow e1's own logical range (both start at 0) —
        // coalesce is about *this pair's* logical+physical contiguity, not
        // global uniqueness, so give e2 the logical offset that actually
        // makes it adjacent to e1 before coalescing.
        let e2_adjacent = Extent {
            logical_start: e1.logical_start + e1.block_count,
            ..e2
        };
        assert_eq!(
            em.coalesce_extents(&e1, &e2),
            Err(ExtmapError::NotAdjacent),
            "e2's logical_start (0) does not follow e1's logical range"
        );

        let merged = em.coalesce_extents(&e1, &e2_adjacent).unwrap();
        assert_eq!(merged.block_count, 4);
        assert_eq!(merged.physical_start, e1.physical_start);
        let expected: Vec<u8> = [
            vec![1u8; EXTMAP_BLOCK_SIZE * 2],
            vec![2u8; EXTMAP_BLOCK_SIZE * 2],
        ]
        .concat();
        assert_eq!(em.read_extents(&[merged], expected.len()), expected);
    }

    #[test]
    fn resize_extent_grows_into_free_space_and_shrinks_by_freeing_trailing_blocks() {
        let mut em = Extmap::new(8);
        let mut extent = em.alloc_extent(2).unwrap();
        em.write_extent_data(&mut extent, &[7u8; EXTMAP_BLOCK_SIZE * 2])
            .unwrap();
        assert_eq!(em.stats().blocks_free, 6);

        let grown = em.resize_extent(&extent, 4).unwrap();
        assert_eq!(grown.block_count, 4);
        assert_eq!(em.stats().blocks_free, 4);
        assert_eq!(em.stats().blocks_live, 4);

        let shrunk = em.resize_extent(&grown, 1).unwrap();
        assert_eq!(shrunk.block_count, 1);
        assert_eq!(
            em.stats().blocks_free,
            7,
            "3 freed blocks must be reclaimed"
        );
    }

    #[test]
    fn resize_extent_grow_fails_when_the_next_blocks_are_not_free() {
        let mut em = Extmap::new(8);
        let mut a = em.alloc_extent(2).unwrap();
        em.write_extent_data(&mut a, &[1u8; EXTMAP_BLOCK_SIZE * 2])
            .unwrap();
        let mut b = em.alloc_extent(2).unwrap(); // claims 2..4, blocking a's growth
        em.write_extent_data(&mut b, &[2u8; EXTMAP_BLOCK_SIZE * 2])
            .unwrap();
        assert_eq!(em.resize_extent(&a, 4), Err(ExtmapError::CannotGrow));
    }

    /// Required proof: reserved/damaged blocks never allocate.
    #[test]
    fn damaged_and_reserved_blocks_are_never_selected_by_allocation() {
        let mut em = Extmap::new(4);
        em.mark_damaged(1).unwrap();
        // Only blocks 0, 2, 3 remain Free; block 1 must never be chosen.
        let e1 = em.alloc_extent(1).unwrap();
        assert_eq!(e1.physical_start, 0);
        let e2 = em.alloc_extent(1).unwrap();
        assert_ne!(e2.physical_start, 1);
        let e3 = em.alloc_extent(1).unwrap();
        assert_ne!(e3.physical_start, 1);
        // No 2-block contiguous run exists any more (0 and {2,3} are split
        // by damaged block 1, and 0/2/3 are all now Reserved besides).
        assert_eq!(em.alloc_extent(1), Err(ExtmapError::OutOfSpace));
    }

    #[test]
    fn mark_damaged_rejects_blocks_currently_in_use() {
        let mut em = Extmap::new(4);
        em.write_inode(1, b"x").unwrap();
        let live_index = {
            let s = em.stats();
            assert_eq!(s.blocks_live, 1);
            0 // write_inode's first write always lands on block 0 in a fresh pool
        };
        assert_eq!(em.mark_damaged(live_index), Err(ExtmapError::InUse));
    }

    /// Required proof: `OutOfSpace` is rejected atomically — no partial
    /// allocation leaks out.
    #[test]
    fn out_of_space_is_rejected_without_partial_allocation() {
        let mut em = Extmap::new(4);
        let big = [0u8; EXTMAP_BLOCK_SIZE * 5];
        assert_eq!(em.write_inode(1, &big), Err(ExtmapError::OutOfSpace));
        assert_eq!(
            em.stats().blocks_free,
            4,
            "a failed write must not consume any block"
        );
        assert_eq!(em.stats().out_of_space_rejections, 1);
        assert!(!em.has_inode(1));
    }

    #[test]
    fn delete_inode_on_unknown_inode_is_a_safe_noop() {
        let mut em = Extmap::new(4);
        em.delete_inode(999).unwrap();
        assert_eq!(em.stats().blocks_free, 4);
    }

    #[test]
    fn read_inode_on_unknown_inode_is_not_found() {
        let mut em = Extmap::new(4);
        assert_eq!(em.read_inode(999), Err(ExtmapError::NotFound));
    }

    /// Required proof: no overlapping ownership or reachable-free block,
    /// across a realistic sequence of writes, dedup, resizes, and deletes.
    /// Per the math contract's own non-overlap rule ("unless explicit
    /// verified sharing is recorded"), a block dedup legitimately shares is
    /// the *only* allowed exception — verified here by requiring every
    /// block's `refcount` to equal exactly how many live extents reference
    /// it, not merely "at least one."
    #[test]
    fn no_overlapping_ownership_or_reachable_free_block_after_a_mixed_sequence() {
        let mut em = Extmap::new(32);
        em.write_inode(1, &[1u8; EXTMAP_BLOCK_SIZE * 3]).unwrap();
        em.write_inode(2, b"dup").unwrap();
        em.write_inode(3, b"dup").unwrap(); // dedups against inode 2's block
        em.write_inode(4, &[4u8; EXTMAP_BLOCK_SIZE * 2]).unwrap();
        em.delete_inode(1).unwrap();
        em.write_inode(5, &[5u8; EXTMAP_BLOCK_SIZE * 4]).unwrap();

        let mut reference_counts: HashMap<u32, u32> = HashMap::new();
        for fe in em.files.values() {
            for extent in &fe.extents {
                for b in extent.physical_start..extent.physical_start + extent.block_count {
                    *reference_counts.entry(b).or_insert(0) += 1;
                }
            }
        }
        for (&block, &refs) in reference_counts.iter() {
            assert_eq!(
                em.blocks[block as usize].refcount, refs,
                "block {block}'s refcount must equal exactly how many live extents reference it"
            );
            assert_ne!(
                em.block_state(block),
                Some(BlockState::Free),
                "block {block} is referenced by a live extent but reports Free"
            );
        }
        for (i, slot) in em.blocks.iter().enumerate() {
            if slot.state == BlockState::Free {
                assert!(
                    !reference_counts.contains_key(&(i as u32)),
                    "block {i} is Free but still referenced by a live extent"
                );
            }
        }
    }
}
