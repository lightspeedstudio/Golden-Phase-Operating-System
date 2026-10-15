//! Physical memory manager (`feature = "pmm"`).
//!
//! Real buddy-allocated frame manager over a caller-supplied `(base, frame_count)`
//! interval, plus a generation-stamped slab allocator for small objects built on
//! top of frames. This is additive: it does not replace [`crate::kernel::memory::Memory`]
//! (the logical phase-tag page model), which remains the authorization/state
//! layer above physical allocation, per
//! `fpga/zynq/evidence/KERNEL_MATH_CONTRACT_2026-08-12.md` Part V.1.
//!
//! ## What this reuses / does not reuse (per the math contract, §0.7)
//! - Does **not** use [`crate::core::gphase_core::PhaseState`] for frame status.
//!   `Memory`'s `PhaseState` already carries defined page-lifecycle semantics
//!   (unmapped/user/kernel/locked); frames need a *different* partition
//!   (Free/Allocated/Reserved/Quarantined), so [`FrameState`] is its own enum,
//!   per math-contract §1's overloading rule.
//! - May anchor a compact frame-map digest with PH-R
//!   ([`crate::kernel::crypto::phase_hash::ph_root`]) at explicit audit points
//!   only ([`PhysicalMemoryManager::frame_map_digest`]); never hashed per
//!   allocation.
//! - Consumes no G64 operation (none exists to consume — math contract §0.4).
//!
//! ## Non-goals
//! No MMU/page-table code. `alloc` returns a physical frame address in the
//! caller-supplied interval. That is not an MMU-off claim and not a global
//! phys==virt identity. Gold Utility runs the Cortex-A9 MMU
//! (`mmu-app-isolation`); kernel DDR is identity-mapped there.

use crate::kernel::crypto::phase_hash::{ph_content, ph_root};
use crate::kernel::hash::StateHash;
use crate::platform::{HashMap, Vec};

/// 4 KiB frames.
pub const PAGE_SHIFT: u32 = 12;
pub const PAGE_SIZE: usize = 1 << PAGE_SHIFT;

/// Allocation-state partition. Disjoint by construction: a frame's `state[i]`
/// is exactly one of these four at any time, matching the math contract's
/// `F ⊔ A ⊔ R ⊔ Q = {0..N-1}` partition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameState {
    Free,
    Allocated,
    Reserved,
    Quarantined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PmmError {
    /// Request exceeds the largest available order.
    OutOfMemory,
    /// Address/order does not correspond to any live allocation.
    OutOfRange,
    /// `free`/`quarantine` called on an address that is not currently allocated
    /// (already free, never allocated, or a double free).
    DoubleFree,
    /// `quarantine` called with a page count that does not match the live
    /// allocation's actual order.
    OrderMismatch,
    /// Slab handle's generation does not match the frame's current generation
    /// (stale handle after the backing frame was retired and reused).
    StaleGeneration,
    /// Slab handle references a frame/slot that is not part of any live region.
    InvalidHandle,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PmmStats {
    pub allocations: u64,
    pub frees: u64,
    pub double_free_rejections: u64,
    pub exhaustions: u64,
    pub quarantines: u64,
}

fn order_for_pages(pages: u32) -> u32 {
    if pages <= 1 {
        return 0;
    }
    32 - (pages - 1).leading_zeros()
}

fn state_tag(s: FrameState) -> u8 {
    match s {
        FrameState::Free => 0,
        FrameState::Allocated => 1,
        FrameState::Reserved => 2,
        FrameState::Quarantined => 3,
    }
}

/// Real physical-frame buddy allocator over `[base, base + frame_count * PAGE_SIZE)`.
pub struct PhysicalMemoryManager {
    base: usize,
    frame_count: u32,
    max_order: u32,
    state: Vec<FrameState>,
    /// `free_lists[o]` holds the start-frame index of every free block of
    /// order `o` (size `2^o` frames), each aligned to `2^o`.
    free_lists: Vec<Vec<u32>>,
    /// Order of each currently-live allocation, keyed by its start frame.
    allocated_orders: HashMap<u32, u32>,
    stats: PmmStats,
}

impl PhysicalMemoryManager {
    /// `frame_count` must be `>= 1`. `base` need not be page-aligned by the
    /// type system, but callers must pass a page-aligned base for `alloc`'s
    /// returned addresses to be page-aligned (true for every real caller in
    /// this repository: the Zynq DDR region and the host simulation region
    /// below are both page-aligned constants).
    pub fn new(base: usize, frame_count: u32) -> Self {
        let frame_count = frame_count.max(1);
        let max_order = 32 - (frame_count - 1).leading_zeros().min(31);
        let max_order = max_order.max(0);
        let mut mgr = Self {
            base,
            frame_count,
            max_order,
            state: alloc_vec(FrameState::Free, frame_count as usize),
            free_lists: (0..=max_order).map(|_| Vec::new()).collect(),
            allocated_orders: HashMap::new(),
            stats: PmmStats::default(),
        };
        mgr.build_initial_free_lists();
        mgr
    }

    fn build_initial_free_lists(&mut self) {
        let mut idx = 0u32;
        while idx < self.frame_count {
            let align_order = if idx == 0 {
                self.max_order
            } else {
                idx.trailing_zeros().min(self.max_order)
            };
            let mut order = align_order;
            while order > 0 && idx + (1u32 << order) > self.frame_count {
                order -= 1;
            }
            self.free_lists[order as usize].push(idx);
            idx += 1u32 << order;
        }
    }

    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Inclusive-base, exclusive-end byte window this manager covers.
    pub fn region(&self) -> (usize, usize) {
        let end = self
            .base
            .saturating_add((self.frame_count as usize).saturating_mul(PAGE_SIZE));
        (self.base, end)
    }

    pub fn max_order(&self) -> u32 {
        self.max_order
    }

    pub fn stats(&self) -> PmmStats {
        self.stats
    }

    pub fn state_of(&self, frame: u32) -> Option<FrameState> {
        self.state.get(frame as usize).copied()
    }

    fn locate_free_block_containing(&self, frame: u32) -> Option<(u32, u32)> {
        for order in (0..=self.max_order).rev() {
            let size = 1u32 << order;
            for &start in &self.free_lists[order as usize] {
                if frame >= start && frame < start + size {
                    return Some((start, order));
                }
            }
        }
        None
    }

    fn remove_from_free_list(&mut self, order: u32, start: u32) -> bool {
        let list = &mut self.free_lists[order as usize];
        if let Some(pos) = list.iter().position(|&s| s == start) {
            list.swap_remove(pos);
            true
        } else {
            false
        }
    }

    fn split_block(&mut self, start: u32, order: u32) {
        let half = 1u32 << (order - 1);
        self.free_lists[(order - 1) as usize].push(start);
        self.free_lists[(order - 1) as usize].push(start + half);
    }

    /// Carve out `[start, start+count)` as permanently unavailable to
    /// `alloc`/slab. Real callers use this for anything a subsystem must
    /// exclude from the pool (e.g. a self-test's own control range) — the
    /// `base`/`frame_count` interval a caller passes to `new` should already
    /// exclude the kernel image, stacks, and `boot::heap`'s region, so this is
    /// not required for baseline safety, only for additional carve-outs.
    pub fn reserve(&mut self, start: u32, count: u32) -> Result<(), PmmError> {
        if count == 0 {
            return Ok(());
        }
        if start
            .checked_add(count)
            .is_none_or(|end| end > self.frame_count)
        {
            return Err(PmmError::OutOfRange);
        }
        for frame in start..start + count {
            if self.state[frame as usize] == FrameState::Reserved {
                continue;
            }
            let (block_start, block_order) = self
                .locate_free_block_containing(frame)
                .ok_or(PmmError::OutOfRange)?;
            self.remove_from_free_list(block_order, block_start);
            let mut cur_start = block_start;
            let mut cur_order = block_order;
            while cur_order > 0 {
                self.split_block(cur_start, cur_order);
                let half = 1u32 << (cur_order - 1);
                let chosen_start = if frame < cur_start + half {
                    cur_start
                } else {
                    cur_start + half
                };
                self.remove_from_free_list(cur_order - 1, chosen_start);
                cur_start = chosen_start;
                cur_order -= 1;
            }
            self.state[frame as usize] = FrameState::Reserved;
        }
        Ok(())
    }

    fn alloc_order(&mut self, order: u32) -> Option<u32> {
        if order > self.max_order {
            return None;
        }
        if let Some(start) = self.free_lists[order as usize].pop() {
            return Some(start);
        }
        let upper = self.alloc_order(order + 1)?;
        let half = 1u32 << order;
        self.free_lists[order as usize].push(upper + half);
        Some(upper)
    }

    /// Allocate `pages` contiguous, naturally-aligned frames. Returns the
    /// physical start address of the block.
    pub fn alloc(&mut self, pages: u32) -> Option<usize> {
        if pages == 0 {
            return None;
        }
        let order = order_for_pages(pages);
        let Some(start) = self.alloc_order(order) else {
            self.stats.exhaustions += 1;
            return None;
        };
        for f in start..start + (1u32 << order) {
            self.state[f as usize] = FrameState::Allocated;
        }
        self.allocated_orders.insert(start, order);
        self.stats.allocations += 1;
        Some(self.base + (start as usize) * PAGE_SIZE)
    }

    fn addr_to_start(&self, addr: usize) -> Option<u32> {
        let offset = addr.checked_sub(self.base)?;
        if offset % PAGE_SIZE != 0 {
            return None;
        }
        let start = offset / PAGE_SIZE;
        if start >= self.frame_count as usize {
            return None;
        }
        Some(start as u32)
    }

    fn merge_and_release(&mut self, mut start: u32, mut order: u32) {
        while order < self.max_order {
            let buddy = start ^ (1u32 << order);
            if buddy + (1u32 << order) > self.frame_count {
                break;
            }
            if !self.remove_from_free_list(order, buddy) {
                break;
            }
            start = start.min(buddy);
            order += 1;
        }
        self.free_lists[order as usize].push(start);
    }

    /// Free a block previously returned by `alloc`. Rejects double free /
    /// unknown addresses without panicking.
    pub fn free(&mut self, addr: usize) -> Result<(), PmmError> {
        let start = self.addr_to_start(addr).ok_or(PmmError::OutOfRange)?;
        let Some(order) = self.allocated_orders.remove(&start) else {
            self.stats.double_free_rejections += 1;
            return Err(PmmError::DoubleFree);
        };
        for f in start..start + (1u32 << order) {
            self.state[f as usize] = FrameState::Free;
        }
        self.merge_and_release(start, order);
        self.stats.frees += 1;
        Ok(())
    }

    /// Move a live allocation into quarantine instead of freeing it.
    /// Quarantine is absorbing: quarantined frames are removed from
    /// `allocated_orders` and never inserted into any free list again by any
    /// code path in this module (no un-quarantine API exists — per the math
    /// contract §0, quarantine stays absorbing unless a hardware
    /// revalidation procedure is separately defined, and none is today).
    pub fn quarantine(&mut self, addr: usize, pages: u32) -> Result<(), PmmError> {
        let start = self.addr_to_start(addr).ok_or(PmmError::OutOfRange)?;
        let order = *self
            .allocated_orders
            .get(&start)
            .ok_or(PmmError::DoubleFree)?;
        if (1u32 << order) != pages {
            return Err(PmmError::OrderMismatch);
        }
        self.allocated_orders.remove(&start);
        for f in start..start + pages {
            self.state[f as usize] = FrameState::Quarantined;
        }
        self.stats.quarantines += 1;
        Ok(())
    }

    /// Cold-path only: compact run-length-encoded PH-R digest over the whole
    /// frame-state array. Never called from `alloc`/`free`/`quarantine`.
    pub fn frame_map_digest(&self) -> StateHash {
        let mut leaves: Vec<StateHash> = Vec::new();
        let mut i = 0usize;
        while i < self.state.len() {
            let tag = state_tag(self.state[i]);
            let mut run: u32 = 1;
            while i + (run as usize) < self.state.len()
                && state_tag(self.state[i + run as usize]) == tag
            {
                run += 1;
            }
            let mut leaf_bytes = [0u8; 5];
            leaf_bytes[0] = tag;
            leaf_bytes[1..5].copy_from_slice(&run.to_le_bytes());
            leaves.push(ph_content(&leaf_bytes));
            i += run as usize;
        }
        ph_root(&leaves)
    }
}

fn alloc_vec(value: FrameState, count: usize) -> Vec<FrameState> {
    let mut v = Vec::with_capacity(count);
    for _ in 0..count {
        v.push(value);
    }
    v
}

// ---------------------------------------------------------------------------
// Slab allocator: small-object allocation on top of `PhysicalMemoryManager`,
// with generation-stamped handles per the math contract's `h = (i, g)` handle
// pattern (§6). This is `pmm`'s own internal, self-contained precedent — it
// does not depend on the `captable` subsystem, which does not exist yet.
// ---------------------------------------------------------------------------

/// `(frame, slot, generation)` handle. Valid only while `generation` matches
/// the live generation counter for `frame` at the time of `free_small`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlabHandle {
    pub frame: u32,
    pub slot: u16,
    pub generation: u32,
}

/// `c(n) = 2^ceil(log2(max(n, align)))`.
pub fn size_class(n: usize, align: usize) -> usize {
    core::cmp::max(n, align).max(1).next_power_of_two()
}

struct SlabRegion {
    frame: u32,
    class: usize,
    generation: u32,
    free_slots: Vec<u16>,
    live: u16,
}

pub struct SlabAllocator {
    regions: Vec<SlabRegion>,
    /// index into `regions` for a given `(frame)` while that frame currently
    /// backs a live slab region.
    region_of_frame: HashMap<u32, usize>,
    by_class: HashMap<usize, Vec<usize>>,
    /// Generation counter per frame index, persists across retirement so a
    /// stale handle referencing a retired-and-reused frame is rejected.
    frame_generation: HashMap<u32, u32>,
}

impl Default for SlabAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl SlabAllocator {
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
            region_of_frame: HashMap::new(),
            by_class: HashMap::new(),
            frame_generation: HashMap::new(),
        }
    }

    pub fn alloc_small(
        &mut self,
        pmm: &mut PhysicalMemoryManager,
        size: usize,
        align: usize,
    ) -> Option<SlabHandle> {
        let class = size_class(size, align).max(8).min(PAGE_SIZE);
        if let Some(indices) = self.by_class.get(&class) {
            for &idx in indices {
                if let Some(slot) = self.regions[idx].free_slots.pop() {
                    self.regions[idx].live += 1;
                    let frame = self.regions[idx].frame;
                    let generation = self.regions[idx].generation;
                    return Some(SlabHandle {
                        frame,
                        slot,
                        generation,
                    });
                }
            }
        }
        let addr = pmm.alloc(1)?;
        let frame = ((addr - pmm.base) / PAGE_SIZE) as u32;
        let generation = *self.frame_generation.entry(frame).or_insert(0);
        let slots_per_region = (PAGE_SIZE / class) as u16;
        let mut free_slots: Vec<u16> = (1..slots_per_region).collect();
        let taken_slot = 0u16;
        let region = SlabRegion {
            frame,
            class,
            generation,
            free_slots: {
                free_slots.reverse();
                free_slots
            },
            live: 1,
        };
        let idx = self.regions.len();
        self.regions.push(region);
        self.region_of_frame.insert(frame, idx);
        self.by_class.entry(class).or_default().push(idx);
        Some(SlabHandle {
            frame,
            slot: taken_slot,
            generation,
        })
    }

    pub fn free_small(
        &mut self,
        pmm: &mut PhysicalMemoryManager,
        handle: SlabHandle,
    ) -> Result<(), PmmError> {
        let &idx = self
            .region_of_frame
            .get(&handle.frame)
            .ok_or(PmmError::InvalidHandle)?;
        if self.regions[idx].generation != handle.generation {
            return Err(PmmError::StaleGeneration);
        }
        let slots_per_region = (PAGE_SIZE / self.regions[idx].class) as u16;
        if handle.slot >= slots_per_region {
            return Err(PmmError::InvalidHandle);
        }
        if self.regions[idx].free_slots.contains(&handle.slot) {
            return Err(PmmError::DoubleFree);
        }
        self.regions[idx].free_slots.push(handle.slot);
        self.regions[idx].live -= 1;
        if self.regions[idx].live == 0 {
            let frame = self.regions[idx].frame;
            let class = self.regions[idx].class;
            self.region_of_frame.remove(&frame);
            if let Some(list) = self.by_class.get_mut(&class) {
                if let Some(pos) = list.iter().position(|&i| i == idx) {
                    list.swap_remove(pos);
                }
            }
            let region_addr = pmm.base + (frame as usize) * PAGE_SIZE;
            let _ = pmm.free(region_addr);
            *self.frame_generation.entry(frame).or_insert(0) += 1;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Zynq DDR usable-region derivation (bare-metal only).
// ---------------------------------------------------------------------------

/// Usable DDR interval for the real PZ7020 board: everything from the end of
/// the existing linker-reserved kernel image/stacks/`boot::heap` region
/// (`__heap_end`, the same symbol `src/boot/heap.rs` already uses) up to the
/// documented end of board DDR (`memory_map::DDR_HIGH`). This never overlaps
/// the kernel image, stacks, or `linked_list_allocator`'s heap, because all
/// three are placed by the linker strictly below `__heap_end`
/// (`src/arch/armv7/linker.ld`).
#[cfg(all(target_arch = "arm", feature = "zynq-a9"))]
pub fn zynq7000_usable_region() -> (usize, u32) {
    extern "C" {
        static __heap_end: u8;
    }
    let heap_end = unsafe { core::ptr::addr_of!(__heap_end) as usize };
    let ddr_end = crate::board::zynq7000::memory_map::DDR_HIGH as usize + 1;
    let base = heap_end.next_multiple_of(PAGE_SIZE);
    let usable_bytes = ddr_end.saturating_sub(base);
    let frame_count = (usable_bytes / PAGE_SIZE) as u32;
    (base, frame_count.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn splitmix64(mut x: u64) -> u64 {
        x = x.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    #[test]
    fn frame_count_and_max_order_are_consistent() {
        let mgr = PhysicalMemoryManager::new(0x1000_0000, 1024);
        assert_eq!(mgr.frame_count(), 1024);
        assert_eq!(mgr.max_order(), 10); // 2^10 == 1024
    }

    #[test]
    fn single_page_alloc_free_round_trip() {
        let mut mgr = PhysicalMemoryManager::new(0x1000_0000, 16);
        let addr = mgr.alloc(1).expect("alloc");
        assert_eq!(addr % PAGE_SIZE, 0);
        assert_eq!(mgr.stats().allocations, 1);
        mgr.free(addr).expect("free");
        assert_eq!(mgr.stats().frees, 1);
        // Freeing again must fail: this is the real double-free check, not a
        // manufactured pass — the second call executes the same `free` path.
        assert_eq!(mgr.free(addr), Err(PmmError::DoubleFree));
        assert_eq!(mgr.stats().double_free_rejections, 1);
    }

    #[test]
    fn no_two_live_allocations_overlap() {
        // Deterministic pseudo-random alloc/free sequence (ported splitmix64,
        // matching the repository's existing phase_cache.rs precedent for
        // reproducible non-cryptographic sequences).
        let mut mgr = PhysicalMemoryManager::new(0x2000_0000, 256);
        let mut live: Vec<(usize, u32)> = Vec::new();
        let mut seed = 0xC0FFEEu64;
        for _ in 0..2000 {
            seed = splitmix64(seed);
            if seed % 3 != 0 || live.len() > 40 {
                if let Some(idx) = live.pop() {
                    mgr.free(idx.0).expect("free tracked allocation");
                }
                continue;
            }
            let pages = 1 + (seed >> 8) % 4;
            if let Some(addr) = mgr.alloc(pages as u32) {
                let size = (1u32 << order_for_pages(pages as u32)) as usize * PAGE_SIZE;
                for &(other_addr, other_order) in &live {
                    let other_size = (1u32 << other_order) as usize * PAGE_SIZE;
                    let overlap = addr < other_addr + other_size && other_addr < addr + size;
                    assert!(
                        !overlap,
                        "overlap between [{addr:#x},{:#x}) and [{other_addr:#x},{:#x})",
                        addr + size,
                        other_addr + other_size
                    );
                }
                live.push((addr, order_for_pages(pages as u32)));
            }
        }
        for (addr, _) in live {
            mgr.free(addr).expect("final free");
        }
    }

    #[test]
    fn alignment_holds_for_every_order() {
        let mut mgr = PhysicalMemoryManager::new(0x3000_0000, 64);
        for pages in [1u32, 2, 3, 4, 5, 8, 16] {
            if let Some(addr) = mgr.alloc(pages) {
                let order = order_for_pages(pages);
                let block_bytes = (1u32 << order) as usize * PAGE_SIZE;
                assert_eq!(
                    (addr - mgr.base) % block_bytes,
                    0,
                    "order-{order} block at {addr:#x} not aligned to {block_bytes:#x}"
                );
            }
        }
    }

    #[test]
    fn reserved_frames_are_never_returned() {
        let mut mgr = PhysicalMemoryManager::new(0x4000_0000, 32);
        mgr.reserve(4, 4).expect("reserve");
        for f in 4..8 {
            assert_eq!(mgr.state_of(f), Some(FrameState::Reserved));
        }
        let mut seen = Vec::new();
        while let Some(addr) = mgr.alloc(1) {
            let frame = ((addr - mgr.base) / PAGE_SIZE) as u32;
            assert!(
                !(4..8).contains(&frame),
                "allocator returned reserved frame {frame}"
            );
            seen.push(addr);
        }
        assert_eq!(
            seen.len(),
            28,
            "expected exactly the 28 non-reserved frames"
        );
        for addr in seen {
            mgr.free(addr).unwrap();
        }
    }

    #[test]
    fn quarantined_frames_are_never_reused_even_under_exhaustion() {
        let mut mgr = PhysicalMemoryManager::new(0x5000_0000, 8);
        let addr = mgr.alloc(1).unwrap();
        let quarantined_frame = (addr - mgr.base) / PAGE_SIZE;
        mgr.quarantine(addr, 1).expect("quarantine");
        assert_eq!(
            mgr.state_of(quarantined_frame as u32),
            Some(FrameState::Quarantined)
        );
        // Exhaust every remaining frame; the quarantined one must never appear.
        let mut got = Vec::new();
        while let Some(a) = mgr.alloc(1) {
            got.push(a);
        }
        assert_eq!(
            got.len(),
            7,
            "one frame is quarantined, 7 must remain allocatable"
        );
        for a in &got {
            let f = (a - mgr.base) / PAGE_SIZE;
            assert_ne!(f, quarantined_frame, "quarantined frame was reallocated");
        }
        assert_eq!(mgr.alloc(1), None, "pool must be fully exhausted now");
        // The `while let` loop above already recorded one exhaustion (its
        // final, failing call), and this explicit re-check records a second.
        assert_eq!(mgr.stats().exhaustions, 2);
        for a in got {
            mgr.free(a).unwrap();
        }
    }

    #[test]
    fn exhaustion_preserves_invariants_and_full_reallocation_succeeds_after_freeing() {
        let mut mgr = PhysicalMemoryManager::new(0x6000_0000, 8);
        let mut all = Vec::new();
        while let Some(a) = mgr.alloc(1) {
            all.push(a);
        }
        assert_eq!(all.len(), 8);
        assert_eq!(mgr.alloc(1), None);
        for a in all {
            mgr.free(a).unwrap();
        }
        // Buddy merge must have fully recombined: a single order-3 (8-page)
        // allocation must now succeed, proving no invariant/leak survived.
        let whole = mgr.alloc(8).expect("full pool must be allocatable again");
        assert_eq!(whole, mgr.base);
        mgr.free(whole).unwrap();
    }

    #[test]
    fn frame_map_digest_is_deterministic_and_state_sensitive() {
        let mut mgr = PhysicalMemoryManager::new(0x7000_0000, 16);
        let d0 = mgr.frame_map_digest();
        assert_eq!(d0, mgr.frame_map_digest());
        let addr = mgr.alloc(1).unwrap();
        let d1 = mgr.frame_map_digest();
        assert_ne!(d0, d1, "digest must change once a frame is allocated");
        mgr.free(addr).unwrap();
        let d2 = mgr.frame_map_digest();
        assert_eq!(
            d0, d2,
            "digest must return to the original value after full free"
        );
    }

    #[test]
    fn slab_alloc_free_round_trip_and_stale_generation_rejected() {
        let mut mgr = PhysicalMemoryManager::new(0x8000_0000, 4);
        let mut slabs = SlabAllocator::new();
        let h1 = slabs.alloc_small(&mut mgr, 32, 8).expect("slab alloc");
        let h2 = slabs.alloc_small(&mut mgr, 32, 8).expect("slab alloc");
        assert_eq!(h1.frame, h2.frame, "same class should share one region");
        assert_ne!(h1.slot, h2.slot);
        slabs.free_small(&mut mgr, h1).expect("free h1");
        slabs
            .free_small(&mut mgr, h2)
            .expect("free h2 retires region");
        // Region retired -> frame returned to pmm and its generation bumped.
        // h1 is now stale even though its bit pattern looks structurally valid.
        let h3 = slabs
            .alloc_small(&mut mgr, 32, 8)
            .expect("re-alloc after retire");
        assert_eq!(h3.frame, h1.frame, "same frame index gets reused");
        assert_ne!(
            h3.generation, h1.generation,
            "generation must have advanced"
        );
        assert_eq!(
            slabs.free_small(&mut mgr, h1),
            Err(PmmError::StaleGeneration)
        );
        slabs.free_small(&mut mgr, h3).unwrap();
    }

    #[test]
    fn slab_double_free_is_rejected() {
        let mut mgr = PhysicalMemoryManager::new(0x9000_0000, 4);
        let mut slabs = SlabAllocator::new();
        let h = slabs.alloc_small(&mut mgr, 16, 8).expect("alloc");
        slabs.free_small(&mut mgr, h).expect("first free");
        assert_eq!(slabs.free_small(&mut mgr, h), Err(PmmError::InvalidHandle));
    }
}
