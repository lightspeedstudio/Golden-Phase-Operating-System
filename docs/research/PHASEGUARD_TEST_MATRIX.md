# PHASEGUARD Fault-Injection Test Matrix (Gate 3)

**Docket:** CAREGAGA-GPOS-2026-001
**Gate:** 3 of 8 — host fault injection + read-only detection rates
**Status:** SIMULATED (host). Read-only Mode-A witness; no kernel state touched.

Reproduce:

```bash
cargo test --features phaseguard-research phase_disk                                          # 12 tests incl. Gate 3
cargo test --features phaseguard-research phase_disk::tests::dump_detection_matrix -- --ignored --nocapture
```

---

## 1. Model

A phase-disk **scan** is a sequence of `PhaseObservation { sequence, sector, s,
branch }`. A healthy scan advances the sector by +1 mod `SECTOR_COUNT` (=10),
strictly increments `sequence`, keeps `branch = Positive`, and keeps the squeeze
product `A(s)·B(s) = √α`.

The **Mode-A witness** is READ-ONLY. It walks adjacent observations and returns
the first named violation of four structural invariants: branch stability,
squeeze-product finiteness, strictly-increasing sequence, and +1 sector
continuity.

## 2. Fault taxonomy and results (scan_len = 40, fault at index 20)

| Fault | Detected | Witness verdict |
|---|---|---|
| None (control) | ✅ clean | Ok |
| Skipped | ✅ | SectorDiscontinuity |
| Duplicated | ✅ | SectorDiscontinuity |
| Reversed | ✅ | SectorDiscontinuity |
| Stale | ✅ | SequenceStalled |
| Replay | ✅ | SequenceRegressed |
| Stall | ✅ | SequenceStalled |
| WrongSqueeze (NaN / breaks product) | ✅ | SqueezeInvariantBroken |
| **SqueezeFalsified (plausible finite s)** | ❌ **MISSED** | Ok |
| BranchFlip | ✅ | BranchFlipped |
| Corruption | ✅ | SectorDiscontinuity |

**Detection rate: 9 / 10 injected fault classes.**

## 3. The honest blind spot

`SqueezeFalsified` replaces the squeeze coordinate with a *plausible finite* wrong
value (`s += 0.37`). The witness does **not** catch it, and this is not a bug in
the witness — it is structural:

> The squeeze product `A(s)·B(s) = √α` is conserved for **every** `s` (Gate 1).
> A product-only witness therefore has **no** invariant that constrains the
> squeeze *value*. Any internally-consistent falsification of `s` is invisible.

This ties directly to Gate 2: the squeeze axis is a free degree of freedom on the
product manifold, so structural checks over that manifold cannot pin the value.

## 4. What would be required to close the blind spot (NOT claimed as done)

Detecting `SqueezeFalsified` would require an **independent** constraint on `s` —
e.g. binding `s` to the observed sector/`epsilon` geometry, or committing `s` into
the statechain digest so a later value must match a signed prior (Gate 4+). None
of that is implemented or proven here; it is noted only to bound the claim.

## 5. Gate-3 verdict

- The Mode-A structural witness **detects 9 of 10** fault classes deterministically,
  including all sequence/sector/branch faults and non-finite squeeze corruption.
- It has **one honest, explained blind spot**: falsification of the squeeze value
  within the product manifold. A witness that claimed 10/10 here would be
  over-fitted to its own invariants; this result is reported *because* it is not.
- No claim is made that this witness detects anything the existing statechain does
  not — that comparison is Gate 4+ (kernel shadow integration).

Next: **Gate 4** — wire a read-only shadow witness into the kernel behind
`phaseguard-research`, confirm boot/scheduler/VM/PhaseFS/statechain stay stable and
all prior tests still pass, on host + QEMU (+ PZ7020 later).
