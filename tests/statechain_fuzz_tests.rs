//! Avenue 6 — adversarial fuzz tests for statechain tamper detection.
//!
//! These tests never reimplement `compose` or the hash/verify logic: chains
//! are built exclusively through the public `ObjectStatechain::append` API
//! (from `kairos::kernel::statechain`), and detection is measured with the
//! real `ObjectStatechain::verify()` — the same function
//! `StatechainLedger::verify_all()` delegates to for `chains_ok` in
//! `StatechainWatchdog::audit`. Tampering itself (flipping bits, deleting,
//! reordering, splicing) is done by mutating the `pub` fields of
//! `StateEvent`/`StateTransitionProof` directly, bypassing `append` on
//! purpose — that is the adversary model: something with in-process access
//! to a stored chain, not something going through the honest append path.
//!
//! Per the roadmap's "honest rule": these tests do not assert a detection
//! rate of exactly 1.0 for every class. A class measured below 100% is a
//! finding to print and explain, not a bug to paper over. Each test prints
//! its detection count so the numbers can be copied into
//! `docs/reports/AVENUE_06_statechain_fuzz.md` verbatim.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use kairos::core::gphase_core::PhaseState;
use kairos::kernel::state_proof::{
    hash_payload, ObjectKind, StateTransitionProof, TopologyProofStatus,
};
use kairos::kernel::state_root::{compute_equation_root, SubsystemKind};
use kairos::kernel::statechain::ObjectStatechain;

const TRIALS_PER_CLASS: usize = 10_000;
const MIN_LEN: usize = 4;
const MAX_LEN: usize = 20; // exclusive

fn random_state(rng: &mut StdRng) -> PhaseState {
    PhaseState::from_u8(rng.gen_range(0..4)).unwrap()
}

fn random_object_kind(rng: &mut StdRng) -> ObjectKind {
    match rng.gen_range(0..8) {
        0 => ObjectKind::Process,
        1 => ObjectKind::MemoryPage,
        2 => ObjectKind::File,
        3 => ObjectKind::Directory,
        4 => ObjectKind::Block,
        5 => ObjectKind::Syscall,
        6 => ObjectKind::Vm,
        _ => ObjectKind::Driver,
    }
}

fn random_topology(rng: &mut StdRng) -> TopologyProofStatus {
    match rng.gen_range(0..3) {
        0 => TopologyProofStatus::None,
        1 => TopologyProofStatus::Same,
        _ => TopologyProofStatus::Reflection,
    }
}

fn random_operation_name(rng: &mut StdRng) -> String {
    let len = rng.gen_range(1..12);
    (0..len)
        .map(|_| (rng.gen_range(0u8..26) + b'a') as char)
        .collect()
}

fn random_payload(rng: &mut StdRng) -> Vec<u8> {
    let len = rng.gen_range(0..64);
    (0..len).map(|_| rng.gen()).collect()
}

/// Build a random, internally-valid statechain of `len` events, using only
/// the public `append()` path (never reimplements hash/compose logic).
fn build_random_chain(rng: &mut StdRng, len: usize) -> ObjectStatechain {
    let eq = compute_equation_root();
    let kind = random_object_kind(rng);
    let id = rng.gen_range(0..1000);
    let mut chain = ObjectStatechain::new(kind, id);
    for i in 0..len {
        let payload = random_payload(rng);
        let compose_result = if rng.gen_bool(0.5) {
            Some(random_state(rng))
        } else {
            None
        };
        let proof = StateTransitionProof {
            object_kind: kind,
            object_id: id,
            sequence_number: 0, // overwritten by append()
            previous_state: random_state(rng),
            requested_state: random_state(rng),
            operation_name: random_operation_name(rng).into(),
            compose_result,
            topology_status: random_topology(rng),
            payload_hash: hash_payload(&payload),
            previous_event_hash: [0u8; 32], // overwritten by append()
            equation_root: eq.hash,
        };
        let assoc_parity = rng.gen_range(0u8..2);
        chain
            .append(proof, i as u64, SubsystemKind::Process, assoc_parity)
            .expect("random chain must build validly via the real append() path");
    }
    chain
}

fn flip_random_bit(bytes: &mut [u8; 32], rng: &mut StdRng) {
    let byte_idx = rng.gen_range(0..32);
    let bit_idx = rng.gen_range(0..8u32);
    bytes[byte_idx] ^= 1 << bit_idx;
}

/// Sanity check: the chain builder itself produces chains that verify clean
/// before any tampering. If this ever fails, the fuzz harness is broken,
/// not the statechain.
#[test]
fn random_chains_verify_clean_before_tampering() {
    let mut rng = StdRng::seed_from_u64(0xC0FFEE_u64);
    for _ in 0..500 {
        let len = rng.gen_range(MIN_LEN..MAX_LEN);
        let chain = build_random_chain(&mut rng, len);
        assert!(
            chain.verify().is_ok(),
            "freshly built random chain must verify"
        );
    }
}

#[test]
fn class_1_payload_hash_bit_flip_detection() {
    let mut rng = StdRng::seed_from_u64(1);
    let mut detected = 0usize;
    for _ in 0..TRIALS_PER_CLASS {
        let len = rng.gen_range(MIN_LEN..MAX_LEN);
        let mut chain = build_random_chain(&mut rng, len);
        let idx = rng.gen_range(0..len);
        flip_random_bit(&mut chain.events[idx].proof.payload_hash, &mut rng);
        if chain.verify().is_err() {
            detected += 1;
        }
    }
    let rate = detected as f64 / TRIALS_PER_CLASS as f64;
    println!(
        "[class 1: payload_hash bit-flip] detected {detected}/{TRIALS_PER_CLASS} (rate {rate:.4})"
    );
    // Sanity floor only — the honest number goes in the report regardless.
    assert!(rate > 0.99, "unexpectedly low detection rate: {rate:.4}");
}

#[test]
fn class_2_stored_event_hash_bit_flip_detection() {
    let mut rng = StdRng::seed_from_u64(2);
    let mut detected = 0usize;
    for _ in 0..TRIALS_PER_CLASS {
        let len = rng.gen_range(MIN_LEN..MAX_LEN);
        let mut chain = build_random_chain(&mut rng, len);
        let idx = rng.gen_range(0..len);
        flip_random_bit(&mut chain.events[idx].event_hash, &mut rng);
        if chain.verify().is_err() {
            detected += 1;
        }
    }
    let rate = detected as f64 / TRIALS_PER_CLASS as f64;
    println!(
        "[class 2: stored event_hash bit-flip] detected {detected}/{TRIALS_PER_CLASS} (rate {rate:.4})"
    );
    assert!(rate > 0.99, "unexpectedly low detection rate: {rate:.4}");
}

#[test]
fn class_3_assoc_parity_flip_detection() {
    let mut rng = StdRng::seed_from_u64(3);
    let mut detected = 0usize;
    for _ in 0..TRIALS_PER_CLASS {
        let len = rng.gen_range(MIN_LEN..MAX_LEN);
        let mut chain = build_random_chain(&mut rng, len);
        let idx = rng.gen_range(0..len);
        chain.events[idx].assoc_parity ^= 1;
        if chain.verify().is_err() {
            detected += 1;
        }
    }
    let rate = detected as f64 / TRIALS_PER_CLASS as f64;
    println!(
        "[class 3: assoc_parity flip] detected {detected}/{TRIALS_PER_CLASS} (rate {rate:.4})"
    );
    assert!(rate > 0.99, "unexpectedly low detection rate: {rate:.4}");
}

/// Entry deletion is split into two sub-classes on top of the overall rate,
/// because chain-internal verify() structurally cannot see a deletion of
/// the *last* event (a truncated valid prefix is itself a valid chain).
/// That is expected, not a bug, and it is exactly why checkpoint sealing
/// exists as a second layer (see docs/STATECHAIN_SPEC.md sections 6, 8).
#[test]
fn class_4_entry_deletion_detection() {
    let mut rng = StdRng::seed_from_u64(4);
    let mut detected = 0usize;
    let mut tail_deletions = 0usize;
    let mut tail_detected = 0usize;
    let mut nontail_deletions = 0usize;
    let mut nontail_detected = 0usize;
    for _ in 0..TRIALS_PER_CLASS {
        let len = rng.gen_range(MIN_LEN..MAX_LEN);
        let mut chain = build_random_chain(&mut rng, len);
        let idx = rng.gen_range(0..len);
        let is_tail = idx == len - 1;
        chain.events.remove(idx);
        let caught = chain.verify().is_err();
        if caught {
            detected += 1;
        }
        if is_tail {
            tail_deletions += 1;
            if caught {
                tail_detected += 1;
            }
        } else {
            nontail_deletions += 1;
            if caught {
                nontail_detected += 1;
            }
        }
    }
    let rate = detected as f64 / TRIALS_PER_CLASS as f64;
    let tail_rate = tail_detected as f64 / tail_deletions.max(1) as f64;
    let nontail_rate = nontail_detected as f64 / nontail_deletions.max(1) as f64;
    println!("[class 4: entry deletion] detected {detected}/{TRIALS_PER_CLASS} (rate {rate:.4})");
    println!(
        "[class 4a: tail-index deletion subclass] detected {tail_detected}/{tail_deletions} (rate {tail_rate:.4})"
    );
    println!(
        "[class 4b: non-tail deletion subclass] detected {nontail_detected}/{nontail_deletions} (rate {nontail_rate:.4})"
    );
    // Non-tail deletion must be caught essentially always (Reorder/Fork).
    assert!(
        nontail_rate > 0.99,
        "unexpectedly low non-tail deletion detection rate: {nontail_rate:.4}"
    );
    // Tail deletion detection at the chain-internal layer is expected to be
    // ~0: a truncated valid prefix is a valid chain by itself. This is the
    // documented finding, not an assertion failure.
}

#[test]
fn class_5_entry_reorder_detection() {
    let mut rng = StdRng::seed_from_u64(5);
    let mut detected = 0usize;
    for _ in 0..TRIALS_PER_CLASS {
        let len = rng.gen_range(MIN_LEN.max(2)..MAX_LEN);
        let mut chain = build_random_chain(&mut rng, len);
        let i = rng.gen_range(0..len);
        let mut j = rng.gen_range(0..len);
        while j == i {
            j = rng.gen_range(0..len);
        }
        chain.events.swap(i, j);
        if chain.verify().is_err() {
            detected += 1;
        }
    }
    let rate = detected as f64 / TRIALS_PER_CLASS as f64;
    println!("[class 5: entry reorder] detected {detected}/{TRIALS_PER_CLASS} (rate {rate:.4})");
    assert!(rate > 0.99, "unexpectedly low detection rate: {rate:.4}");
}

/// Splice two independently-built random valid chains at a random cut
/// point, forming a Frankenstein chain that mixes a genuine prefix from
/// chain A with a genuine (but foreign) suffix from chain B.
#[test]
fn class_6_chain_splice_detection() {
    let mut rng = StdRng::seed_from_u64(6);
    let mut detected = 0usize;
    for _ in 0..TRIALS_PER_CLASS {
        let len = rng.gen_range(MIN_LEN.max(2)..MAX_LEN);
        let chain_a = build_random_chain(&mut rng, len);
        let chain_b = build_random_chain(&mut rng, len);
        let cut = rng.gen_range(1..len); // guarantees real mixing, 1..len-1 inclusive
        let mut spliced_events = chain_a.events[..cut].to_vec();
        spliced_events.extend_from_slice(&chain_b.events[cut..]);
        let spliced = ObjectStatechain {
            object_kind: chain_a.object_kind,
            object_id: chain_a.object_id,
            events: spliced_events,
        };
        if spliced.verify().is_err() {
            detected += 1;
        }
    }
    let rate = detected as f64 / TRIALS_PER_CLASS as f64;
    println!("[class 6: chain splice] detected {detected}/{TRIALS_PER_CLASS} (rate {rate:.4})");
    assert!(rate > 0.99, "unexpectedly low detection rate: {rate:.4}");
}
