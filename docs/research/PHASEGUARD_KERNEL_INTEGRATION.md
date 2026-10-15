# PHASEGUARD Kernel Integration

**Docket:** CAREGAGA-GPOS-2026-001  
**Gate:** 4 (stateclock-bound squeeze prediction)  
**Feature:** `phaseguard-research`  
**Status:** Research — cosine squeeze law remains a **tested hypothesis** until live physical squeeze samples exist.

Reproduce (host):

```bash
cargo test --features phaseguard-research phase_disk
cargo test --features phaseguard-research phaseguard_live
cargo test
```

QEMU / shell (feature on):

```text
phaseguard status
phaseguard sample   # lawful observed=predicted from live kairos_tick
phaseguard trace
```

---

## Stateclock-Bound Squeeze Prediction

### Why PhaseGuard uses the existing stateclock

Golden Phase OS already has one authoritative dual clock (`StateClock`: Chronos + Kairos). Gate-3B proved that a closed-form predictor

```text
s_expected(n) = s* · cos(n · π/5)
```

can close the falsified-squeeze blind spot when `n` is a dual-clock position. Gate 4 binds `n` to the **live** kernel clock so PhaseGuard cannot invent a second time base that races the OS.

### Why a separate clock would be unsafe

A free-running PhaseGuard timer, wall-clock stamp, or independent oscillator could:

- disagree with Kairos/Chronos ordering,
- miss skips/duplicates relative to real transitions,
- let a forged observation look “on time” against the wrong base.

PhaseGuard is therefore a **read-only observer** of `kernel.clock`. It never increments, resets, or retunes the stateclock, never blocks a transition, and never forces the scheduler to follow the cosine law.

### Data path

```text
kernel transition request
→ capture read-only stateclock snapshot
→ derive logical phase position (kairos_tick)
→ calculate expected squeeze
→ compare expected and observed squeeze (if a sample exists)
→ evaluate tick/epoch invariants
→ record PASS / WARNING / FAULT
→ return without changing kernel execution
```

Observation point: [`record_kairos_on_kernel`](../../src/stateclock/clock.rs) — after the authoritative `kairos_tick` advances, PhaseGuard takes a read-only snapshot. Process phase changes stash `(prev, current, proposed)` in `kernel.phaseguard_pending` so the same hook records transition endpoints without a second clock.

### Ten-position phase mapping (36° step)

```text
logical_tick  = StateClock.kairos_tick     # event / transition ordering
phase_index   = logical_tick mod 10
theta         = phase_index × (π/5)        # 36 degrees
expected      = s* × cos(theta)
```

| Symbol | Source |
|--------|--------|
| `s*` | Gate-1 `canonical_squeeze_coordinate()` from α, φ (`phase_disk`) |
| `logical_tick` | **Kairos** tick (not raw hardware timer, not Chronos fine-step) |
| `sector_id` on StateClock | φ-atlas sector — **not** used as `phase_index` |

Chronos still advances φ/`sector_id` for the 10-sector atlas (`delta_phi = π/(5·10)`). PhaseGuard’s Gate-3B projection deliberately uses Kairos `mod 10` so one observation step = one phase-disk step, matching the host Gate-3B `sequence` model.

### Epoch and wraparound

- `StateClock.epoch` starts at 0 and increments on `resync`.
- Ordinary wrap: `kairos_tick` 9→10 yields `phase_index` 9→0 without epoch change.
- Epoch change is recorded as `EpochChanged` (WARNING), distinct from wrap.
- Replay of an earlier `(epoch, tick)` pair is `ReplayEpochTick` (FAULT).

### Predictor independence

- Host reference: `predict_squeeze(sequence)` (Gate 3B).
- Live form: `predict_squeeze_from_logical_tick(kairos_tick)` — same math via explicit `mod 10`.
- Expected value never reads observed squeeze; tampering observed `s` cannot change the prediction.

### Process transitions without a squeeze sensor

Every Kairos advance is observed with `observed_squeeze = None`. Tick/epoch anomalies are scored against the full logical stream; squeeze-mismatch is **not** forced. This avoids pretending the cosine law is measured on every schedule/wake.

`phaseguard sample` prints a **preview** row with `observed = predicted(live kairos_tick)` without mutating the observer (the shell command then advances Kairos normally, which PhaseGuard observes once).

### QEMU / shell evidence

Reproduce:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\boot-qemu-phaseguard-gate4.ps1
```

Artifacts:

- `docs/research/PHASEGUARD_GATE4_QEMU_SERIAL.log` — live UART capture
- `docs/research/PHASEGUARD_GATE4_TRACE_EVIDENCE.txt` — host deterministic trace

With `phaseguard-research` enabled, UART/shell lines look like:

```text
[PhaseGuard] epoch=… tick=… phase_idx=… theta_deg=… prev=… proposed=… observed=… predicted=… diff=… verdict=… fault=…
```

Gate-4 QEMU result: consecutive `kairos_tick` 1…N all `verdict=Pass` / `fault=None` with `observed=NaN` (no physical squeeze sensor). Lawful status: `warn=0 fault=0`. Kernel boot, scheduler, VM, PhaseFS, statechain, watchdog unchanged — PhaseGuard only appends observer trace.

`phaseguard sample` prints a preview with `observed=predicted` from the live tick (does not mutate the observer).

### Mismatches and limitations

Gate 4 asks: *does the live stateclock naturally supply ordering for the 36° squeeze prediction?*

If lawful samples mismatch, preserve evidence and classify:

- wrong tick input (Chronos vs Kairos),
- wrong modulo mapping,
- transition-relative vs global phase,
- epoch/resync handling,
- or an **invalid squeeze-law hypothesis**.

Do not weaken tolerances to obtain a pass.

### Cosine law status

The identity `s = s* cos(n·π/5)` remains a **research hypothesis** validated on host Gate-3B synthetic traces. It is **not** yet a sealed physical law of live process squeeze. Kernel integration proves clock binding and observer honesty first.

### Acceptance (Gate 4)

```text
default suite, feature OFF (--lib):       42/42
PhaseGuard research suite:                PASS (phaseguard_live 13/13, gate4 5/5)
Gate-3B fault detection:                  10/10
unseen valid host false positives:        0
live QEMU valid-trace false positives:    0  (warn=0 fault=0)
second clock introduced:                  NO
kernel execution modified:                NO
stateclock read advances clock:           NO
observe overhead (host, debug):           ~34 µs/observation
```

**Unresolved:** physical squeeze is not yet sampled on live transitions (`observed=NaN`); cosine law vs live hardware squeeze remains open. Sparse process-only observation was rejected because Kairos also advances on commands/boot — that produced skip false positives (transition-relative vs global phase); fixed by observing every Kairos advance.
