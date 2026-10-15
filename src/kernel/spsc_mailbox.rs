//! T6: Lamport SPSC ring for CPU0↔CPU1 (N=16, power of two).
//!
//! Head is written only by the producer. Tail is written only by the consumer.
//! Payload is a plain store; DSB (A9) or `fence` (host) before the Release
//! index update. Not `core::arch::arm::{__dsb,__sev}` — those are not in this
//! crate. Not `&mut self`: both cores share one `static`, so this is `&self`
//! plus `UnsafeCell` slots.
//!
//! Cortex-A9 L1 line is 32 bytes (not 64). Head/tail and each slot are on
//! separate 32-byte lines. `payload_ptr` is omitted: T6 must not pass a
//! pointer into another core's stack.
//!
//! T4 `tensor_ring` stays `&mut self` / irqreg seq on the host. This module is
//! the A9-shareable mailbox. T7 PhaseHash jobs copy `TensorProofInput` into
//! the slot (no pointer into another core's stack). No speedup claim (T13).

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU32, Ordering};

pub const MAILBOX_CAPACITY: usize = 16;
pub const MAILBOX_MASK: u32 = (MAILBOX_CAPACITY as u32) - 1;

pub const OPCODE_PING: u32 = 0xFE;
pub const OPCODE_PONG: u32 = 0xFD;
pub const OPCODE_UNK: u32 = 0xFF;
/// T7: CPU1 computes `ph_tensor_proof` on the copied slot fields.
pub const OPCODE_PHASH: u32 = 0x01;
pub const OPCODE_PHASH_DONE: u32 = 0x02;

#[repr(C, align(32))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MailboxTask {
    pub task_id: u32,
    pub opcode: u32,
    pub arg0: u32,
    pub arg1: u32,
    pub sequence: u64,
    pub tensor: u8,
    pub _pad: [u8; 7],
    pub authority_generation: u64,
    pub chronos: u64,
    pub previous_root: [u8; 32],
    pub boot_session_root: [u8; 32],
    pub digest: [u8; 32],
}

impl MailboxTask {
    pub const EMPTY: Self = Self {
        task_id: 0,
        opcode: 0,
        arg0: 0,
        arg1: 0,
        sequence: 0,
        tensor: 0,
        _pad: [0; 7],
        authority_generation: 0,
        chronos: 0,
        previous_root: [0; 32],
        boot_session_root: [0; 32],
        digest: [0; 32],
    };

    pub const fn ping(task_id: u32, arg0: u32) -> Self {
        Self {
            task_id,
            opcode: OPCODE_PING,
            arg0,
            ..Self::EMPTY
        }
    }

    pub const fn pong(task_id: u32, arg0: u32) -> Self {
        Self {
            task_id,
            opcode: OPCODE_PONG,
            arg0,
            ..Self::EMPTY
        }
    }
}

#[repr(C, align(32))]
pub struct SpscMailbox {
    head: AtomicU32,
    _pad0: [u8; 28],
    tail: AtomicU32,
    _pad1: [u8; 28],
    buffer: UnsafeCell<[MailboxTask; MAILBOX_CAPACITY]>,
}

/// SPSC: one producer thread/core, one consumer. Slot ownership follows
/// Lamport (payload published before Release-head; consumed before Release-tail).
unsafe impl Sync for SpscMailbox {}

impl SpscMailbox {
    pub const fn new() -> Self {
        Self {
            head: AtomicU32::new(0),
            _pad0: [0; 28],
            tail: AtomicU32::new(0),
            _pad1: [0; 28],
            buffer: UnsafeCell::new([MailboxTask::EMPTY; MAILBOX_CAPACITY]),
        }
    }

    #[inline]
    fn payload_barrier() {
        #[cfg(target_arch = "arm")]
        unsafe {
            core::arch::asm!("dsb", options(nostack, preserves_flags));
        }
        #[cfg(not(target_arch = "arm"))]
        core::sync::atomic::fence(Ordering::SeqCst);
    }

    #[inline]
    fn send_event() {
        #[cfg(target_arch = "arm")]
        unsafe {
            core::arch::asm!("sev", options(nomem, nostack, preserves_flags));
        }
    }

    /// Producer. Returns `Err(task)` if the ring is full (no silent drop).
    pub fn push(&self, task: MailboxTask) -> Result<(), MailboxTask> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        if head.wrapping_sub(tail) as usize >= MAILBOX_CAPACITY {
            return Err(task);
        }
        let index = (head & MAILBOX_MASK) as usize;
        unsafe {
            (*self.buffer.get())[index] = task;
        }
        Self::payload_barrier();
        self.head.store(head.wrapping_add(1), Ordering::Release);
        Self::send_event();
        Ok(())
    }

    /// Consumer.
    pub fn pop(&self) -> Option<MailboxTask> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        if head == tail {
            return None;
        }
        Self::payload_barrier();
        let index = (tail & MAILBOX_MASK) as usize;
        let task = unsafe { (*self.buffer.get())[index] };
        Self::payload_barrier();
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        Self::send_event();
        Some(task)
    }
}

impl Default for SpscMailbox {
    fn default() -> Self {
        Self::new()
    }
}

/// CPU0 → CPU1 requests and CPU1 → CPU0 results.
pub struct Cpu1Channels {
    pub requests: SpscMailbox,
    pub results: SpscMailbox,
    pub phash_dispatched: AtomicU32,
}

impl Cpu1Channels {
    pub const fn new() -> Self {
        Self {
            requests: SpscMailbox::new(),
            results: SpscMailbox::new(),
            phash_dispatched: AtomicU32::new(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_rejects_then_drain_order() {
        let q = SpscMailbox::new();
        for i in 0..MAILBOX_CAPACITY as u32 {
            assert_eq!(q.push(MailboxTask::ping(i, i)), Ok(()));
        }
        let extra = MailboxTask::ping(99, 1);
        assert_eq!(q.push(extra), Err(extra));
        for i in 0..MAILBOX_CAPACITY as u32 {
            assert_eq!(q.pop(), Some(MailboxTask::ping(i, i)));
        }
        assert_eq!(q.pop(), None);
        assert_eq!(q.push(MailboxTask::ping(7, 8)), Ok(()));
        assert_eq!(q.pop(), Some(MailboxTask::ping(7, 8)));
    }

    #[test]
    fn wraparound_preserves_order() {
        let q = SpscMailbox::new();
        for i in 0u32..64 {
            assert_eq!(q.push(MailboxTask::ping(i, i ^ 0xA5A5)), Ok(()));
            assert_eq!(q.pop(), Some(MailboxTask::ping(i, i ^ 0xA5A5)));
        }
        for i in 0u32..MAILBOX_CAPACITY as u32 {
            assert_eq!(q.push(MailboxTask::ping(i, i)), Ok(()));
        }
        for i in 0u32..MAILBOX_CAPACITY as u32 {
            assert_eq!(q.pop(), Some(MailboxTask::ping(i, i)));
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod host_concurrency {
    use super::*;
    use std::thread;

    #[test]
    fn two_threads_spsc_order_no_arc_kernel() {
        let q = SpscMailbox::new();
        const N: u32 = 1000;
        thread::scope(|scope| {
            scope.spawn(|| {
                for i in 0..N {
                    let t = MailboxTask::ping(i, i.wrapping_mul(3));
                    while q.push(t).is_err() {
                        thread::yield_now();
                    }
                }
            });
            scope.spawn(|| {
                let mut next = 0u32;
                while next < N {
                    match q.pop() {
                        Some(got) => {
                            assert_eq!(got, MailboxTask::ping(next, next.wrapping_mul(3)));
                            next += 1;
                        }
                        None => thread::yield_now(),
                    }
                }
            });
        });
        assert_eq!(q.pop(), None);
    }
}
