//! IPC and message queues (`feature = "ipc"`), subsystem 5 of the eight
//! kernel-subsystems build program — see
//! `fpga/zynq/evidence/KERNEL_MATH_CONTRACT_2026-08-12.md` and
//! `fpga/zynq/evidence/IPC_STATUS_<date>.md`.
//!
//! Bounded message queues, each a fixed-capacity ring allocated once at
//! creation (cold path) and never resized. `IPC_SEND`/`IPC_RECV` run "over
//! `captable`+`sched2`" (the build contract's own dependency-order text):
//!
//! - **`captable`**: every queue is also a `captable::ObjectKind::Queue`
//!   object; `IPC_SEND`/`IPC_RECV`/`IPC_DESTROY_QUEUE` all require a real,
//!   generation-checked capability with the right bit set
//!   (`RIGHT_WRITE`/`RIGHT_READ`/`RIGHT_DELETE`), authorized via
//!   `captable::CapabilityTable::authorize` before this module is ever
//!   touched — exactly the `FS_OPEN`/`FS_CLOSE` pattern already proven for
//!   files, reused rather than re-invented (see `kernel::syscall`'s
//!   `IPC_*` handlers).
//! - **`sched2`**: `IPC_RECV` on an empty queue blocks the caller via
//!   `sched2::Sched2::block_on_ipc(handle, key, now)` with `key` = this
//!   module's own `IpcHandle` packed into a `u64`
//!   (`(slot as u64) << 32 | generation as u64`); `IPC_SEND` calls
//!   `sched2::Sched2::wake_matching(key, ..)` afterward so a real waiting
//!   receiver actually wakes. `sched2` never imports this module's types
//!   (see `sched2`'s own module docs) — the syscall layer is what composes
//!   the two, keeping both independently meaningful modules.
//!
//! ## What this reuses / does not reuse (per the math contract, §0.4)
//! - [`crate::kernel::event_log::ProcessId`] — no new process-identity type.
//! - The `(slot, generation)` generational-handle shape from Part III, as a
//!   **local** `IpcHandle` type — same rationale `irqreg::IrqHandle` and
//!   `sched2::Sched2Handle` already document (this module predates neither
//!   `captable` nor `sched2` architecturally, but importing either's exact
//!   handle type here would still wrongly couple three independently
//!   real, separately-toggleable concerns into one type).
//! - [`crate::kernel::crypto::phase_hash::ph_content`] (PH-C) over each
//!   message's bytes, computed **once at `send`** (the creation boundary —
//!   math contract §9/§Recovery's "full cryptographic work belongs at
//!   creation... not allocator/IRQ/scheduler/ring-buffer/cache hot loops"
//!   is satisfied because this is a one-time, per-message computation at
//!   the moment a message is created, not a per-tick/per-dispatch hot-loop
//!   cost). Content/identity binding only, matching the math contract's own
//!   row for `ipc`: "never for authorization". `recv` returns the stored
//!   root as audit metadata; it is not re-verified automatically (no
//!   corruption path exists in this single-core, RAM-only ring to detect),
//!   matching `captable`'s own `content_root` precedent.
//! - The exact bounded, per-slot sequence-number ring protocol already
//!   implemented for `irqreg`'s deferred-event ring (`Writable(j,n) =
//!   [seq_j = n]`, `Readable(j,n) = [seq_j = n+1]`, `Reusable(j,n) =
//!   [seq_j = n+N]`) — reused verbatim here for each queue's message ring,
//!   not reinvented.
//! - This subsystem consumes **no G64 operation and no `PhaseHash`/
//!   `PhaseState` reflection** beyond the single PH-C call above — message
//!   queueing is not phase mathematics. Explicitly stated per Gate 0's
//!   instruction to declare this for every subsystem.
//!
//! ## Non-goals (explicit — see also `IPC_STATUS_<date>.md`)
//! - **`send` never blocks.** A full queue returns `IpcError::QueueFull`
//!   immediately. Blocking a *sender* would need its own wait-list
//!   bookkeeping (symmetric to the receiver's), which the "bounded message
//!   queues" contract text does not require; a bounded mailbox with a hard
//!   capacity error on the producer side is a real, complete design on its
//!   own, not a placeholder for a blocking-send that was never built.
//! - No priority/ordering beyond strict FIFO per queue.
//! - No cross-queue select/poll (waiting on multiple queues at once).
//! - No persistence/recovery: this table is RAM-only, like `captable`,
//!   `irqreg`, and `sched2`; `X_before/X_fault/X_restored` recovery
//!   mathematics do not apply to a subsystem with no checkpoint/restore
//!   operation of its own.

use core::fmt;

use crate::kernel::crypto::phase_hash::ph_content;
use crate::kernel::event_log::ProcessId;
use crate::kernel::hash::StateHash;
use crate::platform::Vec;

/// Bound on a single queue's ring capacity (power-of-two, per the same
/// `j = n & (N-1)` indexing `irqreg`'s ring already uses).
pub const MAX_QUEUE_CAPACITY: usize = 64;
/// Fixed per-message byte capacity. A message that does not fit is rejected
/// at `send`, not truncated.
pub const MAX_MESSAGE_LEN: usize = 64;

/// `h = (i, g)` — a local generational handle. See module docs for why this
/// is not `captable::GenerationalHandle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IpcHandle {
    pub slot: u32,
    pub generation: u32,
}

impl IpcHandle {
    /// Pack into the opaque `u64` key `sched2::WaitReason::Ipc`/
    /// `block_on_ipc`/`wake_matching` compare by equality only.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcError {
    /// Slot index out of range, or not currently occupied.
    InvalidHandle,
    /// Slot occupied, but by a different (newer) generation.
    StaleGeneration,
    /// `create_queue`'s capacity was `0`, not a power of two, or exceeded
    /// [`MAX_QUEUE_CAPACITY`].
    InvalidCapacity,
    /// `send`'s payload exceeded [`MAX_MESSAGE_LEN`].
    MessageTooLarge,
    /// The ring was full (see module docs, "Non-goals": `send` never
    /// blocks).
    QueueFull,
    /// The ring was empty. The caller (`kernel::syscall`'s `IPC_RECV`
    /// handler) is expected to block the caller via `sched2` on this
    /// condition, not treat it as a hard failure.
    QueueEmpty,
}

impl fmt::Display for IpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            IpcError::InvalidHandle => "invalid handle",
            IpcError::StaleGeneration => "stale generation",
            IpcError::InvalidCapacity => "invalid queue capacity",
            IpcError::MessageTooLarge => "message too large",
            IpcError::QueueFull => "queue full",
            IpcError::QueueEmpty => "queue empty",
        };
        f.write_str(msg)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IpcStats {
    pub queues_created: u64,
    pub queues_destroyed: u64,
    pub generation_retired: u64,
    pub sent: u64,
    pub send_rejected_full: u64,
    pub send_rejected_too_large: u64,
    pub received: u64,
    pub recv_would_block: u64,
}

/// A received message: the real sender, the real payload bytes, and the
/// PH-C root computed over those bytes at `send` time (audit metadata —
/// see module docs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub sender: ProcessId,
    pub data: Vec<u8>,
    pub content_root: StateHash,
}

#[derive(Clone)]
struct RingSlot {
    /// Vyukov-style bounded-ring sequence number. `Writable(j,n) =
    /// [seq_j = n]`. Payload fields below are only meaningful once
    /// `Readable(j,n) = [seq_j = n+1]` holds.
    sequence: u64,
    sender: ProcessId,
    len: u8,
    data: [u8; MAX_MESSAGE_LEN],
    content_root: StateHash,
}

impl RingSlot {
    fn empty(seed: u64) -> Self {
        Self {
            sequence: seed,
            sender: 0,
            len: 0,
            data: [0u8; MAX_MESSAGE_LEN],
            content_root: StateHash::default(),
        }
    }
}

struct QueueSlot {
    occupied: bool,
    generation: u32,
    capacity_mask: u64,
    ring: Vec<RingSlot>,
    /// `n`: next sequence number the next `send` will write.
    ring_next: u64,
    /// Next sequence number the next `recv` expects to read.
    ring_read: u64,
}

/// Bumps `*generation`. Returns `true` on overflow (`u32::MAX -> 0`
/// avoided); mirrors `kernel::captable`/`kernel::sched2`'s identical rule.
fn bump_generation(generation: &mut u32) -> bool {
    match generation.checked_add(1) {
        Some(next) => {
            *generation = next;
            false
        }
        None => true,
    }
}

/// IPC and message-queue engine. See module docs for the full design and
/// integration points.
pub struct Ipc {
    queues: Vec<QueueSlot>,
    free: Vec<u32>,
    stats: IpcStats,
}

impl Default for Ipc {
    fn default() -> Self {
        Self::new()
    }
}

impl Ipc {
    pub fn new() -> Self {
        Self {
            queues: Vec::new(),
            free: Vec::new(),
            stats: IpcStats::default(),
        }
    }

    pub fn stats(&self) -> IpcStats {
        self.stats
    }

    pub fn queue_count(&self) -> usize {
        self.queues.iter().filter(|q| q.occupied).count()
    }

    fn validate(&self, handle: IpcHandle) -> Result<usize, IpcError> {
        let slot = self
            .queues
            .get(handle.slot as usize)
            .ok_or(IpcError::InvalidHandle)?;
        if !slot.occupied {
            return Err(IpcError::InvalidHandle);
        }
        if slot.generation != handle.generation {
            return Err(IpcError::StaleGeneration);
        }
        Ok(handle.slot as usize)
    }

    /// Create a new bounded queue. `capacity` must be a non-zero power of
    /// two, at most [`MAX_QUEUE_CAPACITY`]. The ring's fixed-size backing
    /// storage is allocated exactly once, here — never inside `send`/`recv`.
    pub fn create_queue(&mut self, capacity: usize) -> Result<IpcHandle, IpcError> {
        if capacity == 0 || capacity > MAX_QUEUE_CAPACITY || !capacity.is_power_of_two() {
            return Err(IpcError::InvalidCapacity);
        }
        // Vyukov-style bounded-ring seeding: slot j's sequence starts at j
        // so the first producer pass (n=j) sees Writable(j,j)=true.
        let ring: Vec<RingSlot> = (0..capacity).map(|j| RingSlot::empty(j as u64)).collect();
        let new_slot = QueueSlot {
            occupied: true,
            generation: 0, // overwritten below if reusing a slot
            capacity_mask: (capacity - 1) as u64,
            ring,
            ring_next: 0,
            ring_read: 0,
        };
        let slot = if let Some(slot) = self.free.pop() {
            let existing_generation = self.queues[slot as usize].generation;
            self.queues[slot as usize] = new_slot;
            self.queues[slot as usize].generation = existing_generation;
            slot
        } else {
            let slot = self.queues.len() as u32;
            self.queues.push(new_slot);
            slot
        };
        self.stats.queues_created += 1;
        Ok(IpcHandle {
            slot,
            generation: self.queues[slot as usize].generation,
        })
    }

    /// Destroy a queue, discarding any still-queued messages. Bumps the
    /// slot's generation, retiring it permanently on `u32` overflow instead
    /// of ever wrapping into a previously-valid generation (mirrors
    /// `captable::close`/`sched2::unregister`).
    pub fn destroy_queue(&mut self, handle: IpcHandle) -> Result<(), IpcError> {
        let idx = self.validate(handle)?;
        self.queues[idx].occupied = false;
        self.queues[idx].ring = Vec::new();
        let retired = bump_generation(&mut self.queues[idx].generation);
        if retired {
            self.stats.generation_retired += 1;
        } else {
            self.free.push(handle.slot);
        }
        self.stats.queues_destroyed += 1;
        Ok(())
    }

    pub fn len(&self, handle: IpcHandle) -> Result<usize, IpcError> {
        let idx = self.validate(handle)?;
        Ok((self.queues[idx].ring_next - self.queues[idx].ring_read) as usize)
    }

    pub fn capacity(&self, handle: IpcHandle) -> Result<usize, IpcError> {
        let idx = self.validate(handle)?;
        Ok((self.queues[idx].capacity_mask + 1) as usize)
    }

    /// Enqueue `data` from `sender`. Never blocks (see module docs,
    /// "Non-goals"): a full ring returns `QueueFull` immediately. PH-C is
    /// computed once here, at the message's creation boundary.
    pub fn send(
        &mut self,
        handle: IpcHandle,
        sender: ProcessId,
        data: &[u8],
    ) -> Result<(), IpcError> {
        if data.len() > MAX_MESSAGE_LEN {
            self.stats.send_rejected_too_large += 1;
            return Err(IpcError::MessageTooLarge);
        }
        let idx = self.validate(handle)?;
        let n = self.queues[idx].ring_next;
        let mask = self.queues[idx].capacity_mask;
        let j = (n & mask) as usize;
        if self.queues[idx].ring[j].sequence != n {
            self.stats.send_rejected_full += 1;
            return Err(IpcError::QueueFull);
        }
        let content_root = ph_content(data);
        let mut buf = [0u8; MAX_MESSAGE_LEN];
        buf[..data.len()].copy_from_slice(data);
        self.queues[idx].ring[j] = RingSlot {
            sequence: n + 1,
            sender,
            len: data.len() as u8,
            data: buf,
            content_root,
        };
        self.queues[idx].ring_next = n + 1;
        self.stats.sent += 1;
        Ok(())
    }

    /// Dequeue the oldest message in strict FIFO order. Returns
    /// `QueueEmpty` (not an error the caller should treat as fatal — see
    /// module docs) when nothing is queued.
    pub fn recv(&mut self, handle: IpcHandle) -> Result<Message, IpcError> {
        let idx = self.validate(handle)?;
        let n = self.queues[idx].ring_read;
        let mask = self.queues[idx].capacity_mask;
        let capacity = mask + 1;
        let j = (n & mask) as usize;
        if self.queues[idx].ring[j].sequence != n + 1 {
            self.stats.recv_would_block += 1;
            return Err(IpcError::QueueEmpty);
        }
        let slot = &self.queues[idx].ring[j];
        let message = Message {
            sender: slot.sender,
            data: slot.data[..slot.len as usize].to_vec(),
            content_root: slot.content_root,
        };
        self.queues[idx].ring[j].sequence = n + capacity;
        self.queues[idx].ring_read = n + 1;
        self.stats.received += 1;
        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_destroy_and_stale_handle_rejected() {
        let mut ipc = Ipc::new();
        let h = ipc.create_queue(4).unwrap();
        assert_eq!(ipc.len(h), Ok(0));
        ipc.destroy_queue(h).unwrap();
        assert_eq!(ipc.len(h), Err(IpcError::InvalidHandle));
        let h2 = ipc.create_queue(4).unwrap();
        if h2.slot == h.slot {
            assert_eq!(ipc.len(h), Err(IpcError::StaleGeneration));
        }
    }

    #[test]
    fn invalid_capacity_is_rejected() {
        let mut ipc = Ipc::new();
        assert_eq!(ipc.create_queue(0), Err(IpcError::InvalidCapacity));
        assert_eq!(ipc.create_queue(3), Err(IpcError::InvalidCapacity));
        assert_eq!(
            ipc.create_queue(MAX_QUEUE_CAPACITY * 2),
            Err(IpcError::InvalidCapacity)
        );
        assert!(ipc.create_queue(MAX_QUEUE_CAPACITY).is_ok());
    }

    /// Required proof: a real send/recv round trip preserves the exact
    /// sender, bytes, and a stable content root, in strict FIFO order.
    #[test]
    fn send_recv_round_trip_preserves_sender_bytes_and_fifo_order() {
        let mut ipc = Ipc::new();
        let h = ipc.create_queue(4).unwrap();
        ipc.send(h, 7, b"first").unwrap();
        ipc.send(h, 9, b"second").unwrap();
        assert_eq!(ipc.len(h), Ok(2));

        let m1 = ipc.recv(h).unwrap();
        assert_eq!(m1.sender, 7);
        assert_eq!(m1.data, b"first");
        assert_eq!(m1.content_root, ph_content(b"first"));

        let m2 = ipc.recv(h).unwrap();
        assert_eq!(m2.sender, 9);
        assert_eq!(m2.data, b"second");

        assert_eq!(ipc.recv(h), Err(IpcError::QueueEmpty));
    }

    #[test]
    fn message_too_large_is_rejected_without_touching_the_ring() {
        let mut ipc = Ipc::new();
        let h = ipc.create_queue(4).unwrap();
        let oversized = [0u8; MAX_MESSAGE_LEN + 1];
        assert_eq!(ipc.send(h, 1, &oversized), Err(IpcError::MessageTooLarge));
        assert_eq!(
            ipc.len(h),
            Ok(0),
            "the rejected send must not enqueue anything"
        );
    }

    /// Required proof: a full bounded queue rejects further sends
    /// immediately (never blocks the sender — see module docs).
    #[test]
    fn full_queue_rejects_further_sends_and_recv_drains_it_in_order() {
        let mut ipc = Ipc::new();
        let h = ipc.create_queue(2).unwrap();
        ipc.send(h, 1, b"a").unwrap();
        ipc.send(h, 1, b"b").unwrap();
        assert_eq!(ipc.send(h, 1, b"c"), Err(IpcError::QueueFull));
        assert_eq!(ipc.recv(h).unwrap().data, b"a");
        // Draining one slot must free exactly one send's worth of room.
        ipc.send(h, 1, b"c").unwrap();
        assert_eq!(ipc.recv(h).unwrap().data, b"b");
        assert_eq!(ipc.recv(h).unwrap().data, b"c");
        assert_eq!(ipc.recv(h), Err(IpcError::QueueEmpty));
    }

    /// Required proof: many send/recv cycles across ring-index wraparound
    /// never corrupt ordering or content (the exact Vyukov protocol
    /// `irqreg` already proved, reused here).
    #[test]
    fn many_cycles_across_ring_wraparound_preserve_order_and_content() {
        let mut ipc = Ipc::new();
        let h = ipc.create_queue(4).unwrap();
        for round in 0u32..500 {
            let payload = round.to_le_bytes();
            ipc.send(h, round, &payload).unwrap();
            let msg = ipc.recv(h).unwrap();
            assert_eq!(msg.sender, round);
            assert_eq!(msg.data, payload);
        }
    }

    #[test]
    fn destroy_then_recreate_reuses_slot_without_leaking_old_messages() {
        let mut ipc = Ipc::new();
        let h = ipc.create_queue(4).unwrap();
        ipc.send(h, 1, b"stale").unwrap();
        ipc.destroy_queue(h).unwrap();
        let h2 = ipc.create_queue(4).unwrap();
        assert_eq!(
            ipc.len(h2),
            Ok(0),
            "a freshly (re)created queue must never carry over the destroyed queue's messages"
        );
        assert_eq!(ipc.recv(h2), Err(IpcError::QueueEmpty));
    }

    #[test]
    fn pack_unpack_round_trips() {
        let h = IpcHandle {
            slot: 0x1234_5678,
            generation: 0x9ABC_DEF0,
        };
        assert_eq!(IpcHandle::unpack(h.pack()), h);
    }
}
