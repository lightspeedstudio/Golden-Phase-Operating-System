//! T7: CPU1 computes [`ph_tensor_proof`] on a copied slot; CPU0 still commits.
//!
//! No `PhaseHash::compute_slice` — that API is not in this tree. No pointers
//! into another core's stack. CPU1 never maps `0x40000000`. Dispatch follows
//! [`worker_path_policy_with_proof`]. Not a speedup claim.

use core::sync::atomic::Ordering;

use crate::kernel::crypto::phase_hash::{ph_tensor_proof, TensorProofInput};
use crate::kernel::fault_syndrome::{
    worker_path_policy_with_proof, Health, WorkerPathPolicy,
};
use crate::kernel::hash::StateHash;
use crate::kernel::spsc_mailbox::{
    Cpu1Channels, MailboxTask, OPCODE_PHASH, OPCODE_PHASH_DONE,
};

pub const T7_SPLIT_COUNT: u32 = 32;
pub const T7_WAIT_SPINS: u32 = 4_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplitHashes {
    pub hash_a: StateHash,
    pub hash_b: StateHash,
    pub used_cpu1: bool,
}

pub fn proof_from_task(task: &MailboxTask) -> TensorProofInput {
    TensorProofInput {
        tensor: task.tensor,
        sequence: task.sequence,
        previous_root: task.previous_root,
        boot_session_root: task.boot_session_root,
        authority_generation: task.authority_generation,
        chronos: task.chronos,
    }
}

pub fn task_from_proof(task_id: u32, input: &TensorProofInput) -> MailboxTask {
    MailboxTask {
        task_id,
        opcode: OPCODE_PHASH,
        arg0: 0,
        arg1: 0,
        sequence: input.sequence,
        tensor: input.tensor,
        _pad: [0; 7],
        authority_generation: input.authority_generation,
        chronos: input.chronos,
        previous_root: input.previous_root,
        boot_session_root: input.boot_session_root,
        digest: [0; 32],
    }
}

/// CPU1 (or a host stand-in): `tuple_hash` → 32-byte `StateHash`. No MMIO.
pub fn cpu1_eval_phash(task: MailboxTask) -> MailboxTask {
    let digest = ph_tensor_proof(&proof_from_task(&task));
    MailboxTask {
        opcode: OPCODE_PHASH_DONE,
        digest,
        ..task
    }
}

fn wait_result(ch: &Cpu1Channels, task_id: u32, spins: u32) -> Option<MailboxTask> {
    for _ in 0..spins {
        if let Some(got) = ch.results.pop() {
            if got.task_id == task_id {
                return Some(got);
            }
        }
        core::hint::spin_loop();
        #[cfg(all(feature = "std", not(target_arch = "arm")))]
        std::thread::yield_now();
    }
    None
}

/// Chunk A on the caller (CPU0). Chunk B on CPU1 only if T12 allows it.
pub fn compute_split(
    ch: &Cpu1Channels,
    chunk_a: &TensorProofInput,
    chunk_b: &TensorProofInput,
    health: Health,
    proof_ok: bool,
    cpu1_online: bool,
    wait_spins: u32,
) -> SplitHashes {
    let allow = worker_path_policy_with_proof(health, proof_ok)
        == WorkerPathPolicy::AllowParallel
        && cpu1_online;
    if !allow {
        return SplitHashes {
            hash_a: ph_tensor_proof(chunk_a),
            hash_b: ph_tensor_proof(chunk_b),
            used_cpu1: false,
        };
    }

    let task_id = ch.phash_dispatched.load(Ordering::Relaxed);
    let task = task_from_proof(task_id, chunk_b);
    if ch.requests.push(task).is_err() {
        return SplitHashes {
            hash_a: ph_tensor_proof(chunk_a),
            hash_b: ph_tensor_proof(chunk_b),
            used_cpu1: false,
        };
    }
    ch.phash_dispatched.fetch_add(1, Ordering::Release);

    let hash_a = ph_tensor_proof(chunk_a);
    match wait_result(ch, task_id, wait_spins) {
        Some(got) if got.opcode == OPCODE_PHASH_DONE => SplitHashes {
            hash_a,
            hash_b: got.digest,
            used_cpu1: true,
        },
        _ => SplitHashes {
            hash_a,
            hash_b: ph_tensor_proof(chunk_b),
            used_cpu1: false,
        },
    }
}

pub fn sample_input(tensor: u8, sequence: u64) -> TensorProofInput {
    TensorProofInput {
        tensor,
        sequence,
        previous_root: [0x11; 32],
        boot_session_root: [0x22; 32],
        authority_generation: 3,
        chronos: 10 + sequence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::spsc_mailbox::Cpu1Channels;

    #[test]
    fn cpu1_eval_matches_cpu0_ph_tensor_proof() {
        let inp = sample_input(0x98, 7);
        let task = task_from_proof(1, &inp);
        let got = cpu1_eval_phash(task);
        assert_eq!(got.opcode, OPCODE_PHASH_DONE);
        assert_eq!(got.digest, ph_tensor_proof(&inp));
    }

    #[test]
    fn suspect_fault_isolate_and_failed_proof_do_not_dispatch() {
        let ch = Cpu1Channels::new();
        let a = sample_input(0x98, 1);
        let b = sample_input(0x99, 1);
        for (health, proof_ok) in [
            (Health::Suspect, true),
            (Health::Fault, true),
            (Health::Isolate, true),
            (Health::Valid, false),
        ] {
            let before = ch.phash_dispatched.load(Ordering::Relaxed);
            let got = compute_split(&ch, &a, &b, health, proof_ok, true, 8);
            assert!(!got.used_cpu1);
            assert_eq!(got.hash_a, ph_tensor_proof(&a));
            assert_eq!(got.hash_b, ph_tensor_proof(&b));
            assert_eq!(ch.phash_dispatched.load(Ordering::Relaxed), before);
            assert!(ch.requests.pop().is_none());
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod host_split {
    use super::*;
    use crate::kernel::spsc_mailbox::Cpu1Channels;
    use std::thread;

    #[test]
    fn parallel_split_matches_single_core_reference() {
        let ch = Cpu1Channels::new();
        thread::scope(|scope| {
            scope.spawn(|| {
                let mut n = 0u32;
                while n < T7_SPLIT_COUNT {
                    if let Some(task) = ch.requests.pop() {
                        let reply = cpu1_eval_phash(task);
                        while ch.results.push(reply).is_err() {
                            thread::yield_now();
                        }
                        n += 1;
                    } else {
                        thread::yield_now();
                    }
                }
            });
            for i in 0..T7_SPLIT_COUNT {
                let a = sample_input(0x98, i as u64);
                let b = sample_input(0x99, i as u64);
                let got = compute_split(&ch, &a, &b, Health::Valid, true, true, 2_000_000);
                assert!(got.used_cpu1, "seq={i}");
                assert_eq!(got.hash_a, ph_tensor_proof(&a));
                assert_eq!(got.hash_b, ph_tensor_proof(&b));
            }
        });
    }
}
