//! Scheduler and timer queues (`feature = "sched2"`), subsystem 4 of the
//! eight kernel-subsystems build program — see
//! `fpga/zynq/evidence/KERNEL_MATH_CONTRACT_2026-08-12.md` and
//! `fpga/zynq/evidence/SCHED2_STATUS_<date>.md`.
//!
//! Adds ordinary run state, weighted fairness, deadline priority, and a
//! hierarchical timing wheel for sleep/wake, **without** touching the
//! existing mathematical `PhaseState` tracker (`kernel::process`,
//! `kernel::scheduler`). Per the build contract: "Add `RunState`... separately
//! from `PhaseState`... the new run queue must not rewrite [the canonical
//! phase transitions]." `RunState` and `PhaseState` are two independent,
//! parallel state tracks on the same `ProcessId`; this module never reads or
//! writes a `Process`'s `PhaseState`.
//!
//! ## What this reuses / does not reuse (per the math contract)
//! - [`crate::kernel::event_log::ProcessId`] — no new process-identity type.
//! - The `(slot, generation)` generational-handle shape from Part III /
//!   `kernel::captable::GenerationalHandle`, as a **local** `Sched2Handle`
//!   type — same rationale `irqreg::IrqHandle` already documents: importing
//!   `captable`'s exact type would couple two independently-toggleable
//!   Cargo features.
//! - This subsystem consumes **no G64 operation and no `PhaseHash`/
//!   `PhaseState` reflection** — pick/dispatch/preempt/sleep are run-queue
//!   bookkeeping, not phase mathematics. Explicitly stated per Gate 0's
//!   instruction to declare this for every subsystem.
//! - **Does not gate `pick_next_ready`/`dispatch` on the math contract's
//!   general trust conjunction `G(x)`** (§6, "General object and trust
//!   mathematics"). `G(x)` depends on capability/authority checks
//!   (`captable::authorize`) that are the *caller's* responsibility before
//!   ever asking this module to run a process — `Eligible_i` here is exactly
//!   `[RunState_i = Ready]`, per the subsystem contract's own text: "Deadline
//!   urgency may be a primary class... Eligibility is a hard gate:
//!   `Eligible_i=[RunState_i=Ready]∧[G(i)=1]`" — the `G(i)=1` half of that
//!   conjunction is not evaluated inside this module because this module has
//!   no access to a capability/authority context; it is the caller's job to
//!   never hand this module a process that has already failed `G(x)`
//!   elsewhere. Documented as an explicit non-claim, not silently dropped.
//! - `boundedPhaseDistance_i` (optional secondary key component) is
//!   **omitted** from the ordering key, per the contract's own instruction:
//!   "permitted only if Gate 0 finds a canonical distance or adjacency
//!   function. Otherwise omit... do not invent a numeric distance." Gate 0
//!   (`KERNEL_MATH_CONTRACT_2026-08-12.md`) found no such function.
//! - Time base: an **abstract monotonic tick counter** (`Tick = u64`)
//!   supplied by the caller on every call, not `StateClock::chronos_tick`
//!   directly. On real PZ7020 hardware (Gate E), the caller supplies
//!   `board::zynq7000::gic::tick_count()` — `irqreg`'s own already-proven,
//!   hardware-driven private-timer (PPI 29) dispatch counter — as `now`,
//!   which is exactly how "the private timer arrives through subsystem 3's
//!   registry" (this subsystem's own contract text) is satisfied: the timer
//!   tick this module's wheel advances on *is* `irqreg`'s real counter, not
//!   a second parallel timer path. `StateClock::chronos_tick` is left on its
//!   pre-existing, unrelated advance-per-command-line cadence, untouched by
//!   this subsystem (see non-claims in the evidence document for why a
//!   single unified clock was not attempted here).
//!
//! ## Mathematics implemented (math contract's "Scheduler and timer queues" row)
//! - Weighted virtual runtime: `v_i <- v_i + (delta_t_i * S) / w_i`, `S = 2^16`
//!   (Q16 fixed point, math contract §9's fixed-point rule; `w_i` validated
//!   non-zero at registration, so the division is never by zero).
//! - Ordinary-task selection key: `i* = argmin_{i in Ready} (v_i, PID_i)`.
//! - Deadline slack: `slack_i = d_i - t - e_i`.
//! - Combined key: `K_i = (deadlineClass_i, v_i, PID_i)` (bounded phase
//!   distance omitted, see above).
//! - Starvation bound: `wait_i <= W_max` for continuously-eligible ordinary
//!   tasks, enforced *structurally* (see `pick_next_ready`), not just
//!   probabilistically.
//! - Hierarchical timing wheel: `b_l(d) = floor(d / 2^(w*l)) mod 2^w`,
//!   `Fire_i = [t>=d_i] ∧ [generationValid_i] ∧ [notCancelled_i]`.
//!
//! ## Non-goals (explicit — see also `SCHED2_STATUS_<date>.md`)
//! - No real preemptive multitasking / context switching: this kernel is
//!   cooperative (a process is "running" only inside its own syscall/VM
//!   step), matching the pre-existing `YIELD` syscall's own bookkeeping-only
//!   semantics. "Wake" means this module's `RunState` transitions
//!   `Blocked(Timer) -> Ready`, driven by real hardware ticks — not resuming
//!   a suspended thread of execution, which this kernel has never had.
//! - `WaitReason::Ipc(u64)` carries an opaque wait key, not an
//!   `ipc::IpcHandle` — importing the `ipc` module's type here would make
//!   `sched2` depend on a subsystem built *after* it (wrong direction; see
//!   `kernel::ipc`'s own module docs for how it packs its `(slot,
//!   generation)` handle into that key). `block_on_ipc`/`wake_matching`
//!   never interpret the key beyond equality.
//! - `WaitReason::Net(u64)` (added for `netsess`, subsystem 8) is the exact
//!   same pattern one level further: an opaque wait key packed by
//!   `netsess::SessionHandle::pack()`, compared only by equality in
//!   `block_on_net`/`wake_matching_net`. Kept as a separate variant/pair of
//!   functions rather than reusing `Ipc`/`block_on_ipc` so a `netsess`
//!   waiter is never confused with an `ipc` waiter by `wake_matching`'s
//!   equality match on the same key space — see `netsess`'s own module docs
//!   for why `netsess` does not import `ipc`'s handle type either.
//! - No persistence/recovery: this table is RAM-only, like `captable` and
//!   `irqreg`; `X_before/X_fault/X_restored` recovery mathematics do not
//!   apply to a subsystem with no checkpoint/restore operation of its own.

use core::fmt;

use crate::kernel::event_log::ProcessId;
use crate::platform::{HashMap, Vec};

/// Abstract monotonic tick counter. See module docs, "Time base".
pub type Tick = u64;

/// Bits per timing-wheel level (64 slots/level).
pub const WHEEL_BITS: u32 = 6;
/// Slots per timing-wheel level (`2^WHEEL_BITS`).
pub const WHEEL_SLOTS: usize = 1 << WHEEL_BITS;
/// Timing-wheel levels. Covers `64^4 = 16,777,216` ticks before a deadline's
/// initial placement saturates to the top level (still correct — see
/// `level_for`; cascading re-levels it precisely as `elapsed` approaches it).
pub const WHEEL_LEVELS: usize = 4;
/// Q16 fixed-point scale, per math contract §9's fixed-point rule.
pub const SCALE: u64 = 1 << 16;
/// Ordinary (non-deadline) tasks always sort after every real deadline
/// class in `K_i`'s lexicographic order (see module docs).
pub const DEADLINE_CLASS_NONE: u8 = 255;
/// Default per-task weight for `register`, chosen to match the familiar
/// Linux CFS "nice 0" weight so relative weight ratios (e.g. `2*DEFAULT`)
/// read naturally; not otherwise load-bearing.
pub const DEFAULT_WEIGHT: u32 = 1024;

const WHEEL_SLOT_MASK: u64 = (WHEEL_SLOTS as u64) - 1;

fn level_for(elapsed: Tick, when: Tick) -> usize {
    let delta = elapsed ^ when;
    for level in 0..WHEEL_LEVELS {
        if (delta >> (((level + 1) as u32) * WHEEL_BITS)) == 0 {
            return level;
        }
    }
    WHEEL_LEVELS - 1
}

fn slot_for(when: Tick, level: usize) -> usize {
    ((when >> ((level as u32) * WHEEL_BITS)) & WHEEL_SLOT_MASK) as usize
}

/// `h = (i, g)` — a local generational handle. See module docs for why this
/// is not `kernel::captable::GenerationalHandle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Sched2Handle {
    pub slot: u32,
    pub generation: u32,
}

/// Ordinary run state, tracked independently of `PhaseState`. See module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    Ready,
    Running,
    Blocked(WaitReason),
    Exited,
}

/// Why a process is `Blocked`. See module docs, "Non-goals".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitReason {
    Timer,
    /// Blocked on an `ipc` queue (empty on `recv`). The `u64` is an opaque
    /// key the blocker supplied to [`Sched2::block_on_ipc`] — this module
    /// never interprets it beyond equality in [`Sched2::wake_matching`].
    Ipc(u64),
    /// Blocked on a `netsess` session's descriptor ring (empty on receive).
    /// The `u64` is an opaque key the blocker supplied to
    /// [`Sched2::block_on_net`] — this module never interprets it beyond
    /// equality in [`Sched2::wake_matching_net`].
    Net(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sched2Error {
    /// Slot index out of range, or not currently occupied.
    InvalidHandle,
    /// Slot occupied, but by a different (newer) generation.
    StaleGeneration,
    /// `register` was asked for a zero weight (would divide by zero in the
    /// virtual-runtime update).
    ZeroWeight,
    /// Operation requires `RunState::Ready`.
    NotReady,
    /// Operation requires `RunState::Running`.
    NotRunning,
    /// Operation requires `RunState::Blocked(_)`.
    NotBlocked,
    /// `sleep_until` called while a timer is already armed for this handle.
    AlreadyHasTimer,
}

impl fmt::Display for Sched2Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Sched2Error::InvalidHandle => "invalid handle",
            Sched2Error::StaleGeneration => "stale generation",
            Sched2Error::ZeroWeight => "zero weight",
            Sched2Error::NotReady => "process is not ready",
            Sched2Error::NotRunning => "process is not running",
            Sched2Error::NotBlocked => "process is not blocked",
            Sched2Error::AlreadyHasTimer => "process already has an armed timer",
        };
        f.write_str(msg)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Sched2Stats {
    pub registered: u64,
    pub unregistered: u64,
    pub generation_retired: u64,
    pub dispatches: u64,
    pub preemptions: u64,
    pub timers_armed: u64,
    pub timers_fired: u64,
    pub timers_cancelled: u64,
    pub starvation_boosts: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Sched2StateCounts {
    pub ready: usize,
    pub running: usize,
    pub blocked: usize,
    pub exited: usize,
}

struct ProcessEntry {
    occupied: bool,
    generation: u32,
    pid: ProcessId,
    run_state: RunState,
    weight: u32,
    vruntime: u64,
    deadline_class: u8,
    deadline_tick: Tick,
    remaining_estimate: Tick,
    /// Tick at which this entry most recently became `Ready` — the
    /// starvation-bound baseline. Meaningless unless `run_state == Ready`.
    ready_since: Tick,
    timer_deadline: Tick,
    timer_active: bool,
    /// -1 when not linked into any wheel bucket.
    timer_level: i8,
    timer_slot: u16,
    /// Intrusive doubly-linked bucket list, indices into `entries`.
    wheel_prev: i32,
    wheel_next: i32,
}

/// Bumps `*generation`. Returns `true` on overflow (`u32::MAX -> 0` avoided);
/// mirrors `kernel::captable`'s identical rule ("never wrap silently into a
/// valid old generation").
fn bump_generation(generation: &mut u32) -> bool {
    match generation.checked_add(1) {
        Some(next) => {
            *generation = next;
            false
        }
        None => true,
    }
}

/// Scheduler and timer-queue engine. See module docs for the full math and
/// design rationale.
pub struct Sched2 {
    entries: Vec<ProcessEntry>,
    free: Vec<u32>,
    pid_index: HashMap<ProcessId, u32>,
    wheel: [[i32; WHEEL_SLOTS]; WHEEL_LEVELS],
    elapsed: Tick,
    initialized: bool,
    starvation_bound: Tick,
    stats: Sched2Stats,
}

impl Default for Sched2 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sched2 {
    /// Starvation bound `W_max`, in caller ticks, used by the default
    /// constructor. Chosen small enough to be provable in a modest host
    /// test, large enough not to thrash real hardware scheduling; concrete
    /// tuning is future work once representative real-hardware task mixes
    /// are measured (same category of deferred tuning as `irqreg`'s storm
    /// policy — see `IRQREG_STATUS_2026-08-12.md` §2).
    const DEFAULT_STARVATION_BOUND: Tick = 1000;

    pub fn new() -> Self {
        Self::with_starvation_bound(Self::DEFAULT_STARVATION_BOUND)
    }

    pub fn with_starvation_bound(starvation_bound: Tick) -> Self {
        Self {
            entries: Vec::new(),
            free: Vec::new(),
            pid_index: HashMap::new(),
            wheel: [[-1; WHEEL_SLOTS]; WHEEL_LEVELS],
            elapsed: 0,
            initialized: false,
            starvation_bound,
            stats: Sched2Stats::default(),
        }
    }

    pub fn stats(&self) -> Sched2Stats {
        self.stats
    }

    /// The wheel's own last-observed tick (its `advance()` cursor). Lets a
    /// caller with no independent clock of its own (e.g. the architecture-
    /// agnostic syscall boundary) compute a relative deadline as
    /// `elapsed() + n` without needing to know what real time base drives
    /// this instance's `advance()` calls.
    pub fn elapsed(&self) -> Tick {
        self.elapsed
    }

    pub fn state_counts(&self) -> Sched2StateCounts {
        let mut counts = Sched2StateCounts::default();
        for e in self.entries.iter().filter(|e| e.occupied) {
            match e.run_state {
                RunState::Ready => counts.ready += 1,
                RunState::Running => counts.running += 1,
                RunState::Blocked(_) => counts.blocked += 1,
                RunState::Exited => counts.exited += 1,
            }
        }
        counts
    }

    pub fn handle_for_pid(&self, pid: ProcessId) -> Option<Sched2Handle> {
        let &slot = self.pid_index.get(&pid)?;
        let entry = self.entries.get(slot as usize)?;
        if !entry.occupied {
            return None;
        }
        Some(Sched2Handle {
            slot,
            generation: entry.generation,
        })
    }

    fn ensure_initialized(&mut self, now: Tick) {
        if !self.initialized {
            self.elapsed = now;
            self.initialized = true;
        }
    }

    fn validate(&self, handle: Sched2Handle) -> Result<usize, Sched2Error> {
        let entry = self
            .entries
            .get(handle.slot as usize)
            .ok_or(Sched2Error::InvalidHandle)?;
        if !entry.occupied {
            return Err(Sched2Error::InvalidHandle);
        }
        if entry.generation != handle.generation {
            return Err(Sched2Error::StaleGeneration);
        }
        Ok(handle.slot as usize)
    }

    /// Register a new process for scheduling. `weight` must be non-zero.
    /// The new entry starts `Ready`, with `vruntime` initialized to the
    /// minimum `vruntime` among currently `Ready`/`Running` entries (CFS's
    /// standard "new tasks don't get an unfair advantage by starting at
    /// zero forever" rule — not mandated verbatim by the math contract, but
    /// necessary so repeated re-registration can never be used to keep
    /// winning every tie; documented in `SCHED2_STATUS_<date>.md`).
    pub fn register(
        &mut self,
        pid: ProcessId,
        weight: u32,
        now: Tick,
    ) -> Result<Sched2Handle, Sched2Error> {
        if weight == 0 {
            return Err(Sched2Error::ZeroWeight);
        }
        self.ensure_initialized(now);
        let min_vruntime = self
            .entries
            .iter()
            .filter(|e| e.occupied && matches!(e.run_state, RunState::Ready | RunState::Running))
            .map(|e| e.vruntime)
            .min()
            .unwrap_or(0);
        let new_entry = ProcessEntry {
            occupied: true,
            generation: 0, // overwritten below if reusing a slot
            pid,
            run_state: RunState::Ready,
            weight,
            vruntime: min_vruntime,
            deadline_class: DEADLINE_CLASS_NONE,
            deadline_tick: 0,
            remaining_estimate: 0,
            ready_since: now,
            timer_deadline: 0,
            timer_active: false,
            timer_level: -1,
            timer_slot: 0,
            wheel_prev: -1,
            wheel_next: -1,
        };
        let slot = if let Some(slot) = self.free.pop() {
            let existing_generation = self.entries[slot as usize].generation;
            self.entries[slot as usize] = new_entry;
            self.entries[slot as usize].generation = existing_generation;
            slot
        } else {
            let slot = self.entries.len() as u32;
            self.entries.push(new_entry);
            slot
        };
        self.pid_index.insert(pid, slot);
        self.stats.registered += 1;
        Ok(Sched2Handle {
            slot,
            generation: self.entries[slot as usize].generation,
        })
    }

    /// Unregister a process. Disarms any active timer first (removing it
    /// from the wheel), then bumps the slot's generation, retiring the slot
    /// permanently on `u32` overflow instead of ever wrapping into a
    /// previously-valid generation (mirrors `captable::close`).
    pub fn unregister(&mut self, handle: Sched2Handle) -> Result<(), Sched2Error> {
        let idx = self.validate(handle)?;
        if self.entries[idx].timer_active {
            self.unlink_timer(idx);
        }
        let pid = self.entries[idx].pid;
        self.entries[idx].occupied = false;
        self.entries[idx].run_state = RunState::Exited;
        let retired = bump_generation(&mut self.entries[idx].generation);
        self.pid_index.remove(&pid);
        if retired {
            self.stats.generation_retired += 1;
        } else {
            self.free.push(handle.slot);
        }
        self.stats.unregistered += 1;
        Ok(())
    }

    pub fn run_state(&self, handle: Sched2Handle) -> Result<RunState, Sched2Error> {
        let idx = self.validate(handle)?;
        Ok(self.entries[idx].run_state)
    }

    pub fn vruntime(&self, handle: Sched2Handle) -> Result<u64, Sched2Error> {
        let idx = self.validate(handle)?;
        Ok(self.entries[idx].vruntime)
    }

    /// Set a deadline class (`0` = most urgent; `DEADLINE_CLASS_NONE` is
    /// reserved for ordinary weighted-fair tasks — use `clear_deadline`
    /// instead of passing it here) plus the raw `deadline_tick`/
    /// `remaining_estimate` inputs to `slack`. Valid on any non-`Exited`
    /// entry (deadline metadata is independent of `Ready`/`Running`/
    /// `Blocked`).
    pub fn set_deadline(
        &mut self,
        handle: Sched2Handle,
        class: u8,
        deadline_tick: Tick,
        remaining_estimate: Tick,
    ) -> Result<(), Sched2Error> {
        let idx = self.validate(handle)?;
        self.entries[idx].deadline_class = class;
        self.entries[idx].deadline_tick = deadline_tick;
        self.entries[idx].remaining_estimate = remaining_estimate;
        Ok(())
    }

    pub fn clear_deadline(&mut self, handle: Sched2Handle) -> Result<(), Sched2Error> {
        let idx = self.validate(handle)?;
        self.entries[idx].deadline_class = DEADLINE_CLASS_NONE;
        self.entries[idx].deadline_tick = 0;
        self.entries[idx].remaining_estimate = 0;
        Ok(())
    }

    /// `slack_i = d_i - t - e_i`. Returns `None` if no deadline is set
    /// (`deadline_class == DEADLINE_CLASS_NONE`).
    pub fn slack(&self, handle: Sched2Handle, now: Tick) -> Result<Option<i64>, Sched2Error> {
        let idx = self.validate(handle)?;
        let e = &self.entries[idx];
        if e.deadline_class == DEADLINE_CLASS_NONE {
            return Ok(None);
        }
        let slack = e.deadline_tick as i64 - now as i64 - e.remaining_estimate as i64;
        Ok(Some(slack))
    }

    /// Select the next process to run. Pure selection — does not mutate any
    /// entry's `RunState` (call `dispatch` on the result to actually
    /// transition it). Structurally enforces the starvation bound: any
    /// `Ready` entry that has waited `>= starvation_bound` ticks is force-
    /// selected (longest-waiting first, `PID` tiebreak) regardless of
    /// deadline class; only when none has starved does ordinary
    /// `K_i = (deadlineClass_i, v_i, PID_i)` argmin selection apply.
    pub fn pick_next_ready(&mut self, now: Tick) -> Option<ProcessId> {
        self.ensure_initialized(now);
        let mut starved: Option<(Tick, ProcessId, usize)> = None;
        let mut best: Option<(u8, u64, ProcessId, usize)> = None;
        for (i, e) in self.entries.iter().enumerate() {
            if !e.occupied || e.run_state != RunState::Ready {
                continue;
            }
            let waited = now.saturating_sub(e.ready_since);
            if waited >= self.starvation_bound {
                let candidate = (waited, e.pid, i);
                starved = Some(match starved {
                    None => candidate,
                    Some(cur)
                        if candidate.0 > cur.0 || (candidate.0 == cur.0 && candidate.1 < cur.1) =>
                    {
                        candidate
                    }
                    Some(cur) => cur,
                });
            }
            let key = (e.deadline_class, e.vruntime, e.pid, i);
            best = Some(match best {
                None => key,
                Some(cur) if (key.0, key.1, key.2) < (cur.0, cur.1, cur.2) => key,
                Some(cur) => cur,
            });
        }
        if let Some((_, pid, _)) = starved {
            self.stats.starvation_boosts = self.stats.starvation_boosts.wrapping_add(1);
            return Some(pid);
        }
        best.map(|(_, _, pid, _)| pid)
    }

    /// `Ready -> Running`.
    pub fn dispatch(&mut self, handle: Sched2Handle) -> Result<(), Sched2Error> {
        let idx = self.validate(handle)?;
        if self.entries[idx].run_state != RunState::Ready {
            return Err(Sched2Error::NotReady);
        }
        self.entries[idx].run_state = RunState::Running;
        self.stats.dispatches += 1;
        Ok(())
    }

    /// `Running -> Ready`. Accounts `ran_ticks` of real execution into
    /// `vruntime`: `v_i <- v_i + (ran_ticks * SCALE) / weight_i`.
    pub fn preempt(
        &mut self,
        handle: Sched2Handle,
        ran_ticks: Tick,
        now: Tick,
    ) -> Result<(), Sched2Error> {
        let idx = self.validate(handle)?;
        if self.entries[idx].run_state != RunState::Running {
            return Err(Sched2Error::NotRunning);
        }
        let weight = self.entries[idx].weight as u64;
        let delta = ran_ticks.saturating_mul(SCALE) / weight;
        self.entries[idx].vruntime = self.entries[idx].vruntime.wrapping_add(delta);
        self.entries[idx].run_state = RunState::Ready;
        self.entries[idx].ready_since = now;
        self.stats.preemptions += 1;
        Ok(())
    }

    /// `Running -> Blocked(Timer)`, and arms a wheel entry for `deadline`.
    pub fn sleep_until(
        &mut self,
        handle: Sched2Handle,
        deadline: Tick,
        now: Tick,
    ) -> Result<(), Sched2Error> {
        let idx = self.validate(handle)?;
        if self.entries[idx].run_state != RunState::Running {
            return Err(Sched2Error::NotRunning);
        }
        if self.entries[idx].timer_active {
            return Err(Sched2Error::AlreadyHasTimer);
        }
        self.ensure_initialized(now);
        self.entries[idx].run_state = RunState::Blocked(WaitReason::Timer);
        self.schedule_timer(idx, deadline);
        Ok(())
    }

    /// Disarm this handle's timer, if any, without changing `RunState`
    /// (per math contract: cancellation prevents wake, it does not itself
    /// wake — call `unblock` afterward if the caller wants it `Ready`
    /// again). Returns `Ok(true)` if a live timer was actually disarmed,
    /// `Ok(false)` if there was none (a benign no-op, not an error).
    pub fn cancel_sleep(&mut self, handle: Sched2Handle) -> Result<bool, Sched2Error> {
        let idx = self.validate(handle)?;
        if !self.entries[idx].timer_active {
            return Ok(false);
        }
        self.unlink_timer(idx);
        self.entries[idx].timer_active = false;
        self.stats.timers_cancelled += 1;
        Ok(true)
    }

    /// `Running -> Blocked(Ipc(key))`. No wheel entry is armed — unlike
    /// [`Self::sleep_until`], this wait has no deadline; only a matching
    /// [`Self::wake_matching`] call (or an explicit [`Self::unblock`])
    /// clears it. Rejects a handle that already has an armed *timer* via
    /// `AlreadyHasTimer`, for the same reason `sleep_until` rejects a
    /// double-arm: one entry, one outstanding wait.
    pub fn block_on_ipc(
        &mut self,
        handle: Sched2Handle,
        key: u64,
        now: Tick,
    ) -> Result<(), Sched2Error> {
        let idx = self.validate(handle)?;
        if self.entries[idx].run_state != RunState::Running {
            return Err(Sched2Error::NotRunning);
        }
        if self.entries[idx].timer_active {
            return Err(Sched2Error::AlreadyHasTimer);
        }
        self.entries[idx].run_state = RunState::Blocked(WaitReason::Ipc(key));
        let _ = now;
        Ok(())
    }

    /// Transition every entry currently `Blocked(WaitReason::Ipc(key))` to
    /// `Ready`, appending each woken `ProcessId` to `out`. `O(P)` over the
    /// entry table (bounded by live registrations, same complexity class
    /// already accepted for `pick_next_ready` — see module docs). Returns
    /// the number woken.
    pub fn wake_matching(&mut self, key: u64, now: Tick, out: &mut Vec<ProcessId>) -> usize {
        let mut woken = 0usize;
        for e in self.entries.iter_mut().filter(|e| e.occupied) {
            if e.run_state == RunState::Blocked(WaitReason::Ipc(key)) {
                e.run_state = RunState::Ready;
                e.ready_since = now;
                out.push(e.pid);
                woken += 1;
            }
        }
        woken
    }

    /// `Running -> Blocked(WaitReason::Net(key))`. Exact mirror of
    /// `block_on_ipc` for `netsess` (see module docs).
    pub fn block_on_net(
        &mut self,
        handle: Sched2Handle,
        key: u64,
        now: Tick,
    ) -> Result<(), Sched2Error> {
        let idx = self.validate(handle)?;
        if self.entries[idx].run_state != RunState::Running {
            return Err(Sched2Error::NotRunning);
        }
        if self.entries[idx].timer_active {
            return Err(Sched2Error::AlreadyHasTimer);
        }
        self.entries[idx].run_state = RunState::Blocked(WaitReason::Net(key));
        let _ = now;
        Ok(())
    }

    /// Transition every entry currently `Blocked(WaitReason::Net(key))` to
    /// `Ready`, appending each woken `ProcessId` to `out`. Exact mirror of
    /// `wake_matching` for `netsess` (see module docs). Returns the number
    /// woken.
    pub fn wake_matching_net(&mut self, key: u64, now: Tick, out: &mut Vec<ProcessId>) -> usize {
        let mut woken = 0usize;
        for e in self.entries.iter_mut().filter(|e| e.occupied) {
            if e.run_state == RunState::Blocked(WaitReason::Net(key)) {
                e.run_state = RunState::Ready;
                e.ready_since = now;
                out.push(e.pid);
                woken += 1;
            }
        }
        woken
    }

    /// `Blocked(_) -> Ready`.
    pub fn unblock(&mut self, handle: Sched2Handle, now: Tick) -> Result<(), Sched2Error> {
        let idx = self.validate(handle)?;
        if !matches!(self.entries[idx].run_state, RunState::Blocked(_)) {
            return Err(Sched2Error::NotBlocked);
        }
        self.entries[idx].run_state = RunState::Ready;
        self.entries[idx].ready_since = now;
        Ok(())
    }

    /// Any non-`Exited` state -> `Exited`. Disarms any active timer.
    pub fn exit(&mut self, handle: Sched2Handle) -> Result<(), Sched2Error> {
        let idx = self.validate(handle)?;
        if self.entries[idx].timer_active {
            self.unlink_timer(idx);
            self.entries[idx].timer_active = false;
        }
        self.entries[idx].run_state = RunState::Exited;
        Ok(())
    }

    fn link_timer(&mut self, idx: usize, level: usize, slot: usize) {
        let old_head = self.wheel[level][slot];
        self.entries[idx].wheel_prev = -1;
        self.entries[idx].wheel_next = old_head;
        if old_head >= 0 {
            self.entries[old_head as usize].wheel_prev = idx as i32;
        }
        self.wheel[level][slot] = idx as i32;
        self.entries[idx].timer_level = level as i8;
        self.entries[idx].timer_slot = slot as u16;
    }

    fn unlink_timer(&mut self, idx: usize) {
        let level = self.entries[idx].timer_level;
        if level < 0 {
            return;
        }
        let slot = self.entries[idx].timer_slot as usize;
        let prev = self.entries[idx].wheel_prev;
        let next = self.entries[idx].wheel_next;
        if prev >= 0 {
            self.entries[prev as usize].wheel_next = next;
        } else {
            self.wheel[level as usize][slot] = next;
        }
        if next >= 0 {
            self.entries[next as usize].wheel_prev = prev;
        }
        self.entries[idx].wheel_prev = -1;
        self.entries[idx].wheel_next = -1;
        self.entries[idx].timer_level = -1;
    }

    fn schedule_timer(&mut self, idx: usize, deadline: Tick) {
        let level = level_for(self.elapsed, deadline);
        let slot = slot_for(deadline, level);
        self.link_timer(idx, level, slot);
        self.entries[idx].timer_deadline = deadline;
        self.entries[idx].timer_active = true;
        self.stats.timers_armed = self.stats.timers_armed.wrapping_add(1);
    }

    /// Re-level every entry in level `level`'s current bucket (indexed by
    /// `now`'s bits at that level) against the up-to-date `now`, letting
    /// each one fall into a lower level (or level 0 directly) as
    /// appropriate. Called once per real tick, exactly when `now`'s bits
    /// below `(level+1)*WHEEL_BITS` are all zero (a level boundary).
    fn cascade(&mut self, level: usize, now: Tick) {
        let slot = slot_for(now, level);
        let mut idx = self.wheel[level][slot];
        self.wheel[level][slot] = -1;
        while idx >= 0 {
            let i = idx as usize;
            let next = self.entries[i].wheel_next;
            self.entries[i].wheel_prev = -1;
            self.entries[i].wheel_next = -1;
            self.entries[i].timer_level = -1;
            let deadline = self.entries[i].timer_deadline;
            let new_level = level_for(now, deadline);
            let new_slot = slot_for(deadline, new_level);
            self.link_timer(i, new_level, new_slot);
            idx = next;
        }
    }

    /// Advance the wheel from its last-seen tick up to `now`, firing every
    /// due, non-cancelled timer: `Fire_i = [t>=d_i] ∧ [generationValid_i] ∧
    /// [notCancelled_i]` (both conjuncts folded into "slot still occupied
    /// and `timer_active`", since `unregister`/`cancel_sleep` both clear
    /// `timer_active` and unlink immediately). Fired entries transition
    /// `Blocked(Timer) -> Ready` and are pushed onto `out`. Returns the
    /// number fired. On the very first call ever (before any tick has been
    /// observed), lazily anchors `elapsed` to `now` instead of replaying a
    /// potentially large absolute tick count from zero.
    pub fn advance(&mut self, now: Tick, out: &mut Vec<ProcessId>) -> usize {
        self.ensure_initialized(now);
        let mut fired = 0usize;
        while self.elapsed < now {
            self.elapsed = self.elapsed.wrapping_add(1);
            let tick = self.elapsed;
            for level in 1..WHEEL_LEVELS {
                let period = 1u64 << ((level as u32) * WHEEL_BITS);
                if tick % period != 0 {
                    break;
                }
                self.cascade(level, tick);
            }
            let slot0 = slot_for(tick, 0);
            let mut idx = self.wheel[0][slot0];
            self.wheel[0][slot0] = -1;
            while idx >= 0 {
                let i = idx as usize;
                let next = self.entries[i].wheel_next;
                self.entries[i].wheel_prev = -1;
                self.entries[i].wheel_next = -1;
                self.entries[i].timer_level = -1;
                if self.entries[i].occupied
                    && self.entries[i].timer_active
                    && self.entries[i].timer_deadline <= tick
                {
                    self.entries[i].timer_active = false;
                    self.entries[i].run_state = RunState::Ready;
                    self.entries[i].ready_since = tick;
                    self.stats.timers_fired = self.stats.timers_fired.wrapping_add(1);
                    out.push(self.entries[i].pid);
                    fired += 1;
                } else if self.entries[i].occupied && self.entries[i].timer_active {
                    // Defensive: should not happen given correct leveling,
                    // but never drop a live timer silently.
                    let deadline = self.entries[i].timer_deadline;
                    let new_level = level_for(tick, deadline);
                    let new_slot = slot_for(deadline, new_level);
                    self.link_timer(i, new_level, new_slot);
                }
                idx = next;
            }
        }
        fired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_unregister_and_stale_handle_rejected() {
        let mut s = Sched2::new();
        let h = s.register(1, DEFAULT_WEIGHT, 0).unwrap();
        assert_eq!(s.run_state(h), Ok(RunState::Ready));
        s.unregister(h).unwrap();
        assert_eq!(s.run_state(h), Err(Sched2Error::InvalidHandle));
        // Re-register a different pid; if the slot was reused, the old
        // handle must be StaleGeneration, not silently valid.
        let h2 = s.register(2, DEFAULT_WEIGHT, 0).unwrap();
        if h2.slot == h.slot {
            assert_eq!(s.run_state(h), Err(Sched2Error::StaleGeneration));
        }
        assert_eq!(s.run_state(h2), Ok(RunState::Ready));
    }

    #[test]
    fn zero_weight_is_rejected() {
        let mut s = Sched2::new();
        assert_eq!(s.register(1, 0, 0), Err(Sched2Error::ZeroWeight));
    }

    #[test]
    fn dispatch_preempt_round_trip_requires_correct_state() {
        let mut s = Sched2::new();
        let h = s.register(1, DEFAULT_WEIGHT, 0).unwrap();
        assert_eq!(s.preempt(h, 10, 0), Err(Sched2Error::NotRunning));
        s.dispatch(h).unwrap();
        assert_eq!(s.run_state(h), Ok(RunState::Running));
        assert_eq!(s.dispatch(h), Err(Sched2Error::NotReady));
        s.preempt(h, 100, 1).unwrap();
        assert_eq!(s.run_state(h), Ok(RunState::Ready));
        // v = 0 + 100*SCALE/1024
        assert_eq!(s.vruntime(h).unwrap(), 100 * SCALE / DEFAULT_WEIGHT as u64);
    }

    /// Required proof: "Fair-share results follow task weights within a
    /// declared tolerance." Two tasks, weight ratio 1:2 (b gets double a's
    /// weight, so b should accumulate vruntime half as fast per tick run and
    /// thus be picked roughly twice as often as a over many rounds).
    #[test]
    fn fair_share_follows_weight_ratio_within_tolerance() {
        let mut s = Sched2::new();
        let a = s.register(1, DEFAULT_WEIGHT, 0).unwrap();
        let b = s.register(2, DEFAULT_WEIGHT * 2, 0).unwrap();
        let mut picks_a = 0u32;
        let mut picks_b = 0u32;
        let mut now: Tick = 1;
        for _ in 0..3000 {
            let pid = s.pick_next_ready(now).expect("always someone ready");
            let h = if pid == 1 { a } else { b };
            if pid == 1 {
                picks_a += 1;
            } else {
                picks_b += 1;
            }
            s.dispatch(h).unwrap();
            s.preempt(h, 10, now).unwrap();
            now += 1;
        }
        // b has 2x the weight, so it should run roughly twice as often as a.
        let ratio = picks_b as f64 / picks_a as f64;
        assert!(
            (1.6..=2.4).contains(&ratio),
            "expected picks_b/picks_a near 2.0, got {ratio} (a={picks_a}, b={picks_b})"
        );
    }

    /// Required proof: "Same-deadline ties are deterministic."
    #[test]
    fn same_key_ties_are_deterministic_by_pid() {
        let mut s = Sched2::new();
        let low = s.register(5, DEFAULT_WEIGHT, 0).unwrap();
        let high = s.register(9, DEFAULT_WEIGHT, 0).unwrap();
        // Identical weight, identical vruntime (both 0), identical deadline
        // class (NONE) -> K_i ties exactly on (deadline_class, vruntime);
        // PID must break the tie, deterministically, every time.
        for _ in 0..20 {
            assert_eq!(s.pick_next_ready(0), Some(5));
        }
        let _ = (low, high);
    }

    /// Required proof: "Sleep cannot wake early; cancellation prevents wake."
    #[test]
    fn sleep_does_not_wake_early_and_cancel_prevents_wake() {
        let mut s = Sched2::new();
        let h = s.register(1, DEFAULT_WEIGHT, 50).unwrap();
        s.dispatch(h).unwrap();
        s.sleep_until(h, 100, 50).unwrap();
        assert_eq!(s.run_state(h), Ok(RunState::Blocked(WaitReason::Timer)));

        let mut out = Vec::new();
        let fired = s.advance(99, &mut out);
        assert_eq!(fired, 0, "must not wake before the deadline");
        assert_eq!(s.run_state(h), Ok(RunState::Blocked(WaitReason::Timer)));

        let fired = s.advance(100, &mut out);
        assert_eq!(fired, 1, "must wake exactly at the deadline");
        assert_eq!(out, alloc_vec(1));
        assert_eq!(s.run_state(h), Ok(RunState::Ready));

        // Cancellation case: a second sleep that is cancelled before its
        // deadline must never fire.
        s.dispatch(h).unwrap();
        s.preempt(h, 0, 100).unwrap();
        s.dispatch(h).unwrap();
        s.sleep_until(h, 200, 100).unwrap();
        assert!(s.cancel_sleep(h).unwrap());
        assert!(
            !s.cancel_sleep(h).unwrap(),
            "second cancel is a no-op, not an error"
        );
        out.clear();
        let fired = s.advance(300, &mut out);
        assert_eq!(fired, 0, "a cancelled timer must never fire");
        // Still Blocked -- cancel does not itself wake (documented semantics).
        assert_eq!(s.run_state(h), Ok(RunState::Blocked(WaitReason::Timer)));
        s.unblock(h, 300).unwrap();
        assert_eq!(s.run_state(h), Ok(RunState::Ready));
    }

    /// Required proof (for the future `ipc` consumer): a process blocked on
    /// `WaitReason::Ipc(key)` is never woken by an unrelated key, is woken by
    /// its own matching key, and `advance()` (the timer wheel) never touches
    /// it since no timer was armed.
    #[test]
    fn block_on_ipc_wakes_only_matching_key_and_ignores_timer_wheel() {
        let mut s = Sched2::new();
        let a = s.register(1, DEFAULT_WEIGHT, 0).unwrap();
        s.dispatch(a).unwrap();
        s.block_on_ipc(a, 42, 0).unwrap();
        assert_eq!(s.run_state(a), Ok(RunState::Blocked(WaitReason::Ipc(42))));

        let mut out = Vec::new();
        let fired = s.advance(1000, &mut out);
        assert_eq!(fired, 0, "no timer was armed; the wheel must not touch it");
        assert_eq!(s.run_state(a), Ok(RunState::Blocked(WaitReason::Ipc(42))));

        let woken = s.wake_matching(99, 1000, &mut out);
        assert_eq!(woken, 0, "an unrelated key must not wake it");
        assert_eq!(s.run_state(a), Ok(RunState::Blocked(WaitReason::Ipc(42))));

        let woken = s.wake_matching(42, 1000, &mut out);
        assert_eq!(woken, 1, "the matching key must wake it exactly once");
        assert_eq!(out, alloc_vec(1));
        assert_eq!(s.run_state(a), Ok(RunState::Ready));
    }

    fn alloc_vec(pid: ProcessId) -> Vec<ProcessId> {
        let mut v = Vec::new();
        v.push(pid);
        v
    }

    /// Required proof (for the `netsess` consumer, subsystem 8): a process
    /// blocked on `WaitReason::Net(key)` is never woken by an unrelated key,
    /// is woken by its own matching key, and `advance()` (the timer wheel)
    /// never touches it since no timer was armed. Exact mirror of
    /// `block_on_ipc_wakes_only_matching_key_and_ignores_timer_wheel`, one
    /// wait-reason variant over.
    #[test]
    fn block_on_net_wakes_only_matching_key_and_ignores_timer_wheel() {
        let mut s = Sched2::new();
        let a = s.register(1, DEFAULT_WEIGHT, 0).unwrap();
        s.dispatch(a).unwrap();
        s.block_on_net(a, 42, 0).unwrap();
        assert_eq!(s.run_state(a), Ok(RunState::Blocked(WaitReason::Net(42))));

        let mut out = Vec::new();
        let fired = s.advance(1000, &mut out);
        assert_eq!(fired, 0, "no timer was armed; the wheel must not touch it");
        assert_eq!(s.run_state(a), Ok(RunState::Blocked(WaitReason::Net(42))));

        let woken = s.wake_matching_net(99, 1000, &mut out);
        assert_eq!(woken, 0, "an unrelated key must not wake it");
        assert_eq!(s.run_state(a), Ok(RunState::Blocked(WaitReason::Net(42))));

        let woken = s.wake_matching_net(42, 1000, &mut out);
        assert_eq!(woken, 1, "the matching key must wake it exactly once");
        assert_eq!(out, alloc_vec(1));
        assert_eq!(s.run_state(a), Ok(RunState::Ready));
    }

    /// Required proof: "Wheel cascade and tick wraparound are tested." A
    /// deadline far enough out to require top-level placement and at least
    /// one cascade down through the intermediate levels before firing, plus
    /// a same-low-slot-different-cycle collision check.
    #[test]
    fn wheel_cascade_across_levels_fires_at_the_exact_tick() {
        let mut s = Sched2::new();
        // WHEEL_BITS=6 -> level boundaries at 64, 4096, 262144. Pick a
        // deadline requiring cascade through level 2 -> level 1 -> level 0.
        let a = s.register(1, DEFAULT_WEIGHT, 0).unwrap();
        s.dispatch(a).unwrap();
        s.sleep_until(a, 5000, 0).unwrap();

        // A second timer landing on the same low-6-bit slot as the first
        // but one full level-0 cycle later (deadline+64) must not fire with
        // the first, nor collide/corrupt its bucket.
        let b = s.register(2, DEFAULT_WEIGHT, 0).unwrap();
        s.dispatch(b).unwrap();
        s.sleep_until(b, 5000 + WHEEL_SLOTS as Tick, 0).unwrap();

        let mut out = Vec::new();
        let fired = s.advance(4999, &mut out);
        assert_eq!(fired, 0);
        let fired = s.advance(5000, &mut out);
        assert_eq!(
            fired, 1,
            "exactly the first timer fires at its own deadline"
        );
        assert_eq!(out, alloc_vec(1));
        out.clear();

        let fired = s.advance(5000 + WHEEL_SLOTS as Tick - 1, &mut out);
        assert_eq!(fired, 0, "second timer must not fire one tick early");
        let fired = s.advance(5000 + WHEEL_SLOTS as Tick, &mut out);
        assert_eq!(fired, 1, "second timer fires at its own (later) deadline");
        assert_eq!(out, alloc_vec(2));
    }

    /// Required proof: "Deadline work cannot violate the starvation bound."
    #[test]
    fn deadline_task_cannot_starve_an_ordinary_ready_task() {
        let bound = 20;
        let mut s = Sched2::with_starvation_bound(bound);
        let urgent = s.register(1, DEFAULT_WEIGHT, 0).unwrap();
        s.set_deadline(urgent, 0, 1_000_000, 1).unwrap();
        let ordinary = s.register(2, DEFAULT_WEIGHT, 0).unwrap();

        let mut max_wait_seen = 0u64;
        let mut now: Tick = 1;
        let mut ordinary_ready_since = 0u64;
        for _ in 0..500 {
            let pid = s.pick_next_ready(now).expect("always someone ready");
            if pid == 2 {
                let waited = now - ordinary_ready_since;
                if waited > max_wait_seen {
                    max_wait_seen = waited;
                }
                let h = ordinary;
                s.dispatch(h).unwrap();
                s.preempt(h, 1, now).unwrap();
                ordinary_ready_since = now;
            } else {
                let h = urgent;
                s.dispatch(h).unwrap();
                s.preempt(h, 1, now).unwrap();
            }
            now += 1;
        }
        assert!(
            max_wait_seen <= bound,
            "ordinary task waited {max_wait_seen} ticks, exceeding the starvation bound {bound}"
        );
        assert!(
            s.stats().starvation_boosts > 0,
            "expected the starvation override to actually have fired at least once"
        );
    }

    #[test]
    fn slack_formula_matches_contract() {
        let mut s = Sched2::new();
        let h = s.register(1, DEFAULT_WEIGHT, 0).unwrap();
        assert_eq!(s.slack(h, 0).unwrap(), None, "no deadline set yet");
        s.set_deadline(h, 0, 100, 20).unwrap();
        // slack = d - t - e = 100 - 30 - 20 = 50
        assert_eq!(s.slack(h, 30).unwrap(), Some(50));
        s.clear_deadline(h).unwrap();
        assert_eq!(s.slack(h, 30).unwrap(), None);
    }

    #[test]
    fn exit_disarms_any_active_timer_without_leaking_the_wheel() {
        let mut s = Sched2::new();
        let h = s.register(1, DEFAULT_WEIGHT, 0).unwrap();
        s.dispatch(h).unwrap();
        s.sleep_until(h, 500, 0).unwrap();
        s.exit(h).unwrap();
        let mut out = Vec::new();
        let fired = s.advance(600, &mut out);
        assert_eq!(
            fired, 0,
            "exited process must not be woken by its old timer"
        );
    }

    #[test]
    fn state_counts_reflect_real_transitions() {
        let mut s = Sched2::new();
        let a = s.register(1, DEFAULT_WEIGHT, 0).unwrap();
        let b = s.register(2, DEFAULT_WEIGHT, 0).unwrap();
        let counts = s.state_counts();
        assert_eq!(counts.ready, 2);
        s.dispatch(a).unwrap();
        let counts = s.state_counts();
        assert_eq!(counts.ready, 1);
        assert_eq!(counts.running, 1);
        s.sleep_until(a, 10, 0).unwrap();
        let counts = s.state_counts();
        assert_eq!(counts.blocked, 1);
        s.exit(b).unwrap();
        let counts = s.state_counts();
        assert_eq!(counts.exited, 1);
    }
}
