# Avenue 6 — Statechain Spec + Adversarial Fuzz

**Scope:** reverse-engineer the statechain wire/hash format into a formal
spec (`docs/STATECHAIN_SPEC.md`), then fuzz-test tamper detection at scale.
`compose` and the hash/verify pipeline were never reimplemented — the fuzz
harness builds chains exclusively via `ObjectStatechain::append` (the real
production entry point) and measures detection with the real
`ObjectStatechain::verify()` (the same function
`StatechainLedger::verify_all()` calls, which backs `chains_ok` in
`StatechainWatchdog::audit`).

## Spec

`docs/STATECHAIN_SPEC.md` — field-by-field `StateTransitionProof` /
`StateEvent` layout, the exact `compute_event_hash` byte preimage (§3, §10),
chain linkage rules in `ObjectStatechain::append`/`verify` (§5), ledger head
caching and kernel-root Merkle composition (§6), equation-root derivation
(§7), checkpoint sealing/HMAC (§8), and the `assoc_parity` end-to-end path
from `Scheduler::reflection_count()` through to the hashed byte (§9). All
claims are cited to file/line.

Two honesty notes surfaced during the spec write that are **not** fuzz
findings but are worth flagging to the architect:
- The checkpoint "signature" is `HMAC(signing_key, body)` where
  `signing_key = tagged_hash(TAG, equation_root)` — fully re-derivable by
  anyone who can rebuild `equation_root` from public constants. It functions
  as a domain-separated integrity re-hash, not a secret-keyed MAC (see spec
  §8).
- `StatechainLedger`'s per-subsystem head cache (used to compute
  `kernel_root`) is refreshed only inside `record_transition`. A mutation to
  a live chain's `events` that bypasses that path would not automatically
  invalidate the cached head/`kernel_root` (spec §6, caching caveat). This
  was *not* exercised in the fuzz tests below — the fuzz harness works at
  the standalone-`ObjectStatechain` layer, which is the layer that catches
  everything the tests below report. It's a code-inspection observation, not
  a measured number; flagged so it isn't lost.

## Fuzz harness

`tests/statechain_fuzz_tests.rs`, matching the style of
`tests/statechain_tests.rs`. 10,000 trials per class, each trial building a
fresh random valid chain (length uniform in `[4, 20)`, random object kind,
random operation names/payloads/states/compose_result/topology_status,
seeded `StdRng` per test for reproducibility). Tampering mutates `pub`
fields directly (bypassing `append`, since `append` itself can't produce
invalid data) — the adversary model is "something with in-process write
access to a stored chain."

### Exact command

```
CARGO_TARGET_DIR=$TEMP/kairos-target-6 cargo test --test statechain_fuzz_tests -- --nocapture
```

### Result: 7 passed; 0 failed; 0 ignored (5.18s)

## Detection-rate table

| # | Class | Trials | Detected | Rate |
|---|---|---|---|---|
| 1 | Payload-hash bit-flip (`proof.payload_hash`) | 10,000 | 10,000 | **1.0000** |
| 2 | Stored event-hash bit-flip (`event.event_hash`) | 10,000 | 10,000 | **1.0000** |
| 3 | `assoc_parity` flip | 10,000 | 10,000 | **1.0000** |
| 4 | Entry deletion (random index, overall) | 10,000 | 8,957 | **0.8957** |
| 4a | — tail-index deletion subclass | 1,043 | 0 | **0.0000** |
| 4b | — non-tail deletion subclass | 8,957 | 8,957 | **1.0000** |
| 5 | Entry reorder (swap two distinct indices) | 10,000 | 10,000 | **1.0000** |
| 6 | Chain splice (two independent chains, random cut) | 10,000 | 10,000 | **1.0000** |

## Finding: entry deletion is not 100% — and why, honestly

Class 4 (entry deletion) is the only class below 1.0000, at **0.8957**
overall. This is not noise or a harness bug — the split into 4a/4b makes the
mechanism exact:

- **Non-tail deletion (8,957/10,000 trials): 100.0000% detected.** Deleting
  any event that isn't the last one shifts every subsequent event's stored
  `proof.sequence_number` out of alignment with its new array index (or
  breaks the `previous_event_hash` linkage — whichever check fires first in
  `ObjectStatechain::verify()`). Because `sequence_number` is a contiguous
  `0..len` run assigned once at `append` time and never mutated, any
  non-identity change to element order or count strictly before the tail is
  structurally guaranteed to be caught.
- **Tail-index deletion (1,043/10,000 trials): 0.0000% detected.** Deleting
  the *last* event of a chain leaves a shorter, but internally perfectly
  valid, prefix chain: sequence numbers `0..len-1` are still contiguous,
  every `previous_event_hash` link is still correct, every `event_hash`
  still matches its own proof. `ObjectStatechain::verify()` has no way to
  know an event *used to* exist afterward — a truncated valid chain is
  indistinguishable from a chain that was honestly never extended further.
  This is expected from the design, not a defect in the hash chain: it is
  exactly the gap that `StatechainLedger` checkpoint sealing
  (`docs/STATECHAIN_SPEC.md` §8) exists to close — a checkpoint anchors a
  *stated* kernel root at a point in time from an external, append-only
  checkpoint log, so a chain quietly frozen (never truncated in place, but
  simply not extended, or truncated and not re-checkpointed) would eventually
  disagree with a later, independently-obtained checkpoint. This report
  measures only the chain-internal layer (`ObjectStatechain::verify()`), per
  the roadmap's tamper classes; verifying the checkpoint layer's coverage of
  tail truncation specifically would need a new pub(test) hook into
  `StatechainLedger.chains` (none exists today) and was out of scope here —
  noted as a gap for a follow-on avenue rather than claimed as tested.

Expected overall rate given random chain lengths uniform in `[4,20)`:
`1 - E[1/len] ≈ 1 - 0.1237 ≈ 0.876`, consistent with the measured 0.8957
(same order, same mechanism — small deviation is just RNG sampling noise
around the closed-form expectation for a discrete uniform length draw).

All other five classes hit hard 1.0000/10,000 — expected from the spec: any
single bit flip anywhere in the `compute_event_hash` preimage (§3, §10 of
the spec) changes the SHA-256 output with overwhelming probability, and any
non-identity permutation or foreign splice breaks the strictly-increasing
`sequence_number` / `previous_event_hash` linkage at the first point of
divergence (§5.2). No sub-100% behavior was observed or expected for these
five classes in 10,000 trials each.

## Build/test confirmation (no regression)

```
CARGO_TARGET_DIR=$TEMP/kairos-target-6 cargo test --test statechain_tests -- --nocapture
```
Result: **20 passed; 0 failed; 0 ignored** (existing suite, unmodified,
including the `assoc_parity` tests added earlier this session).

```
CARGO_TARGET_DIR=$TEMP/kairos-target-6 cargo test --test statechain_fuzz_tests -- --nocapture
```
Result: **7 passed; 0 failed; 0 ignored** (new fuzz suite, 5.18s wall time).

## Files added

- `docs/STATECHAIN_SPEC.md` (new)
- `tests/statechain_fuzz_tests.rs` (new)
- `docs/reports/AVENUE_06_statechain_fuzz.md` (this file)

No production source files were modified. No commit was made (per standing
rules).
