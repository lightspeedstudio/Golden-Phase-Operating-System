//! Bounded SPSC request/result rings for tensor evaluate/commit (T4).
//!
//! Sequence protocol copied from `kernel::irqreg` / the math-contract IPC
//! ring: `Writable(j,n) = [seq_j = n]`, `Readable(j,n) = [seq_j = n+1]`,
//! `Reusable(j,n) = [seq_j = n+N]`. Single-core `&mut self`. Host tests may
//! wrap the ring in `Mutex`; they must not put `Kernel` in `Arc`.
//!
//! Not a Cortex-A9 SMP proof. Not lock-free on the board (MMU off).

use super::tensor_state::{TensorOp, TensorState};

pub const RING_CAPACITY: usize = 8;
const RING_MASK: u64 = (RING_CAPACITY - 1) as u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TensorRingError {
    Full,
    Empty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TensorRequest {
    pub sequence: u64,
    pub generation: u32,
    pub current: TensorState,
    pub op: TensorOp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TensorResult {
    pub sequence: u64,
    pub generation: u32,
    pub candidate: TensorState,
}

#[derive(Clone, Copy)]
struct Slot<T: Copy> {
    sequence: u64,
    value: Option<T>,
}

/// One bounded SPSC ring. Producer is CPU0 (requests) or the worker (results);
/// consumer is the other side. Use two rings, not shared `Kernel`.
pub struct TensorSpsc<T: Copy> {
    ring: [Slot<T>; RING_CAPACITY],
    write: u64,
    read: u64,
    pub dropped: u64,
}

impl<T: Copy> TensorSpsc<T> {
    pub fn new() -> Self {
        Self {
            ring: core::array::from_fn(|j| Slot {
                sequence: j as u64,
                value: None,
            }),
            write: 0,
            read: 0,
            dropped: 0,
        }
    }

    pub fn push(&mut self, value: T) -> Result<(), TensorRingError> {
        let n = self.write;
        let j = (n & RING_MASK) as usize;
        if self.ring[j].sequence != n {
            self.dropped = self.dropped.wrapping_add(1);
            return Err(TensorRingError::Full);
        }
        self.ring[j].value = Some(value);
        self.ring[j].sequence = n + 1;
        self.write = n + 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Result<T, TensorRingError> {
        let n = self.read;
        let j = (n & RING_MASK) as usize;
        if self.ring[j].sequence != n + 1 {
            return Err(TensorRingError::Empty);
        }
        let value = self.ring[j].value.take().expect("readable slot has payload");
        self.ring[j].sequence = n + RING_CAPACITY as u64;
        self.read = n + 1;
        Ok(value)
    }
}

impl<T: Copy> Default for TensorSpsc<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// CPU0→worker requests and worker→CPU0 results, plus a shutdown generation.
/// Bumping `shutdown_generation` invalidates in-flight work the consumer
/// must drop (compare against the request's `generation`).
pub struct TensorChannels {
    pub requests: TensorSpsc<TensorRequest>,
    pub results: TensorSpsc<TensorResult>,
    pub shutdown_generation: u32,
    pub next_sequence: u64,
}

impl TensorChannels {
    pub fn new() -> Self {
        Self {
            requests: TensorSpsc::new(),
            results: TensorSpsc::new(),
            shutdown_generation: 0,
            next_sequence: 0,
        }
    }

    pub fn submit(
        &mut self,
        current: TensorState,
        op: TensorOp,
    ) -> Result<u64, TensorRingError> {
        let sequence = self.next_sequence;
        self.requests.push(TensorRequest {
            sequence,
            generation: self.shutdown_generation,
            current,
            op,
        })?;
        self.next_sequence = sequence.wrapping_add(1);
        Ok(sequence)
    }
}

impl Default for TensorChannels {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::gphase_core::PhaseState;
    use crate::kernel::tensor_state::{commit, evaluate, TensorOp, TensorState};

    #[test]
    fn seq_protocol_fills_then_rejects_then_drains() {
        let mut q = TensorSpsc::new();
        for i in 0..RING_CAPACITY {
            assert_eq!(q.push(i as u32), Ok(()));
        }
        assert_eq!(q.push(99), Err(TensorRingError::Full));
        assert_eq!(q.dropped, 1);
        for i in 0..RING_CAPACITY {
            assert_eq!(q.pop(), Ok(i as u32));
        }
        assert_eq!(q.pop(), Err(TensorRingError::Empty));
        assert_eq!(q.push(7), Ok(()));
        assert_eq!(q.pop(), Ok(7));
    }

    #[test]
    fn wraparound_preserves_order() {
        let mut q = TensorSpsc::new();
        for i in 0u32..64 {
            assert_eq!(q.push(i), Ok(()));
            assert_eq!(q.pop(), Ok(i));
        }
        for i in 0u32..RING_CAPACITY as u32 {
            assert_eq!(q.push(i), Ok(()));
        }
        for i in 0u32..RING_CAPACITY as u32 {
            assert_eq!(q.pop(), Ok(i));
        }
    }

    #[test]
    fn request_evaluate_result_commit_round_trip() {
        let mut ch = TensorChannels::new();
        let current = TensorState::snapshot(PhaseState::Phi2, 1, 1);
        let op = TensorOp::Reflect;
        let seq = ch.submit(current, op).unwrap();
        let req = ch.requests.pop().unwrap();
        assert_eq!(req.sequence, seq);
        assert_eq!(req.generation, 0);
        let candidate = evaluate(req.current, req.op);
        assert_eq!(
            ch.results.push(TensorResult {
                sequence: req.sequence,
                generation: req.generation,
                candidate,
            }),
            Ok(())
        );
        let res = ch.results.pop().unwrap();
        assert_eq!(res.sequence, seq);
        assert_eq!(commit(req.current, res.candidate, req.op), Ok(candidate));
    }

    #[test]
    fn stale_generation_is_dropped_not_committed() {
        let mut ch = TensorChannels::new();
        let current = TensorState::snapshot(PhaseState::Phi0, 0, 0);
        ch.submit(current, TensorOp::AdvanceM).unwrap();
        ch.shutdown_generation = 1;
        let req = ch.requests.pop().unwrap();
        assert_ne!(req.generation, ch.shutdown_generation);
    }

    #[test]
    fn duplicate_result_sequence_rejected_by_caller() {
        let mut ch = TensorChannels::new();
        let current = TensorState::snapshot(PhaseState::Phi0, 0, 0);
        let seq = ch.submit(current, TensorOp::Reflect).unwrap();
        let req = ch.requests.pop().unwrap();
        let candidate = evaluate(req.current, req.op);
        let res = TensorResult {
            sequence: seq,
            generation: 0,
            candidate,
        };
        assert_eq!(ch.results.push(res), Ok(()));
        let first = ch.results.pop().unwrap();
        assert_eq!(first.sequence, seq);
        let duplicate = TensorResult {
            sequence: seq,
            generation: 0,
            candidate,
        };
        assert_ne!(duplicate.sequence, ch.next_sequence);
    }
}

#[cfg(all(test, feature = "std"))]
mod host_concurrency {
    use super::*;
    use crate::core::gphase_core::PhaseState;
    use crate::kernel::tensor_state::{commit, evaluate, TensorOp, TensorState};
    use std::sync::Mutex;
    use std::thread;

    #[test]
    fn two_threads_mutex_ring_no_arc_kernel() {
        let ch = Mutex::new(TensorChannels::new());
        let current = TensorState::snapshot(PhaseState::Phi1, 3, 2);
        let op = TensorOp::Compose(PhaseState::Phi2);
        thread::scope(|scope| {
            scope.spawn(|| {
                let mut g = ch.lock().unwrap();
                g.submit(current, op).unwrap();
            });
        });
        thread::scope(|scope| {
            scope.spawn(|| {
                let mut g = ch.lock().unwrap();
                let req = g.requests.pop().expect("request");
                let candidate = evaluate(req.current, req.op);
                g.results
                    .push(TensorResult {
                        sequence: req.sequence,
                        generation: req.generation,
                        candidate,
                    })
                    .unwrap();
            });
        });
        let mut g = ch.lock().unwrap();
        let res = g.results.pop().expect("result");
        assert_eq!(commit(current, res.candidate, op), Ok(res.candidate));
    }
}
