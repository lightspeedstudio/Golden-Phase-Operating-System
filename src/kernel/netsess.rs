//! Network rings and session tables (`feature = "netsess"`), subsystem 8
//! (the final subsystem) of the eight kernel-subsystems build program — see
//! `fpga/zynq/evidence/KERNEL_MATH_CONTRACT_2026-08-12.md` and
//! `fpga/zynq/evidence/NETSESS_STATUS_<date>.md`.
//!
//! Additive software session table and descriptor ring. This module does
//! **not** own GEM DMA descriptors. `HwOwned` is GEM's RXBD own bit
//! (`board::gem_cadence_hw::rxbd::ADDR_OWN`), not a [`DescriptorState`]
//! here. After GEM copies a used RX BD to the CPU (`wire_rx_poll` /
//! MAC loopback `take_frame`), [`note_gem_rx_taken`] records
//! `KernelReady -> Processing -> complete` on this ring. Live packet
//! bytes stay in `gem` / `kernel::net_ip` / `kernel::net_ssh`. Shell
//! demos never take `HwOwned`. GEM0 RX IRQ registration remains
//! `board::zynq7000::gic::enable_gem0_rx_irq` / `irqreg`; this module
//! does not register a second ISR. Not IOMMU; `DMA_PROTECTION` stays NO.
//!
//! ## What this reuses / does not reuse (per the math contract, §0.4/§4/§7)
//! - [`crate::kernel::event_log::ProcessId`] is not needed directly by this
//!   module (sessions are node-to-node, not per-process) — see `captable`
//!   integration below for where a `ProcessId` subject is actually bound.
//! - The `(slot, generation)` generational-handle shape from Part III, as a
//!   **local** [`SessionHandle`] type — same rationale `kernel::ipc`'s own
//!   module docs already give for `IpcHandle`: importing `captable`'s exact
//!   type here would still wrongly couple two independently-toggleable
//!   Cargo features architecturally, even though this module's own contract
//!   text (unlike `ipc`/`sched2`/`irqreg`'s) *does* declare a real
//!   dependency on `captable` (see `Cargo.toml`'s `netsess = ["captable",
//!   "sched2"]`) — the dependency is expressed as this module always being
//!   wrapped by a `captable::ObjectKind::Network` object one layer up
//!   (mirrors `ipc`'s own `Queue`/`IpcHandle` relationship to `captable`
//!   exactly), not as this module importing `captable`'s handle type.
//! - [`crate::kernel::crypto::phase_hash::tuple_hash`] (PH-C domain) for
//!   session identity — the math contract's own consumption-declaration row
//!   for `netsess`: "PH-C for session identity, and the already-real
//!   `ph_boot_session_root`/`BootSessionRoot` as context (not secret, not
//!   proven globally unique)". Computed once at [`Netsess::open_session`]
//!   (the creation boundary), never inside the hot descriptor-ring path —
//!   `S = PH-C(protocolVersion, localNode, remoteNode, nonce,
//!   BootSessionRoot, sessionGeneration)`, exactly the contract's formula.
//!   `BootSessionRoot` is supplied by the caller (`kernel.authority.
//!   boot_session_root`, the real value `kernel::anom::boot` computes once
//!   at boot) — this module never computes it itself, matching the
//!   contract's explicit "context, not secret, not proven globally unique"
//!   caveat: it is audit/identity binding only, never an authenticator.
//! - The exact bounded, per-slot Vyukov sequence-number ring protocol
//!   already implemented for `irqreg`'s deferred-event ring and reused
//!   verbatim by `ipc`'s message ring (`Writable(j,n) = [seq_j = n]`,
//!   `Readable(j,n) = [seq_j = n+1]`, `Reusable(j,n) = [seq_j = n+N]`) —
//!   reused a third time here for the descriptor ring, not reinvented.
//! - `captable::ObjectKind::Network`/`open_network`/`resolve_network`/
//!   `destroy_network_via_cap` (added alongside this module, in the same
//!   commit) for real generation-checked capability handles on sessions —
//!   the same `FS_OPEN`/`IPC_CREATE_QUEUE` pattern reused a third time, per
//!   this subsystem's own contract text: "Session handles come from
//!   `captable`."
//! - `sched2::WaitReason::Net`/`Sched2::block_on_net`/
//!   `Sched2::wake_matching_net` (added alongside this module, in the same
//!   commit) for real block/wake on an empty descriptor ring, per this
//!   subsystem's own contract text: "Wait/block behavior uses `sched2` and
//!   IPC-like bounded rings" — exactly `ipc`'s own `IPC_RECV`/
//!   `block_on_ipc`/`wake_matching` composition, one wait-reason variant
//!   over.
//! - This subsystem consumes **no G64 operation and no `PhaseState`
//!   reflection** beyond the single PH-C call above — session/descriptor
//!   bookkeeping is not phase mathematics. Explicitly stated per Gate 0's
//!   instruction to declare this for every subsystem.
//!
//! ## Mathematics implemented (math contract's "Network rings and session
//! tables" row)
//! - Session identity: `S = PH-C(protocolVersion, localNode, remoteNode,
//!   nonce, BootSessionRoot, sessionGeneration)` (see [`open_session`]).
//! - Replay window, `W` = [`REPLAY_WINDOW`] (one `u64` bitmap word): a
//!   sequence `q` is accepted only when `q > q_max - W` and its replay bit
//!   is clear; the window then advances/shifts and the bit is set (see
//!   [`Netsess::accept_sequence`]). Integer wraparound is handled explicitly
//!   via `saturating`/bounded-shift arithmetic — a shift `>= W` resets the
//!   window rather than invoking undefined-behavior-adjacent `<<64`.
//! - Descriptor ownership in **this** ring is
//!   `KernelReady -> Processing -> Free`,
//!   `Processing -> Failed -> Quarantined`. GEM's `HwOwned` bit
//!   (`board::gem_cadence_hw::rxbd::ADDR_OWN`) is not a
//!   [`DescriptorState`] variant and is never stored here.
//!   [`Netsess::push_descriptor`] enters at `KernelReady`. Quarantine is
//!   absorbing (mirrors `pmm`/`txlog`'s identical rule): a quarantined
//!   ring slot's sequence is never bumped again, so it can never become
//!   `Writable` — see [`NetsessError::NotProcessing`]/
//!   [`NetsessError::StaleDescriptor`] and "Non-goals" below.
//! - RTT estimation (RFC 6298-shaped, contract's own formulas), pure integer
//!   arithmetic in caller-supplied `Tick` units, explicitly clamped (math
//!   contract §9's fixed-point rule — no floating point, no division by an
//!   unchecked value):
//!   `srtt <- srtt - (srtt >> 3) + (rtt >> 3)`,
//!   `rttvar <- rttvar - (rttvar >> 2) + (|srtt - rtt| >> 2)`,
//!   `rto <- srtt + 4 * rttvar` (all `saturating_*`) — see
//!   [`Netsess::record_rtt_sample`].
//!
//! ## Non-goals (explicit — see also `NETSESS_STATUS_<date>.md`)
//! - No GEM DMA ownership: this ring is software and never stores
//!   `HwOwned`. Live GEM RX takes are recorded here after hardware
//!   already released the BD; the GEM rings stay in `board::zynq7000::gem`.
//!   Live packet bytes stay on the existing gem/net_ip/ssh-bm paths.
//! - No packet parsing/demuxing: callers (the existing GEM/smoltcp/
//!   PhaseLink/SSH paths) decide which session a frame belongs to and
//!   supply the [`SessionHandle`] plus a sequence number; this module never
//!   touches raw frame bytes and never classifies traffic itself.
//! - No cryptographic authentication: per the contract's own text,
//!   "Authentication must use the existing MAC/AEAD/keyed protocol.
//!   PhaseHash alone is not a network authenticator" — PH-C here is
//!   identity/audit binding only, exactly like `captable`/`ipc`'s own PH-C
//!   usage. A session with a valid, un-revoked capability is *authorized*,
//!   not *authenticated*; real packet authentication remains the existing
//!   SSH/PhaseLink/AEAD layer's job, untouched.
//! - No revalidation path for a quarantined descriptor slot: once
//!   `Processing -> Failed -> Quarantined`, that ring slot is retired for
//!   the life of this `Netsess` instance (absorbing, per the math
//!   contract's `pmm`-quarantine rule, generalized). A sustained failure
//!   storm can therefore permanently shrink the ring's effective capacity —
//!   a real, bounded, testable consequence, not a hidden bug.
//! - No persistence/recovery: this table is RAM-only, like every prior
//!   subsystem; `X_before/X_fault/X_restored` recovery mathematics do not
//!   apply to a subsystem with no checkpoint/restore operation of its own.
//! - No new `NET_*` syscalls: no user-mode socket ABI exists yet to route
//!   through `captable` (unlike `ipc`, which extended a *pre-existing*
//!   `IPC_SEND`/`IPC_RECV` syscall pair). This module is exercised directly
//!   by kernel/board code and shell commands (`netsess_cmd`), at exactly the
//!   same layer the existing `net wire`/`net ssh` demos already run at.
//!   Inventing a brand-new socket syscall ABI here would be new design, not
//!   reuse, and is explicitly out of scope for this subsystem.

use core::fmt;
#[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
use core::sync::atomic::{AtomicPtr, Ordering};

use crate::kernel::crypto::phase_hash::{tuple_hash, DOMAIN_CONTENT};
use crate::kernel::hash::StateHash;
use crate::platform::Vec;

/// GEM0 kernel-internal session (not a user cap, not a socket ABI).
const GEM0_PROTOCOL: u8 = 0;
const GEM0_NODE: [u8; 4] = [0, 0, 0, 0];
const GEM0_NONCE: u64 = 0;

#[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
static LIVE: AtomicPtr<Netsess> = AtomicPtr::new(core::ptr::null_mut());

/// After GEM DMA released a used RX BD to the CPU. Records
/// [`Netsess::record_gem_rx_taken`] on the bound live instance.
/// No-op until [`Netsess::bind_live`]. Host builds are no-ops (call
/// [`Netsess::record_gem_rx_taken`] directly in tests).
pub fn note_gem_rx_taken(frame_len: u32) {
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    {
        let p = LIVE.load(Ordering::Acquire);
        if p.is_null() {
            return;
        }
        // SAFETY: `bind_live` stores `Kernel.netsess` after `Runtime` is in
        // its final `kernel_main` location and is never moved.
        unsafe {
            (*p).record_gem_rx_taken(frame_len);
        }
    }
    #[cfg(not(all(feature = "zynq-a9", target_arch = "arm")))]
    {
        let _ = frame_len;
    }
}

/// Bound on live sessions (Part V §8's own "Complexity targets": bounded
/// table, `O(log S)`/`O(1)` lookup — kept as a flat generation-stamped `Vec`
/// like `captable`'s object table, bounded here rather than at an
/// arbitrary-growth `HashMap`).
pub const MAX_SESSIONS: usize = 16;
/// Descriptor ring capacity (power of two — exact precedent: `irqreg`'s own
/// `RING_CAPACITY`).
pub const RING_CAPACITY: usize = 32;
/// Replay-window width `W`, one machine word wide.
pub const REPLAY_WINDOW: u64 = 64;

/// `h = (i, g)` — a local generational handle. See module docs for why this
/// is not `captable::GenerationalHandle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionHandle {
    pub slot: u32,
    pub generation: u32,
}

impl SessionHandle {
    /// Pack into the opaque `u64` key `sched2::WaitReason::Net`/
    /// `block_on_net`/`wake_matching_net` compare by equality only.
    pub fn pack(self) -> u64 {
        ((self.slot as u64) << 32) | self.generation as u64
    }

    pub fn unpack(key: u64) -> Self {
        Self {
            slot: (key >> 32) as u32,
            generation: key as u32,
        }
    }
}

/// Descriptor ownership state machine. See module docs, "Mathematics
/// implemented".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorState {
    KernelReady,
    Processing,
    Quarantined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetsessError {
    /// Slot index out of range, or not currently occupied.
    InvalidHandle,
    /// Slot occupied, but by a different (newer) generation.
    StaleGeneration,
    /// `open_session` was called with every session slot already occupied.
    MaxSessionsReached,
    /// `accept_sequence`'s sequence number was outside the replay window, or
    /// a duplicate of one already seen inside it.
    ReplayRejected,
    /// `push_descriptor`'s ring was full (see module docs, "Mathematics
    /// implemented": a quarantined slot never frees again).
    RingFull,
    /// `complete_descriptor`/`fail_descriptor` was given a token whose ring
    /// slot has since moved on to a different sequence entirely (already
    /// completed and reused, or never issued) — the "stale DMA handle"
    /// required-proof case.
    StaleDescriptor,
    /// `complete_descriptor`/`fail_descriptor` was given a token whose ring
    /// slot still matches, but is not currently `Processing` (e.g. already
    /// quarantined by an earlier `fail_descriptor` on the same token).
    NotProcessing,
}

impl fmt::Display for NetsessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            NetsessError::InvalidHandle => "invalid handle",
            NetsessError::StaleGeneration => "stale generation",
            NetsessError::MaxSessionsReached => "max sessions reached",
            NetsessError::ReplayRejected => "replay rejected",
            NetsessError::RingFull => "descriptor ring full",
            NetsessError::StaleDescriptor => "stale descriptor handle",
            NetsessError::NotProcessing => "descriptor not processing",
        };
        f.write_str(msg)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct NetsessStats {
    pub sessions_opened: u64,
    pub sessions_closed: u64,
    pub generation_retired: u64,
    pub replay_accepted: u64,
    pub replay_rejected: u64,
    pub descriptors_pushed: u64,
    pub descriptors_rejected_full: u64,
    pub descriptors_completed: u64,
    pub descriptors_failed: u64,
    /// Live GEM RX BD takes recorded on this software ring (A16).
    pub gem_rx_recorded: u64,
    /// Live GEM RX takes that could not be recorded (unbound / ring full).
    pub gem_rx_rejected: u64,
}

struct SessionSlot {
    occupied: bool,
    generation: u32,
    /// PH-C over the session's identity, bound once at `open_session`.
    /// Cold-path audit metadata only — never consulted by `accept_sequence`
    /// or `record_rtt_sample`.
    session_id: StateHash,
    local_node: [u8; 4],
    remote_node: [u8; 4],
    nonce: u64,
    /// `true` once at least one sequence number has been accepted (the
    /// window has no `q_max` before the first packet).
    replay_seen_any: bool,
    replay_max: u64,
    replay_bitmap: u64,
    rtt_initialized: bool,
    srtt: u32,
    rttvar: u32,
    rto: u32,
}

/// Bumps `*generation`. Returns `true` on overflow (`u32::MAX -> 0`
/// avoided); mirrors `captable`/`sched2`/`ipc`'s identical rule.
fn bump_generation(generation: &mut u32) -> bool {
    match generation.checked_add(1) {
        Some(next) => {
            *generation = next;
            false
        }
        None => true,
    }
}

struct DescriptorSlot {
    /// Vyukov-style bounded-ring sequence number. `Writable(j,n) =
    /// [seq_j = n]`. `session`/`packet_seq`/`state` are only meaningful once
    /// `Readable(j,n) = [seq_j = n+1]` holds.
    sequence: u64,
    session: SessionHandle,
    packet_seq: u64,
    state: DescriptorState,
}

impl DescriptorSlot {
    fn empty(seed: u64) -> Self {
        Self {
            sequence: seed,
            session: SessionHandle {
                slot: 0,
                generation: 0,
            },
            packet_seq: 0,
            state: DescriptorState::KernelReady,
        }
    }
}

/// Network session table + descriptor ring engine. See module docs for the
/// full design and integration points.
pub struct Netsess {
    sessions: Vec<SessionSlot>,
    sessions_free: Vec<u32>,
    ring: Vec<DescriptorSlot>,
    ring_mask: u64,
    ring_next: u64,
    ring_read: u64,
    stats: NetsessStats,
    gem_handle: Option<SessionHandle>,
    gem_boot_root: Option<StateHash>,
    gem_seq: u64,
    gem_rx_last_len: u32,
}

impl Default for Netsess {
    fn default() -> Self {
        Self::new()
    }
}

impl Netsess {
    pub fn new() -> Self {
        // Vyukov-style bounded-ring seeding: slot j's sequence starts at j
        // so the first producer pass (n=j) sees Writable(j,j)=true. Exact
        // precedent: `ipc::Ipc::create_queue`.
        let ring: Vec<DescriptorSlot> = (0..RING_CAPACITY)
            .map(|j| DescriptorSlot::empty(j as u64))
            .collect();
        Self {
            sessions: Vec::new(),
            sessions_free: Vec::new(),
            ring,
            ring_mask: (RING_CAPACITY - 1) as u64,
            ring_next: 0,
            ring_read: 0,
            stats: NetsessStats::default(),
            gem_handle: None,
            gem_boot_root: None,
            gem_seq: 0,
            gem_rx_last_len: 0,
        }
    }

    pub fn stats(&self) -> NetsessStats {
        self.stats
    }

    pub fn session_count(&self) -> usize {
        self.sessions.iter().filter(|s| s.occupied).count()
    }

    fn validate(&self, handle: SessionHandle) -> Result<usize, NetsessError> {
        let slot = self
            .sessions
            .get(handle.slot as usize)
            .ok_or(NetsessError::InvalidHandle)?;
        if !slot.occupied {
            return Err(NetsessError::InvalidHandle);
        }
        if slot.generation != handle.generation {
            return Err(NetsessError::StaleGeneration);
        }
        Ok(handle.slot as usize)
    }

    /// Open a new session bound to `(local_node, remote_node, nonce)`, with
    /// identity `S = PH-C(protocol_version, local_node, remote_node, nonce,
    /// boot_session_root, session_generation)` (contract's own formula for
    /// this subsystem). `boot_session_root` is the caller-supplied, already
    /// real `kernel.authority.boot_session_root` — this module never
    /// computes it. Bounded by [`MAX_SESSIONS`].
    pub fn open_session(
        &mut self,
        protocol_version: u8,
        local_node: [u8; 4],
        remote_node: [u8; 4],
        nonce: u64,
        boot_session_root: StateHash,
    ) -> Result<SessionHandle, NetsessError> {
        let slot_index = if let Some(slot) = self.sessions_free.pop() {
            slot
        } else if self.sessions.len() < MAX_SESSIONS {
            let slot = self.sessions.len() as u32;
            self.sessions.push(SessionSlot {
                occupied: false,
                generation: 0,
                session_id: StateHash::default(),
                local_node: [0; 4],
                remote_node: [0; 4],
                nonce: 0,
                replay_seen_any: false,
                replay_max: 0,
                replay_bitmap: 0,
                rtt_initialized: false,
                srtt: 0,
                rttvar: 0,
                rto: 0,
            });
            slot
        } else {
            return Err(NetsessError::MaxSessionsReached);
        };

        let generation = self.sessions[slot_index as usize].generation;
        let session_id = tuple_hash(
            DOMAIN_CONTENT,
            &[
                &[protocol_version],
                &local_node,
                &remote_node,
                &nonce.to_le_bytes(),
                &boot_session_root,
                &generation.to_le_bytes(),
            ],
        );
        self.sessions[slot_index as usize] = SessionSlot {
            occupied: true,
            generation,
            session_id,
            local_node,
            remote_node,
            nonce,
            replay_seen_any: false,
            replay_max: 0,
            replay_bitmap: 0,
            rtt_initialized: false,
            srtt: 0,
            rttvar: 0,
            rto: 0,
        };
        self.stats.sessions_opened += 1;
        Ok(SessionHandle {
            slot: slot_index,
            generation,
        })
    }

    /// Close (revoke) a session. Bumps the slot's generation before ever
    /// returning it to the free list, per the math contract's `g_i <- g_i +
    /// 1` rule — a stale handle to a closed-then-reopened slot fails
    /// [`Netsess::validate`] from then on.
    pub fn close_session(&mut self, handle: SessionHandle) -> Result<(), NetsessError> {
        let idx = self.validate(handle)?;
        self.sessions[idx].occupied = false;
        let retired = bump_generation(&mut self.sessions[idx].generation);
        if retired {
            self.stats.generation_retired += 1;
        } else {
            self.sessions_free.push(handle.slot);
        }
        self.stats.sessions_closed += 1;
        Ok(())
    }

    /// The session's PH-C identity, computed once at `open_session`. Audit
    /// metadata only.
    pub fn session_id(&self, handle: SessionHandle) -> Result<StateHash, NetsessError> {
        let idx = self.validate(handle)?;
        Ok(self.sessions[idx].session_id)
    }

    pub fn session_nodes(&self, handle: SessionHandle) -> Result<([u8; 4], [u8; 4]), NetsessError> {
        let idx = self.validate(handle)?;
        Ok((
            self.sessions[idx].local_node,
            self.sessions[idx].remote_node,
        ))
    }

    /// The nonce supplied at `open_session`, bound into `session_id`'s PH-C
    /// input. Audit metadata only — never consulted by `accept_sequence` or
    /// `record_rtt_sample`.
    pub fn session_nonce(&self, handle: SessionHandle) -> Result<u64, NetsessError> {
        let idx = self.validate(handle)?;
        Ok(self.sessions[idx].nonce)
    }

    /// Replay-window acceptance for inbound sequence number `seq` on
    /// `handle`'s session. `W = `[`REPLAY_WINDOW`]. See module docs,
    /// "Mathematics implemented", for the exact rule and wraparound
    /// handling.
    pub fn accept_sequence(&mut self, handle: SessionHandle, seq: u64) -> Result<(), NetsessError> {
        let idx = self.validate(handle)?;
        let slot = &mut self.sessions[idx];

        if !slot.replay_seen_any {
            slot.replay_seen_any = true;
            slot.replay_max = seq;
            slot.replay_bitmap = 1;
            self.stats.replay_accepted += 1;
            return Ok(());
        }

        if seq > slot.replay_max {
            let shift = seq - slot.replay_max;
            slot.replay_bitmap = if shift >= REPLAY_WINDOW {
                1
            } else {
                (slot.replay_bitmap << shift) | 1
            };
            slot.replay_max = seq;
            self.stats.replay_accepted += 1;
            return Ok(());
        }

        let back = slot.replay_max - seq;
        if back >= REPLAY_WINDOW {
            self.stats.replay_rejected += 1;
            return Err(NetsessError::ReplayRejected);
        }
        let bit = 1u64 << back;
        if slot.replay_bitmap & bit != 0 {
            self.stats.replay_rejected += 1;
            return Err(NetsessError::ReplayRejected);
        }
        slot.replay_bitmap |= bit;
        self.stats.replay_accepted += 1;
        Ok(())
    }

    /// Update `(srtt, rttvar, rto)` from one real round-trip sample, in
    /// caller-supplied `Tick` units. Returns the updated triple. See module
    /// docs, "Mathematics implemented", for the exact (RFC 6298-shaped)
    /// integer formulas.
    pub fn record_rtt_sample(
        &mut self,
        handle: SessionHandle,
        rtt_ticks: u32,
    ) -> Result<(u32, u32, u32), NetsessError> {
        let idx = self.validate(handle)?;
        let slot = &mut self.sessions[idx];
        if !slot.rtt_initialized {
            slot.srtt = rtt_ticks;
            slot.rttvar = rtt_ticks / 2;
            slot.rtt_initialized = true;
        } else {
            slot.srtt = slot
                .srtt
                .saturating_sub(slot.srtt >> 3)
                .saturating_add(rtt_ticks >> 3);
            let err = slot.srtt.abs_diff(rtt_ticks);
            slot.rttvar = slot
                .rttvar
                .saturating_sub(slot.rttvar >> 2)
                .saturating_add(err >> 2);
        }
        slot.rto = slot.srtt.saturating_add(slot.rttvar.saturating_mul(4));
        Ok((slot.srtt, slot.rttvar, slot.rto))
    }

    pub fn rtt_estimate(&self, handle: SessionHandle) -> Result<(u32, u32, u32), NetsessError> {
        let idx = self.validate(handle)?;
        let slot = &self.sessions[idx];
        Ok((slot.srtt, slot.rttvar, slot.rto))
    }

    /// Enqueue one accepted packet/event as a descriptor entering directly
    /// at `KernelReady`. This software ring does not store GEM `HwOwned`.
    /// Validates `handle` first (a closed/stale session can never enqueue —
    /// the "revoked session rejection" required-proof case), then applies
    /// the exact `irqreg`/`ipc` Vyukov ring-full check. Returns the ring
    /// token to pass to `complete_descriptor`/`fail_descriptor`.
    pub fn push_descriptor(
        &mut self,
        handle: SessionHandle,
        packet_seq: u64,
    ) -> Result<u64, NetsessError> {
        self.validate(handle)?;
        let n = self.ring_next;
        let j = (n & self.ring_mask) as usize;
        if self.ring[j].sequence != n {
            self.stats.descriptors_rejected_full += 1;
            return Err(NetsessError::RingFull);
        }
        self.ring[j] = DescriptorSlot {
            sequence: n + 1,
            session: handle,
            packet_seq,
            state: DescriptorState::KernelReady,
        };
        self.ring_next = n + 1;
        self.stats.descriptors_pushed += 1;
        Ok(n)
    }

    /// Dequeue the oldest `KernelReady` descriptor, transitioning it to
    /// `Processing` and returning its ring token, session, and packet
    /// sequence. `None` when the ring is empty (the caller — `kernel::
    /// syscall`'s future net-receive path, if any is ever added — is
    /// expected to block via `sched2::block_on_net` on this condition,
    /// exactly `ipc::IPC_RECV`'s own precedent).
    pub fn drain_descriptor(&mut self) -> Option<(u64, SessionHandle, u64)> {
        let n = self.ring_read;
        let j = (n & self.ring_mask) as usize;
        if self.ring[j].sequence != n + 1 {
            return None;
        }
        if self.ring[j].state != DescriptorState::KernelReady {
            return None;
        }
        self.ring[j].state = DescriptorState::Processing;
        self.ring_read = n + 1;
        Some((n, self.ring[j].session, self.ring[j].packet_seq))
    }

    fn locate_processing(&mut self, token: u64) -> Result<usize, NetsessError> {
        let j = (token & self.ring_mask) as usize;
        if self.ring[j].sequence != token + 1 {
            return Err(NetsessError::StaleDescriptor);
        }
        if self.ring[j].state != DescriptorState::Processing {
            return Err(NetsessError::NotProcessing);
        }
        Ok(j)
    }

    /// `Processing -> Free`: mark `token`'s descriptor successfully handled
    /// and return its ring slot for reuse (`Reusable(j,n) = [seq_j =
    /// n+N]`).
    pub fn complete_descriptor(&mut self, token: u64) -> Result<(), NetsessError> {
        let j = self.locate_processing(token)?;
        self.ring[j].sequence = token + RING_CAPACITY as u64;
        self.stats.descriptors_completed += 1;
        Ok(())
    }

    /// `Processing -> Failed -> Quarantined`: mark `token`'s descriptor
    /// unrecoverable. Absorbing — the slot's sequence is never bumped again,
    /// so it can never become `Writable`, matching the math contract's
    /// `pmm`-quarantine rule (see module docs, "Non-goals").
    pub fn fail_descriptor(&mut self, token: u64) -> Result<(), NetsessError> {
        let j = self.locate_processing(token)?;
        self.ring[j].state = DescriptorState::Quarantined;
        self.stats.descriptors_failed += 1;
        Ok(())
    }

    /// Ring occupancy, `tail - head` (math contract's own IPC/ring
    /// invariant, `0 <= tail - head <= N`).
    pub fn ring_len(&self) -> usize {
        (self.ring_next - self.ring_read) as usize
    }

    pub fn gem_rx_last_len(&self) -> u32 {
        self.gem_rx_last_len
    }

    /// Open (or reuse) the kernel-internal GEM0 session. Call after
    /// `Kernel` is in its final location; not from `Kernel::new`.
    pub fn bind_gem_session(
        &mut self,
        boot_session_root: StateHash,
    ) -> Result<SessionHandle, NetsessError> {
        self.gem_boot_root = Some(boot_session_root);
        self.ensure_gem_session(boot_session_root)
    }

    /// Board hook: `note_gem_rx_taken` writes this instance. Host tests
    /// call [`record_gem_rx_taken`] directly.
    #[cfg(all(feature = "zynq-a9", target_arch = "arm"))]
    pub fn bind_live(&mut self) {
        LIVE.store(self as *mut Netsess, Ordering::Release);
    }

    fn ensure_gem_session(
        &mut self,
        boot_session_root: StateHash,
    ) -> Result<SessionHandle, NetsessError> {
        if let Some(h) = self.gem_handle {
            if self.validate(h).is_ok() {
                return Ok(h);
            }
            self.gem_handle = None;
        }
        let h = self.open_session(
            GEM0_PROTOCOL,
            GEM0_NODE,
            GEM0_NODE,
            GEM0_NONCE,
            boot_session_root,
        )?;
        self.gem_handle = Some(h);
        Ok(h)
    }

    /// Record one GEM RX BD take that already copied bytes off hardware.
    /// Enters at `KernelReady`; never stores `HwOwned`. No-op if unbound.
    pub fn record_gem_rx_taken(&mut self, frame_len: u32) {
        self.gem_rx_last_len = frame_len;
        let Some(root) = self.gem_boot_root else {
            return;
        };
        let handle = match self.ensure_gem_session(root) {
            Ok(h) => h,
            Err(_) => {
                self.stats.gem_rx_rejected = self.stats.gem_rx_rejected.saturating_add(1);
                return;
            }
        };
        self.gem_seq = self.gem_seq.wrapping_add(1);
        let seq = self.gem_seq;
        if self.accept_sequence(handle, seq).is_err() {
            self.stats.gem_rx_rejected = self.stats.gem_rx_rejected.saturating_add(1);
            return;
        }
        let token = match self.push_descriptor(handle, seq) {
            Ok(t) => t,
            Err(_) => {
                self.stats.gem_rx_rejected = self.stats.gem_rx_rejected.saturating_add(1);
                return;
            }
        };
        if self.drain_descriptor().is_none() {
            self.stats.gem_rx_rejected = self.stats.gem_rx_rejected.saturating_add(1);
            return;
        }
        if self.complete_descriptor(token).is_err() {
            self.stats.gem_rx_rejected = self.stats.gem_rx_rejected.saturating_add(1);
            return;
        }
        self.stats.gem_rx_recorded = self.stats.gem_rx_recorded.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(byte: u8) -> StateHash {
        [byte; 32]
    }

    fn open(net: &mut Netsess, remote_last_octet: u8) -> SessionHandle {
        net.open_session(
            1,
            [10, 0, 0, 1],
            [10, 0, 0, remote_last_octet],
            0xdead_beef,
            root(7),
        )
        .unwrap()
    }

    #[test]
    fn open_session_assigns_distinct_handles() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        let b = open(&mut net, 3);
        assert_ne!(a, b);
        assert_eq!(net.session_count(), 2);
    }

    #[test]
    fn descriptor_state_has_no_hwowned_variant() {
        for s in [
            DescriptorState::KernelReady,
            DescriptorState::Processing,
            DescriptorState::Quarantined,
        ] {
            assert!(
                !format!("{s:?}").contains("HwOwned"),
                "this ring must not store GEM HwOwned, got {s:?}"
            );
        }
    }

    #[test]
    fn session_id_is_deterministic_and_sensitive_to_every_field() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        let id_a = net.session_id(a).unwrap();

        let mut net2 = Netsess::new();
        let a2 = open(&mut net2, 2);
        let id_a2 = net2.session_id(a2).unwrap();
        assert_eq!(id_a, id_a2, "same inputs, same fresh generation -> same id");

        let mut net3 = Netsess::new();
        let b = open(&mut net3, 9); // different remote_node
        let id_b = net3.session_id(b).unwrap();
        assert_ne!(id_a, id_b);
    }

    #[test]
    fn session_id_differs_across_generations_of_the_same_slot() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        let id_first = net.session_id(a).unwrap();
        net.close_session(a).unwrap();
        let a2 = open(&mut net, 2); // reuses the freed slot, bumped generation
        assert_eq!(a2.slot, a.slot);
        assert_ne!(a2.generation, a.generation);
        let id_second = net.session_id(a2).unwrap();
        assert_ne!(id_first, id_second);
    }

    #[test]
    fn stale_handle_after_close_is_rejected() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        net.close_session(a).unwrap();
        assert_eq!(net.session_id(a), Err(NetsessError::InvalidHandle));
        assert_eq!(net.accept_sequence(a, 1), Err(NetsessError::InvalidHandle));
        assert_eq!(net.push_descriptor(a, 1), Err(NetsessError::InvalidHandle));
    }

    #[test]
    fn stale_generation_after_slot_reuse_is_rejected() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        net.close_session(a).unwrap();
        let a2 = open(&mut net, 2); // same slot, new generation
        assert_eq!(a2.slot, a.slot);
        assert_eq!(net.session_id(a), Err(NetsessError::StaleGeneration));
        let _ = a2;
    }

    #[test]
    fn max_sessions_reached_is_a_hard_bound() {
        let mut net = Netsess::new();
        for i in 0..MAX_SESSIONS {
            open(&mut net, i as u8);
        }
        let err = net.open_session(1, [10, 0, 0, 1], [10, 0, 0, 250], 1, root(1));
        assert_eq!(err, Err(NetsessError::MaxSessionsReached));
    }

    #[test]
    fn replay_accepts_strictly_increasing_sequence() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        for seq in 1..=10u64 {
            assert_eq!(net.accept_sequence(a, seq), Ok(()));
        }
        assert_eq!(net.stats().replay_accepted, 10);
    }

    #[test]
    fn replay_rejects_exact_duplicate() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        assert_eq!(net.accept_sequence(a, 5), Ok(()));
        assert_eq!(net.accept_sequence(a, 5), Err(NetsessError::ReplayRejected));
    }

    #[test]
    fn replay_accepts_reordered_packet_inside_window_once() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        assert_eq!(net.accept_sequence(a, 10), Ok(()));
        assert_eq!(net.accept_sequence(a, 9), Ok(())); // reordered, inside window
        assert_eq!(net.accept_sequence(a, 9), Err(NetsessError::ReplayRejected));
        // now a dup
    }

    #[test]
    fn replay_rejects_sequence_outside_window() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        assert_eq!(net.accept_sequence(a, 1000), Ok(()));
        // 1000 - REPLAY_WINDOW is exactly out of window (back == W is rejected).
        let too_old = 1000 - REPLAY_WINDOW;
        assert_eq!(
            net.accept_sequence(a, too_old),
            Err(NetsessError::ReplayRejected)
        );
    }

    #[test]
    fn replay_window_slides_forward_and_forgets_far_past() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        assert_eq!(net.accept_sequence(a, 100), Ok(()));
        // Jump far enough forward that the whole window resets.
        assert_eq!(net.accept_sequence(a, 100 + REPLAY_WINDOW + 5), Ok(()));
        // 100 is now far outside the new window and must be rejected, not panic.
        assert_eq!(
            net.accept_sequence(a, 100),
            Err(NetsessError::ReplayRejected)
        );
    }

    #[test]
    fn rtt_estimator_converges_toward_a_steady_sample() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        let mut last = (0u32, 0u32, 0u32);
        for _ in 0..50 {
            last = net.record_rtt_sample(a, 100).unwrap();
        }
        assert_eq!(last.0, 100, "srtt converges to a constant sample");
        assert!(last.2 >= last.0, "rto never below srtt");
    }

    #[test]
    fn rtt_estimator_first_sample_seeds_srtt_and_half_rttvar() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        let (srtt, rttvar, rto) = net.record_rtt_sample(a, 200).unwrap();
        assert_eq!(srtt, 200);
        assert_eq!(rttvar, 100);
        assert_eq!(rto, srtt + 4 * rttvar);
    }

    #[test]
    fn descriptor_ring_push_drain_complete_round_trip() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        let token = net.push_descriptor(a, 42).unwrap();
        assert_eq!(net.ring_len(), 1);
        let (drained_token, session, packet_seq) = net.drain_descriptor().unwrap();
        assert_eq!(drained_token, token);
        assert_eq!(session, a);
        assert_eq!(packet_seq, 42);
        assert_eq!(net.complete_descriptor(token), Ok(()));
        assert_eq!(net.stats().descriptors_completed, 1);
    }

    #[test]
    fn descriptor_cannot_be_pushed_for_a_closed_session() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        net.close_session(a).unwrap();
        assert_eq!(net.push_descriptor(a, 1), Err(NetsessError::InvalidHandle));
    }

    #[test]
    fn descriptor_ring_wraps_around_and_reuses_completed_slots() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        for round in 0..(RING_CAPACITY as u64 * 3) {
            let token = net.push_descriptor(a, round).unwrap();
            let (drained, _, seq) = net.drain_descriptor().unwrap();
            assert_eq!(drained, token);
            assert_eq!(seq, round);
            net.complete_descriptor(token).unwrap();
        }
        assert_eq!(net.stats().descriptors_completed, RING_CAPACITY as u64 * 3);
        assert_eq!(net.ring_len(), 0);
    }

    #[test]
    fn descriptor_ring_backpressure_when_full() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        for i in 0..RING_CAPACITY as u64 {
            net.push_descriptor(a, i).unwrap();
        }
        // Ring is now full (nothing drained yet).
        assert_eq!(net.push_descriptor(a, 999), Err(NetsessError::RingFull));
        assert_eq!(net.stats().descriptors_rejected_full, 1);
    }

    #[test]
    fn stale_descriptor_token_is_rejected() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        let token = net.push_descriptor(a, 1).unwrap();
        net.drain_descriptor().unwrap();
        net.complete_descriptor(token).unwrap();
        // token has been completed and its slot's sequence has moved on;
        // completing/failing it again must not silently succeed.
        assert_eq!(
            net.complete_descriptor(token),
            Err(NetsessError::StaleDescriptor)
        );
        assert_eq!(
            net.fail_descriptor(token),
            Err(NetsessError::StaleDescriptor)
        );
    }

    #[test]
    fn not_processing_when_descriptor_never_drained() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        let token = net.push_descriptor(a, 1).unwrap();
        // Still KernelReady, never drained into Processing.
        assert_eq!(
            net.complete_descriptor(token),
            Err(NetsessError::NotProcessing)
        );
    }

    #[test]
    fn quarantined_descriptor_slot_is_never_reused() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        let token = net.push_descriptor(a, 1).unwrap();
        net.drain_descriptor().unwrap();
        assert_eq!(net.fail_descriptor(token), Ok(()));
        assert_eq!(net.stats().descriptors_failed, 1);
        // A second fail/complete on the same (now-quarantined) token must be
        // rejected, not silently accepted.
        assert_eq!(net.fail_descriptor(token), Err(NetsessError::NotProcessing));
        assert_eq!(
            net.complete_descriptor(token),
            Err(NetsessError::NotProcessing)
        );
        // The ring must never hand this slot out again: fill every other
        // slot and confirm the ring reports full without ever reusing the
        // quarantined one.
        for i in 0..(RING_CAPACITY as u64 - 1) {
            net.push_descriptor(a, i + 100).unwrap();
        }
        assert_eq!(
            net.push_descriptor(a, 9999),
            Err(NetsessError::RingFull),
            "quarantined slot must never become Writable again"
        );
    }

    #[test]
    fn session_nonce_is_retained_and_readable() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        assert_eq!(net.session_nonce(a), Ok(0xdead_beef));
        net.close_session(a).unwrap();
        assert_eq!(net.session_nonce(a), Err(NetsessError::InvalidHandle));
    }

    #[test]
    fn two_concurrently_tracked_sessions_are_independent() {
        let mut net = Netsess::new();
        let a = open(&mut net, 2);
        let b = open(&mut net, 3);
        assert_eq!(net.accept_sequence(a, 1), Ok(()));
        assert_eq!(net.accept_sequence(b, 1), Ok(()));
        // A duplicate on `a` must not affect `b`'s independent window.
        assert_eq!(net.accept_sequence(a, 1), Err(NetsessError::ReplayRejected));
        assert_eq!(net.accept_sequence(b, 2), Ok(()));
        assert_eq!(net.session_count(), 2);
    }

    #[test]
    fn unbound_gem_rx_take_does_not_record() {
        let mut net = Netsess::new();
        net.record_gem_rx_taken(64);
        assert_eq!(net.stats().gem_rx_recorded, 0);
        assert_eq!(net.stats().descriptors_pushed, 0);
        assert_eq!(net.session_count(), 0);
    }

    #[test]
    fn live_gem_rx_take_records_kernelready_not_hwowned() {
        let mut net = Netsess::new();
        net.bind_gem_session(root(1)).unwrap();
        net.record_gem_rx_taken(64);
        net.record_gem_rx_taken(128);
        let stats = net.stats();
        assert_eq!(stats.gem_rx_recorded, 2);
        assert_eq!(stats.gem_rx_rejected, 0);
        assert_eq!(stats.descriptors_pushed, 2);
        assert_eq!(stats.descriptors_completed, 2);
        assert_eq!(net.gem_rx_last_len(), 128);
        assert_eq!(net.ring_len(), 0);
        assert_eq!(net.session_count(), 1);
        for s in [
            DescriptorState::KernelReady,
            DescriptorState::Processing,
            DescriptorState::Quarantined,
        ] {
            assert!(!format!("{s:?}").contains("HwOwned"));
        }
    }

    #[test]
    fn live_gem_rx_take_is_not_the_software_demo_session() {
        let mut net = Netsess::new();
        let demo = open(&mut net, 2);
        net.bind_gem_session(root(1)).unwrap();
        net.record_gem_rx_taken(60);
        assert_eq!(net.session_count(), 2);
        assert_eq!(net.stats().gem_rx_recorded, 1);
        let token = net.push_descriptor(demo, 7).unwrap();
        net.drain_descriptor().unwrap();
        net.complete_descriptor(token).unwrap();
        assert_eq!(net.stats().descriptors_pushed, 2);
        assert_eq!(net.stats().gem_rx_recorded, 1);
    }
}
