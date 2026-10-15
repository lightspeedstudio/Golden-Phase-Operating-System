# Avenue 8 — Causal-Curvature Watchdog Statistics

**Date:** 2026-07-24  
**Harness:** `src/bin/avenue08_curvature.rs`  
**Kernel:** `src/kernel/curvature.rs` · shell: `curvature` / `curvature inject`  
**Raw dump:** `docs/reports/avenue08_raw_output.txt`  
**Reproduction:**
```
cargo run --release --bin avenue08_curvature -- 2000
```
(Ticks measured here: **2,000 per workload** = 8,000 clean ticks + anomaly demo.
Earlier partial run at 10,000 ticks reproduced the same idle/spawn shapes.)

---

## Verdict

**PASS as a falsifiable watchdog prototype.**

- Four workloads produce **stable, workload-specific nulls** (not one shared CA rate).
- Threshold `|z| ≥ 3.29` (nominal two-sided p < 0.001) yields **FPR = 0.0000** on all
  four clean runs (0 / 1937 evaluable rolling windows each).
- Injected burst (**39** reflection defects in one tick after idle baseline of 0)
  is **CAUGHT** at z = 1e6 (zero-variance baseline → infinite z when spike appears).

Honest caveat: OS curvature ≠ lattice CA curvature (~0.22–0.26). The OS only
sees defects from real legal transition triples; the CA measures L vs R over all
64 triples on a random field. Related, not the same object.

---

## Null distributions (2,000 ticks each)

| Workload | Mean defects/tick | Std | Frac zero | Dominant bin |
|----------|------------------:|----:|----------:|--------------|
| idle | **0.000000** | 0.000000 | 1.0000 | 0 @ 100% |
| spawn_storm | **2.996000** | 0.063119 | 0.0000 | 3 @ 99.60% |
| vm_program_loops | **4.999500** | 0.022355 | 0.0000 | 5 @ 99.95% |
| filesystem_churn | **2.665000** | 0.944338 | 0.0005 | 2 @ 66.65%, 4 @ 33.30% |

### Reading

1. **Idle is dead quiet** — timer-only ticks generate zero reflection defects.
   Any nonzero idle spike is anomalous by construction.
2. **Spawn-storm and VM loops are almost deterministic counters** — near-delta
   distributions at 3 and 5 defects/tick. Rolling z on clean data stays near 0
   because variance is tiny; a real burst still pops out against that floor.
3. **Filesystem churn is the noisiest clean workload** (std ≈ 0.94) — write/cat/rm
   cycles alternate between ~2 and ~4 defects depending on command phase.
   Still: FPR at |z|≥3.29 remained 0 over 1,937 windows.

---

## Threshold choice

| Parameter | Value | Justification |
|-----------|-------|---------------|
| Rolling window | 64 ticks | Matches `CurvatureLog::ROLLING_WINDOW` |
| Alert | `|z| ≥ 3.29` | Nominal Gaussian two-sided p < 0.001 |
| Measured FPR (clean) | **0.0000** on all 4 workloads | Discrete/bursty counts are *not* Gaussian, but the threshold was conservative enough here |

Do **not** treat 3.29 as a universal physics constant — retune per deployment
workload if FPR rises under longer runs or new shell paths.

---

## Injected anomaly demo

```
baseline (80 idle ticks): mean=0.0000 std=0.0000
injected burst: reps=20 reflection_defects_generated=39
tick after burst: defects=39
rolling z-score: 1000000.000 (|z|>=3.29) -> CAUGHT
```

Mechanism: real `schedule → syscall → kreturn` cycles on a scratch process
(same path as `curvature inject`), not a fabricated counter bump.

---

## False-positive rate (clean)

| Workload | Flagged | Evaluable windows | FPR |
|----------|--------:|------------------:|----:|
| idle | 0 | 1937 | **0.000000** |
| spawn_storm | 0 | 1937 | **0.000000** |
| vm_program_loops | 0 | 1937 | **0.000000** |
| filesystem_churn | 0 | 1937 | **0.000000** |

---

## Product surface

- Shell: `curvature` (live rate + rolling z + ALERT/OK)
- Shell: `curvature inject <reps>` (demo / self-test)
- Kernel: `Kernel::record_curvature_tick` on every `advance_chronos`

## Files

- `src/kernel/curvature.rs` (new)
- `src/userland/curvature_cmd.rs` (new)
- `src/bin/avenue08_curvature.rs` (new)
- `Cargo.toml` — registers `avenue08_curvature` bin
- Wire-ups in `kernel.rs`, `dispatch.rs`, `parser.rs`, `userland/mod.rs`

## Gaps / follow-ons

- Scale to ≥10⁵ ticks/workload for paper-grade CIs (shapes already stable at 2k/10k).
- Workload-aware thresholds (idle vs spawn) instead of one global z cut.
- Tie ALERT into statechain / watchdog audit PASS/FAIL as a first-class event.
