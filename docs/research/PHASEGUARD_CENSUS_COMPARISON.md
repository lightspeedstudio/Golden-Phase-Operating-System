# PHASEGUARD Census Comparison (Gate 2)

**Docket:** CAREGAGA-GPOS-2026-001
**Gate:** 2 of 8 — canonical 16/4/64 census vs. phase-disk branch structure
**Status:** SIMULATED (host). Census taken **bit-exact** from
`gphase_core::topology_status` (the sole authority); phase-disk features never
recompute the algebra.

Reproduce:

```bash
cargo test --features phaseguard-research phase_disk                                   # 8 tests incl. Gate 2
cargo test --features phaseguard-research phase_disk::tests::dump_census -- --ignored --nocapture
```

---

## 1. The question

Gate 1 showed the phase-disk model *reproduces* the canonical roots (σ=+1) and
produces the imaginary inner pair (σ=−1). Gate 2 asks the sharper question:

> Does the phase-disk branch structure — specifically **inner / ib involvement**,
> where the inner states {Φ1, Φ2} are the imaginary-branch pair — **explain**
> which 12 of the 64 associator triples are reflections, or does it only
> reproduce the 52/12/0 census?

`gphase_core::threshold_energy` already encodes the split: {Φ1,Φ2} → 0 (inner /
imaginary branch), {Φ0,Φ3} → 1 (real rail). So "ib involvement" is directly
testable as a predicate over each triple.

## 2. Census reproduction (must be exact)

| | count |
|---|---|
| Same | **52** |
| Reflection | **12** |
| Error | **0** |

Matches `gphase_core` and `recursive_shell::associator_census` bit-exact.
(test `census_matches_canonical_52_12_0`).

## 3. Contingency — reflections vs. inner structure

**By inner_count** (number of {Φ1,Φ2} among the three positions):

| inner_count | 0 | 1 | 2 | 3 |
|---|---|---|---|---|
| **Reflection** | 0 | 0 | **8** | **4** |
| Same | 8 | 24 | 16 | 4 |

**By middle element** `b`:

| | middle inner (b∈{Φ1,Φ2}) | middle rail (b∈{Φ0,Φ3}) |
|---|---|---|
| **Reflection** | **12** | 0 |
| Same | 20 | 32 |

## 4. Predicate scores (is any inner-feature a perfect classifier?)

| predicate | TP | FP | FN | necessary | sufficient | **perfect** |
|---|---|---|---|---|---|---|
| `middle_inner` | 12 | 20 | 0 | ✅ | ❌ | ❌ |
| `inner_count ≥ 2` | 12 | 20 | 0 | ✅ | ❌ | ❌ |
| `inner_count ≥ 1` | 12 | 44 | 0 | ✅ | ❌ | ❌ |
| `all_inner (=3)` | 4 | 4 | 8 | ❌ | ❌ | ❌ |

(test `no_single_inner_predicate_perfectly_explains_the_12`.)

## 5. The 12 reflection triples

```
(Φ1,Φ1,Φ2)  (Φ1,Φ1,Φ3)  (Φ1,Φ2,Φ2)  (Φ1,Φ2,Φ3)
(Φ2,Φ1,Φ1)  (Φ2,Φ1,Φ3)  (Φ2,Φ2,Φ1)  (Φ2,Φ2,Φ3)
(Φ3,Φ1,Φ1)  (Φ3,Φ1,Φ2)  (Φ3,Φ2,Φ1)  (Φ3,Φ2,Φ2)
```

Observed necessary conditions (all 12 satisfy, but not unique to them):
`b ∈ {Φ1,Φ2}` (inner middle), `inner_count ≥ 2`, and `a ≠ Φ0`.

## 6. Gate-2 verdict — REPRODUCES, does NOT EXPLAIN

- **Positive:** the phase-disk inner/ib structure is a genuine **necessary
  condition** — every one of the 12 reflections has an inner middle element and
  inner_count ≥ 2. There are **zero** reflections outside that band (0 false
  negatives for both predicates).
- **Negative (the falsification):** it is **not sufficient**. The same inner
  signature is shared by **20 Same triples** (`middle_inner`) — the 12
  reflections are a strict 12-of-32 subset. **No single phase-disk inner-feature
  perfectly classifies the 12.** Pinning down exactly which 12 requires the full
  compose algebra, not the branch labels alone.

> **Conclusion (Gate 2):** the oscillator/branch structure **reproduces** the
> 52/12/0 census and provides a real *necessary filter* (32 candidates), but it
> does **not explain** the reflection set on its own. ib-involvement predicts
> where reflections *can* occur, not where they *do*. This is a partial,
> honestly-bounded result — not a mechanism, and not yet evidence that PhaseGuard
> detects anything the existing algebra does not.

Next: **Gate 3** — host fault injection (skipped / duplicated / reversed / stale /
replay / stall / wrong-squeeze / branch-flip / corruption) and measured detection
rates, including the undetected cases.
