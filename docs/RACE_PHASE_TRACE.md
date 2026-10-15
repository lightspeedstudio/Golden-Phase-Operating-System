# Race Phase Trace — Proof of Concept

## What was built

| Component | Path | Role |
|-----------|------|------|
| Race program | `kairos/scripts/race_counter.py` | Two threads RMW-increment a shared counter **without** a lock; emits JSONL events (`seq`, `thread`, `op`, `value_before`, `value_after`, `ts`) plus a `run_summary` line |
| Phase trace | `kairos/scripts/race_phase_trace.py` | Ingests JSONL bytes → `bytes_to_phase`, `gpl_checksum`, `check_topology` (existing GPL modules); adds ground-truth conflict detection and rolling heuristic anomalies |
| Sample outputs | `kairos/scripts/samples/race_events.*` | Captured race run + `.race_phase.json` / `.race_phase.html` |

The race program uses a **spin delay** (`--spin-ns`) between read and write to widen the lost-update window. With `--trials N`, it stops at the first manifesting race and copies that log to `--out`.

## How to run

From repo root:

```bash
# 1. Generate event log (intentional race)
python kairos/scripts/race_counter.py \
  --out kairos/scripts/samples/race_events.jsonl \
  --iterations 3000 --spin-ns 1200 --trials 5

# 2. Phase-encode log and analyze
python kairos/scripts/race_phase_trace.py kairos/scripts/samples/race_events.jsonl
```

Outputs:

- `race_events.jsonl` — structured event stream
- `race_events.race_phase.json` — full report
- `race_events.race_phase.html` — charts (histogram, rolling reflection/entropy/chi drift)

Options:

- `race_counter.py`: `--threads`, `--iterations`, `--spin-ns`, `--trials`
- `race_phase_trace.py`: `--window-stride` (default 16), `--max-windows` (default 200)

## Ground truth (event log)

The trace script marks **lost-update conflicts** when two or more threads write after reading the same `value_before` (duplicate base snapshot). The `run_summary` line records `expected_count`, `actual_count`, and `lost_updates`.

Interleaved thread bursts (alternating `thread` ids) are also counted as contention proxies.

## Phase heuristics (honest, not magic)

On rolling cumulative log prefixes (sampled every N events):

1. **Reflection rate** — fraction of sliding triples classified `reflection` on a trailing local phase window
2. **Transition entropy** — Shannon entropy of adjacent phase pairs in that window
3. **Chi drift** — absolute change in GPL layer-2 `chi` between consecutive windows

Windows more than one standard deviation above the run mean for any metric are flagged as phase anomalies. Overlap with ground-truth conflict sequence numbers is reported.

Phase encoding fingerprints **UTF-8 log bytes**. It does not observe CPU memory directly.

## Results (2026-07-07 run)

### Race run (`--iterations 3000 --spin-ns 1200`)

```
trial 0: expected=6000 actual=5847 lost=153 race=True
events      : 12001
conflicts   : 67 (duplicate value_before groups)
anomalies   : 48 phase windows flagged
correlation : overlap=0.0962 (10/104 anomalies near conflict seqs)
verdict     : weak_or_no_correlation
```

Full-log topology: `reflection_rate ≈ 0.137` (843183 / 6136802 sliding triples).  
GPL checksum: `acc=Phi3`, `chi≈24739.11`.

### What correlated

- **Event log ground truth** reliably identifies lost updates when the race manifests (`lost=153`).
- **Interleave bursts** align with contention periods (high thread alternation in JSONL order).
- A minority of phase anomalies (**~10% overlap**) landed near known conflict sequence numbers — mostly `entropy_spike` and occasional `reflection_spike` / `chi_drift_spike`.
- The **global** phase fingerprint (histogram, total reflection count, final chi) characterizes the *entire log* but does not localize races by itself.

### What did not correlate strongly

- Topology reflection spikes did **not** consistently align with race windows; many conflicts left no flagged anomaly.
- Checksum drift between coarse windows is dominated by log growth, not race events alone.
- Phase trajectory alone is **not** a dependable race detector for this POC — it encodes serialized events, so races and non-races with similar interleaving produce similar byte patterns.

## Verdict

**Partial validation of the hypothesis:**

- ✅ Anything observable (here, JSONL bytes) can be captured to phase states and analyzed as the process progresses.
- ✅ Ground-truth race windows are recoverable from the structured event log.
- ⚠️ GPL topology / reflection / chi heuristics show **weak localization** of race windows (~9.6% anomaly overlap in the main run). They fingerprint log structure; they do not replace happens-before analysis.
- ❌ Identifying the race from topology **alone**, without event semantics, is **not demonstrated** by this POC.

## Related

- `kairos/scripts/ai_trace_phase.py` — same encoder/topology pipeline for text artifacts
- `kairos/docs/AI_PHASE_TRACE.md` — interpretive guide for phase traces
- `golden_phase_runtime/protocol/encoder.py`, `topology_checker.py`, `checksum.py` — authority modules (not reimplemented)
