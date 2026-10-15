//! PhaseCache — a real, `no_std`, two-clock trusted cache over `PhaseFs`
//! path resolution.
//!
//! This ports a host-only research prototype (a synthetic C++ benchmark
//! modeling a "two-clock trusted symmetry cache") onto real Golden Phase OS
//! primitives. Every synthetic stand-in in that prototype is replaced here
//! by an existing, already-tested kernel construct — nothing here is a
//! placeholder:
//!
//! | prototype concept | real replacement used here |
//! |---|---|
//! | generic integer `object` key | `InodeId` resolved from a `PhaseFs` path |
//! | 8 synthetic `state`s | the cached inode's real `PhaseState` tag |
//! | synthetic reflection `r(s) = 7-s` | [`crate::core::gphase_core::reflect`] via [`ph_reflection_orbit`] (2 real orbit classes: `{Phi0,Phi3}`, `{Phi1,Phi2}`) |
//! | lower clock (ticks per access) | the live `StateClock.chronos_tick`, passed in by the caller |
//! | upper clock (ticks per state transition) | a self-tracked count of orbit-class transitions across successive accesses |
//! | `AcceptedTimeline` (generation/root, rollback) | the real [`StatechainLedger`]'s HMAC-SHA256 integrity [`SignedCheckpoint`](crate::kernel::statechain_checkpoint::SignedCheckpoint) chain (not Ed25519, not PSC1) |
//! | `payload_digest_ok` / `corrupt_payload` | explicit [`PhaseCacheController::invalidate_path`] / [`invalidate_inode`](PhaseCacheController::invalidate_inode) calls wired into the mutating `FS_*` syscalls |
//! | floating-point retention score | pure integer fixed-point (Q16, [`SCALE`]) |
//! | `splitmix64` victim sampler | ported byte-for-byte (already pure `u64` integer math) |
//!
//! ## Scope
//!
//! This implements the prototype's *two-clock symmetry* retention blend
//! (lower-clock recency + upper-clock recency + frequency, all orbit-aware).
//! It does not port the prototype's separate Markov transition-matrix
//! forward-forecast layer (`confidence`/`predictive_information`) — that is
//! a materially larger, separately-falsifiable statistical model that was
//! not part of what was scoped for this pass.
//!
//! ## Trust gate
//!
//! An entry inserted before any checkpoint has ever been sealed is bound to
//! the ledger's stable genesis [`StatechainLedger::equation_root`] (which
//! never changes after boot). An entry inserted after at least one
//! checkpoint exists is bound to that checkpoint's `(sequence, kernel_root)`.
//! A cached entry is only ever served if its bound root still matches the
//! ledger's live view — a rolled-back or corrupted checkpoint is rejected,
//! not served, exactly like the prototype's trust gate.

use crate::core::gphase_core::PhaseState;
use crate::fs::InodeId;
use crate::kernel::crypto::phase_hash::ph_reflection_orbit;
use crate::kernel::hash::StateHash;
use crate::kernel::statechain::StatechainLedger;
use crate::platform::{BTreeMap, String, ToString, Vec};

/// Fixed-point scale for the retention score (Q16: values are fractions of
/// `SCALE`, so `SCALE` itself represents `1.0`).
const SCALE: u32 = 1 << 16;
/// Weight on lower-clock (access-tick) recency — `0.45 * SCALE`.
const W_LOWER_RECENCY: u32 = 29_491;
/// Weight on upper-clock (orbit-transition-tick) recency — `0.20 * SCALE`.
const W_UPPER_RECENCY: u32 = 13_107;
/// Weight on log-scaled access frequency — `0.35 * SCALE`. The three weights
/// sum to exactly `SCALE`.
const W_FREQUENCY: u32 = 22_938;
/// Floor on the lower-clock recency time-constant, in access ticks.
const MIN_LOWER_TAU: u64 = 32;
/// Upper-clock recency time-constant, in orbit-transition ticks.
const UPPER_TAU: u64 = 3;
/// `bit_width(frequency) / MAX_FREQUENCY_BITS` saturates the frequency term
/// at `SCALE` once an entry has been hit `2^MAX_FREQUENCY_BITS` times.
const MAX_FREQUENCY_BITS: u32 = 12;
/// Initial prior for the orbit-dwell EMA before any real transitions have
/// been observed; converges to the live value after a handful of accesses.
const INITIAL_DURATION_EMA: u64 = 480;
/// Bounded victim-candidate sample size. Real cache capacities here are far
/// smaller than the prototype's 2,048-8,192 entry benchmark, so a small
/// bounded sample is proportionate.
const VICTIM_SAMPLE_COUNT: usize = 8;

/// One cached `path -> InodeId` resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PhaseCacheEntry {
    path: String,
    inode: InodeId,
    /// Controller `lower_tick` (live Chronos tick) at last access.
    lower_tick: u64,
    /// Controller `upper_tick` (self-tracked orbit-transition count) at last access.
    upper_tick: u64,
    /// The checkpoint generation this entry is bound to, or `None` for the
    /// pre-checkpoint genesis generation (bound to `equation_root` instead).
    checkpoint_sequence: Option<u64>,
    checkpoint_root: StateHash,
    /// Cleared explicitly by `invalidate_path`/`invalidate_inode` when the
    /// underlying inode is mutated — never re-derived by re-hashing content
    /// on every access, which would defeat the point of caching.
    valid: bool,
    frequency: u32,
    /// Reflection-orbit class of the cached inode's `PhaseState`: `0` for
    /// `{Phi0, Phi3}`, `1` for `{Phi1, Phi2}` (see [`ph_reflection_orbit`]).
    orbit: u8,
}

/// Hit/miss/rejection counters, for diagnostics and hardware proof.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PhaseCacheStats {
    pub hits: u64,
    pub misses: u64,
    /// Misses caused specifically by a trust-gate rejection (entry present
    /// but bound to a generation/root the ledger no longer accepts).
    pub trust_rejections: u64,
}

/// Trusted, two-clock, orbit-aware cache over `PhaseFs` path resolution.
#[derive(Debug)]
pub struct PhaseCacheController {
    capacity: usize,
    slots: Vec<Option<PhaseCacheEntry>>,
    index: BTreeMap<String, usize>,
    lower_tick: u64,
    upper_tick: u64,
    current_orbit: Option<u8>,
    duration_ema: u64,
    current_duration: u64,
    counters: PhaseCacheStats,
}

impl PhaseCacheController {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            slots: Vec::new(),
            index: BTreeMap::new(),
            lower_tick: 0,
            upper_tick: 0,
            current_orbit: None,
            duration_ema: INITIAL_DURATION_EMA,
            current_duration: 1,
            counters: PhaseCacheStats::default(),
        }
    }

    /// Look up `path`. Returns `Some(inode)` only if an entry is present
    /// *and* trusted against `ledger`'s current checkpoint chain; a stale or
    /// trust-rejected entry is evicted immediately and counted as both a
    /// miss and a trust rejection.
    pub fn get(
        &mut self,
        path: &str,
        ledger: &StatechainLedger,
        chronos_tick: u64,
    ) -> Option<InodeId> {
        if self.capacity == 0 {
            self.counters.misses += 1;
            return None;
        }
        let Some(&index) = self.index.get(path) else {
            self.counters.misses += 1;
            return None;
        };
        let trusted = self.slots[index]
            .as_ref()
            .map(|entry| Self::is_trusted(entry, ledger))
            .unwrap_or(false);
        if !trusted {
            self.index.remove(path);
            self.slots[index] = None;
            self.counters.trust_rejections += 1;
            self.counters.misses += 1;
            return None;
        }
        let orbit = self.slots[index].as_ref().unwrap().orbit;
        self.observe_orbit(orbit);
        self.lower_tick = chronos_tick;
        let entry = self.slots[index].as_mut().unwrap();
        entry.lower_tick = self.lower_tick;
        entry.upper_tick = self.upper_tick;
        entry.frequency = entry.frequency.saturating_add(1);
        self.counters.hits += 1;
        Some(entry.inode)
    }

    /// Cache `path -> inode`, tagged with `state`'s reflection orbit and
    /// bound to `ledger`'s current checkpoint generation. Evicts a victim
    /// (bounded pseudo-random sample, lowest retention value wins, untrusted
    /// entries are preferred victims) if at capacity.
    pub fn insert(
        &mut self,
        path: &str,
        inode: InodeId,
        state: PhaseState,
        ledger: &StatechainLedger,
        chronos_tick: u64,
    ) {
        if self.capacity == 0 {
            return;
        }
        let orbit = ph_reflection_orbit(state)[0].as_u8();
        self.observe_orbit(orbit);
        self.lower_tick = chronos_tick;

        let (checkpoint_sequence, checkpoint_root) = match ledger.latest_checkpoint() {
            Some(cp) => (Some(cp.sequence), cp.kernel_root),
            None => (None, ledger.equation_root()),
        };
        let entry = PhaseCacheEntry {
            path: path.to_string(),
            inode,
            lower_tick: self.lower_tick,
            upper_tick: self.upper_tick,
            checkpoint_sequence,
            checkpoint_root,
            valid: true,
            frequency: 1,
            orbit,
        };

        if let Some(&existing) = self.index.get(path) {
            self.slots[existing] = Some(entry);
            return;
        }

        let target = if self.slots.len() < self.capacity {
            self.slots.push(Some(entry));
            self.slots.len() - 1
        } else {
            let victim = self.choose_victim(path, ledger);
            if let Some(old) = self.slots[victim].take() {
                self.index.remove(&old.path);
            }
            self.slots[victim] = Some(entry);
            victim
        };
        self.index.insert(path.to_string(), target);
    }

    /// Mark the entry for `path` invalid (does not remove its slot; a
    /// subsequent `get` will treat it as a trust rejection, freeing it for
    /// eviction on the next `insert`). Returns `true` if `path` was cached.
    pub fn invalidate_path(&mut self, path: &str) -> bool {
        if let Some(&index) = self.index.get(path) {
            if let Some(entry) = self.slots[index].as_mut() {
                entry.valid = false;
                return true;
            }
        }
        false
    }

    /// Invalidate `prefix` and every cached path under it (`prefix/`…).
    /// Used after a directory rename so descendant resolutions cannot hit.
    pub fn invalidate_path_prefix(&mut self, prefix: &str) -> bool {
        if prefix.is_empty() {
            return false;
        }
        let child = format!("{prefix}/");
        let paths: Vec<String> = self
            .index
            .keys()
            .filter(|p| *p == prefix || p.starts_with(&child))
            .cloned()
            .collect();
        let mut found = false;
        for path in paths {
            found |= self.invalidate_path(&path);
        }
        found
    }

    /// Mark every cached entry for `inode` invalid, by inode id rather than
    /// path (used when the caller only has the id, e.g. after `rm`).
    /// Returns `true` if at least one entry was found.
    pub fn invalidate_inode(&mut self, inode: InodeId) -> bool {
        let mut found = false;
        for slot in self.slots.iter_mut().flatten() {
            if slot.inode == inode {
                slot.valid = false;
                found = true;
            }
        }
        found
    }

    /// Drop every cache entry. Used after journal recovery so no stale path
    /// resolution survives a reconstructed inode map.
    #[cfg(feature = "phasefs-journal")]
    pub fn invalidate_all(&mut self) {
        for slot in self.slots.iter_mut() {
            *slot = None;
        }
        self.index = crate::platform::BTreeMap::new();
    }

    pub fn len(&self) -> usize {
        self.index.len()
    }

    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    pub fn stats(&self) -> PhaseCacheStats {
        self.counters
    }

    fn is_trusted(entry: &PhaseCacheEntry, ledger: &StatechainLedger) -> bool {
        if !entry.valid {
            return false;
        }
        match entry.checkpoint_sequence {
            None => entry.checkpoint_root == ledger.equation_root(),
            Some(seq) => ledger
                .checkpoints()
                .iter()
                .any(|c| c.sequence == seq && c.kernel_root == entry.checkpoint_root),
        }
    }

    /// Update the self-tracked upper clock: two accesses in the same
    /// reflection orbit are the *same* upper-clock position (this is the
    /// symmetry quotient); a change of orbit advances it by one.
    fn observe_orbit(&mut self, orbit: u8) {
        match self.current_orbit {
            None => self.current_orbit = Some(orbit),
            Some(prev) if prev == orbit => {
                self.current_duration = self.current_duration.saturating_add(1);
            }
            Some(_) => {
                self.duration_ema = (self.duration_ema * 7 + self.current_duration) / 8;
                self.current_duration = 1;
                self.current_orbit = Some(orbit);
                self.upper_tick += 1;
            }
        }
    }

    fn retention_value(&self, entry: &PhaseCacheEntry) -> u32 {
        let lower_age = self.lower_tick.saturating_sub(entry.lower_tick);
        let upper_age = self.upper_tick.saturating_sub(entry.upper_tick);
        let lower_tau = core::cmp::max(MIN_LOWER_TAU, self.duration_ema * 3 / 4);
        let lower_recency = scaled_ratio(lower_tau, lower_age.saturating_add(lower_tau));
        let upper_recency = scaled_ratio(UPPER_TAU, upper_age.saturating_add(UPPER_TAU));
        let frequency_score = core::cmp::min(
            SCALE,
            bit_width(entry.frequency) * SCALE / MAX_FREQUENCY_BITS,
        );
        ((W_LOWER_RECENCY as u64 * lower_recency as u64
            + W_UPPER_RECENCY as u64 * upper_recency as u64
            + W_FREQUENCY as u64 * frequency_score as u64)
            / SCALE as u64) as u32
    }

    /// Bounded pseudo-random victim sample: prefers an untrusted candidate
    /// outright, else the lowest-`retention_value` candidate seen. The seed
    /// is a non-cryptographic hash of `seed_path` mixed with the lower
    /// clock — used only for sampling diversity, never for trust decisions.
    fn choose_victim(&self, seed_path: &str, ledger: &StatechainLedger) -> usize {
        let len = self.slots.len() as u64;
        let seed = fnv1a64(seed_path.as_bytes()) ^ self.lower_tick ^ 0x4556_4943_5449_4f4e;
        let mut rng = splitmix64(seed);
        let mut victim = (rng % len) as usize;
        let samples = VICTIM_SAMPLE_COUNT.min(self.slots.len());
        for _ in 0..samples {
            rng = splitmix64(rng);
            let candidate = (rng % len) as usize;
            let candidate_trusted = self.slots[candidate]
                .as_ref()
                .map(|e| Self::is_trusted(e, ledger))
                .unwrap_or(false);
            if !candidate_trusted {
                return candidate;
            }
            let victim_trusted = self.slots[victim]
                .as_ref()
                .map(|e| Self::is_trusted(e, ledger))
                .unwrap_or(false);
            if !victim_trusted
                || self.retention_value(self.slots[candidate].as_ref().unwrap())
                    < self.retention_value(self.slots[victim].as_ref().unwrap())
            {
                victim = candidate;
            }
        }
        victim
    }
}

/// `numerator / denominator`, scaled to a `SCALE`-fraction fixed-point value.
fn scaled_ratio(numerator: u64, denominator: u64) -> u32 {
    if denominator == 0 {
        return 0;
    }
    ((numerator as u128 * SCALE as u128) / denominator as u128) as u32
}

/// Integer log2-ceiling (`0` for `0`), used as a float-free stand-in for the
/// prototype's `log2(frequency + 1)` frequency ranking.
fn bit_width(value: u32) -> u32 {
    32 - value.leading_zeros()
}

/// SplitMix64, ported byte-for-byte from the prototype. Pure integer
/// arithmetic, `no_std`-safe, used only for victim-sample seeding.
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

/// FNV-1a 64-bit. Non-cryptographic; used only to seed the victim sampler
/// from a path string, never for trust or identity decisions.
fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::state_proof::ObjectKind;
    use crate::kernel::state_root::SubsystemKind;

    /// Advances `ledger`'s kernel root away from its zeroed default so
    /// `seal_checkpoint` succeeds, mirroring how `kernel::record_fs_event`
    /// et al. drive the real ledger.
    fn advance(ledger: &mut StatechainLedger) {
        ledger
            .record_transition(
                ObjectKind::File,
                1,
                SubsystemKind::Filesystem,
                PhaseState::Phi1,
                PhaseState::Phi2,
                "test-advance",
                None,
                b"seed",
            )
            .unwrap();
    }

    #[test]
    fn miss_on_empty_cache() {
        let mut cache = PhaseCacheController::new(4);
        let ledger = StatechainLedger::new();
        assert_eq!(cache.get("/a", &ledger, 0), None);
        assert_eq!(cache.stats().misses, 1);
        assert_eq!(cache.stats().hits, 0);
    }

    #[test]
    fn insert_then_get_hits() {
        let mut cache = PhaseCacheController::new(4);
        let ledger = StatechainLedger::new();
        cache.insert("/a.txt", 7, PhaseState::Phi1, &ledger, 0);
        assert_eq!(cache.get("/a.txt", &ledger, 1), Some(7));
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn invalidate_path_forces_a_miss() {
        let mut cache = PhaseCacheController::new(4);
        let ledger = StatechainLedger::new();
        cache.insert("/a.txt", 7, PhaseState::Phi1, &ledger, 0);
        assert!(cache.invalidate_path("/a.txt"));
        assert_eq!(cache.get("/a.txt", &ledger, 1), None);
        assert_eq!(cache.stats().trust_rejections, 1);
    }

    #[test]
    fn invalidate_inode_finds_entry_by_id() {
        let mut cache = PhaseCacheController::new(4);
        let ledger = StatechainLedger::new();
        cache.insert("/a.txt", 7, PhaseState::Phi1, &ledger, 0);
        assert!(cache.invalidate_inode(7));
        assert!(!cache.invalidate_inode(999));
        assert_eq!(cache.get("/a.txt", &ledger, 1), None);
    }

    #[test]
    fn invalidate_path_prefix_drops_descendants_not_siblings() {
        let mut cache = PhaseCacheController::new(8);
        let ledger = StatechainLedger::new();
        cache.insert("/docs", 1, PhaseState::Phi2, &ledger, 0);
        cache.insert("/docs/a.txt", 2, PhaseState::Phi1, &ledger, 0);
        cache.insert("/other.txt", 3, PhaseState::Phi1, &ledger, 0);
        assert!(cache.invalidate_path_prefix("/docs"));
        assert_eq!(cache.get("/docs", &ledger, 1), None);
        assert_eq!(cache.get("/docs/a.txt", &ledger, 1), None);
        assert_eq!(cache.get("/other.txt", &ledger, 1), Some(3));
    }

    #[test]
    fn eviction_makes_room_at_capacity() {
        let mut cache = PhaseCacheController::new(2);
        let ledger = StatechainLedger::new();
        cache.insert("/a", 1, PhaseState::Phi1, &ledger, 0);
        cache.insert("/b", 2, PhaseState::Phi1, &ledger, 1);
        cache.insert("/c", 3, PhaseState::Phi1, &ledger, 2);
        assert_eq!(cache.len(), 2);
        // The third insert must have evicted exactly one of the first two.
        let survivors = [
            cache.get("/a", &ledger, 3),
            cache.get("/b", &ledger, 3),
            cache.get("/c", &ledger, 3),
        ]
        .into_iter()
        .filter(Option::is_some)
        .count();
        assert_eq!(survivors, 2);
    }

    #[test]
    fn same_orbit_accesses_do_not_advance_upper_tick() {
        let mut cache = PhaseCacheController::new(4);
        let ledger = StatechainLedger::new();
        cache.insert("/a", 1, PhaseState::Phi0, &ledger, 0);
        assert_eq!(cache.upper_tick, 0);
        // Phi3 is Phi0's reflection mirror -- same orbit, no transition.
        cache.insert("/b", 2, PhaseState::Phi3, &ledger, 1);
        assert_eq!(cache.upper_tick, 0);
        // Phi1 is a different orbit -- this must advance the upper clock.
        cache.insert("/c", 3, PhaseState::Phi1, &ledger, 2);
        assert_eq!(cache.upper_tick, 1);
    }

    #[test]
    fn genesis_entries_stay_trusted_across_the_first_checkpoint() {
        let mut cache = PhaseCacheController::new(4);
        let mut ledger = StatechainLedger::new();
        cache.insert("/a.txt", 7, PhaseState::Phi1, &ledger, 0);
        advance(&mut ledger);
        ledger.seal_checkpoint().unwrap();
        // Sealing a checkpoint must not retroactively invalidate an entry
        // bound to the stable pre-checkpoint genesis root.
        assert_eq!(cache.get("/a.txt", &ledger, 1), Some(7));
    }

    /// Trust-negative test, ported from the pasted host-research
    /// prototype's `run_trust_negative_test`, but using the *real*
    /// `StatechainLedger` and its own `replace_checkpoint_for_audit_test`
    /// audit hook (the same convention `statechain.rs` already uses for
    /// this exact purpose) instead of a synthetic stand-in.
    #[test]
    fn corrupted_checkpoint_root_is_rejected_not_served() {
        let mut cache = PhaseCacheController::new(4);
        let mut ledger = StatechainLedger::new();
        advance(&mut ledger);
        let checkpoint = ledger.seal_checkpoint().unwrap();

        cache.insert("/secret", 42, PhaseState::Phi1, &ledger, 10);
        assert_eq!(cache.get("/secret", &ledger, 11), Some(42));

        let mut corrupted = checkpoint.clone();
        corrupted.kernel_root = [9u8; 32];
        ledger.replace_checkpoint_for_audit_test(0, corrupted);

        // Rejected, not served: a miss was recorded, not a second hit.
        assert_eq!(cache.get("/secret", &ledger, 12), None);
        assert_eq!(cache.stats().trust_rejections, 1);
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().misses, 1);
    }

    #[test]
    fn is_trusted_rejects_an_invalidated_entry() {
        let mut cache = PhaseCacheController::new(4);
        let ledger = StatechainLedger::new();
        cache.insert("/a.txt", 1, PhaseState::Phi1, &ledger, 0);
        cache.invalidate_path("/a.txt");
        assert_eq!(cache.get("/a.txt", &ledger, 1), None);
    }
}
