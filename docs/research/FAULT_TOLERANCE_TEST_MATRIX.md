# Phase-State Fault Tolerance — Test Matrix

**Branch:** `research/phase-state-fault-tolerance`
**Status:** Gates 1–4 implemented; unit-tested on host + folded into `system selftest`. Thresholds
are starting values to be tuned against live hardware distributions.

Golden Phase OS carries its fault tolerance as a **phase-syndrome detector** running beside the
primary kernel — it compares the observed state against the *mathematically permitted* state, not
merely "did a process crash." Pipeline:

```
predict → measure → syndrome → confirm over time → identify bad estimate → restore verified state
```

All hot-path math is **fixed-point integer** (16-bit "turn" phase; `65536 = 360°`), so it runs on
bare metal. Code: `src/kernel/fault_syndrome.rs` (core + TMR), `src/userland/fault_cmd.rs` (the
`fault` command), and the "Fault Tolerance" area in `system selftest`.

## The canonical-equation binding

The generic detector is bound to the **actual Golden Phase equation** — this is the point.

| Symbol | Golden Phase reality | Source |
|---|---|---|
| `FK` (predict) | discrete `Φ' = compose(Φ, operand)`; continuous `phi' = phi + delta_phi` | `gphase_core::compose`, `StateClock` |
| `I1` | the observed transition equals the predicted `compose` result | `gphase_core::compose` / `fdir::verify_compose_step` |
| `I2` | **coherence** (Theorem 1): a mismatched value must stay in the reflection orbit `{v, 3−v}` | `gphase_core::reflect`, `topology_status` |
| `ϕ2 / ϕ3` | StateClock Kairos/atlas phase, as 16-bit turns | `stateclock/clock.rs` |
| `algebra_root` / `chain_root` | `equation_root` / `kernel_root` (low 64 bits) | `statechain` |
| `ẑ(P) / ẑ(G) / ẑ(C)` | live kernel / PhaseGuard reconstruction / last verified checkpoint | kernel / phaseguard / checkpoint |

## Syndrome → health

`D² = Σ wᵢ·qᵢ² + w_alg·[alg≠] + w_chain·[chain≠] + w_I1·[¬I1] + w_I2·[¬I2]` (all integer), then:

| D² | Health |
|---|---|
| `< 100` | **VALID** |
| `100 – 399` | **SUSPECT** |
| `400 – 899` | **FAULT** |
| `≥ 900` | **ISOLATE** |

A single noisy sample never triggers recovery: `FaultAccumulator` low-passes (`A = 7A/8 + D²`) and
requires 3 consecutive faults — except a **chain-root mismatch**, which is isolated immediately.

## Fault-class × detector matrix (`fault run`)

Each class is injected against a clean predicted transition (`compose(Φ1,Φ2)=Φ3`):

| Class | I1 (compose) | I2 (coherence) | chain | phase | tick | verdict | recovery |
|---|:--:|:--:|:--:|:--:|:--:|---|---|
| **clean** | ok | ok | ok | ok | ok | VALID | keep-primary |
| **compose** (wrong result, in-orbit) | ✗ | ok | ok | ok | ok | fault-scored | restore-from-guard |
| **coherence** (out-of-orbit value) | ✗ | ✗ | ok | ok | ok | ISOLATE | restore-from-guard |
| **chain-tamper** (root mutated) | ok | ok | ✗ | ok | ok | ISOLATE (immediate) | restore-from-guard |
| **phase-drift** (large phase error) | ok | ok | ok | ✗ | ok | FAULT | restore-from-guard |
| **tick-skip** (missed transitions) | ok | ok | ok | ok | ✗ | FAULT | restore-from-guard |

The **coherence** row is the mathematically special one: a value outside the reflection orbit is
*impossible* under the real algebra, so it is the strongest fault signal — exactly the redundancy
the phase algebra gives for free (Theorem 1).

## Triple modular redundancy + recovery

Three independent estimates of the same state: primary (live kernel), PhaseGuard (cosine-law
reconstruction from the Kairos tick), and the last verified checkpoint. If two agree within
tolerance and the third is the outlier, restore from the agreeing pair; the corrected phase is
their **circular mean** (integer shorter-arc midpoint). No trustworthy majority ⇒ halt safely.

## Honesty notes (the PhaseGuard rule)

- Thresholds `T1=100 / T2=400 / T3=900` and `AGREE_TOL=64` are **starting values**. They must be
  tuned against measured clean-workload distributions on hardware; never lower a threshold to
  manufacture a catch.
- `system selftest` asserts only what is proven: clean→VALID, an injected coherence break→ISOLATE,
  and the TMR restore decision. It does not claim tuned false-positive rates yet.

## Verification status

- `src/kernel/fault_syndrome.rs`: **11/11** unit tests pass on host (wrap seam, FK=compose,
  I1/I2 violations, chain-isolate, debounce, TMR agree/outlier/no-majority, circular mean).
- `system selftest` "Fault Tolerance" area: PASS on host; pending an on-board `system selftest`
  capture + a `fault run` transcript (blocked only on the `D:` BitLocker unlock for bootgen).
