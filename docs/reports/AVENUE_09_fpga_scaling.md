# Avenue 9 — FPGA phase-lattice scaling study

Target: iCE40UP5K, package sg48 (iCEBreaker board), OSS CAD Suite
(`C:\Tools\oss-cad-suite`). **No hardware was flashed** — this is a
synthesis / place-and-route / simulation study only, per the standing rule.
All numbers below are copy-pasted from real tool output (yosys / nextpnr-ice40
logs), not estimated. The physically-verified N=16 experiment
(`phase_lattice_uart.v`, `phase_lattice_top.v`,
`evidence/phase_lattice/PHYSICALLY_VERIFIED.txt`) is untouched by this study.

## Files added (avenue-specific names, nothing overwritten)

- `fpga/phase_lattice_uart_scale.v` — parameterized 1-D ring
  (`phase_lattice_uart_scale #(.N(...))`, top `phase_lattice_scale_top`).
  Reuses `compose_gate` and `lattice_cell` from `phase_lattice_uart.v`
  **unchanged** (read together at synth/sim time — compose is never
  reimplemented).
- `fpga/phase_lattice_2d_scale.v` — 2-D torus (`phase_lattice_2d_scale
  #(.M(...))`, top `phase_lattice_2d_top`). Reuses `compose_gate` from
  `phase_lattice_uart.v` unchanged; introduces `lattice_cell_2d` (a new
  wiring of the same primitive, since Avenue 2's 2-D rule is a genuinely
  different neighborhood shape than the 1-D ring's `lattice_cell`).
- `fpga/verify_lattice_scale.py` — Python reference extended to arbitrary N
  (sibling of `verify_lattice.py`, does not modify or import it).
- `fpga/tb_phase_lattice_scale.v` — iverilog testbench, N/GENS/CLK_GENS
  overridable via `-P`.
- `fpga/build-phase-lattice-scale.ps1` — build-loop driver (yosys + nextpnr,
  no icepack/flash needed for a resource study).
- `fpga/evidence/phase_lattice_scale/` — all logs, JSON reports, and
  `summary.csv` for every configuration below.

## Deliberate simplifications vs. the fixed N=16 experiment (documented, not hidden)

- Init field is `x[i] = i % 4` (not the N=16 experiment's hand-picked field)
  — any fixed, N-independent rule is fine for a resource/Fmax study, and
  `verify_lattice_scale.py` replicates it exactly.
- The defect count field is 3 decimal digits (was 2) since N can exceed 99.
- `GENS = 8` (was 40) to keep line buffers and simulation time modest; this
  only affects FSM iteration count, not per-cycle combinational cost.
- 2-D per-cell defect = **rule (a) vs rule (b) directly** (see below), not
  each rule against its own "mirror" as in `AVENUE_02_lattice_2d.md`'s
  software study. Chosen for compactness in hardware; documented here as a
  deliberate difference from Avenue 2's methodology, not a re-derivation of
  its coherence result.

## 1. 1-D ring: exact build commands

Single parameterized source, `chparam` overrides `N` per build:

```
yosys -l evidence\phase_lattice_scale\N<val>\yosys.log -p \
  "read_verilog uart_tx.v phase_lattice_uart.v phase_lattice_uart_scale.v; \
   chparam -set N <val> phase_lattice_scale_top; \
   synth_ice40 -top phase_lattice_scale_top -json evidence\phase_lattice_scale\N<val>\out.json"

nextpnr-ice40 --up5k --package sg48 \
  --json evidence\phase_lattice_scale\N<val>\out.json \
  --pcf icebreaker.pcf \
  --asc evidence\phase_lattice_scale\N<val>\out.asc \
  --report evidence\phase_lattice_scale\N<val>\report.json
```

Driver: `powershell -File fpga\build-phase-lattice-scale.ps1 -Skip2D`
(loops `N ∈ {16,32,64,128,256}`).

## 2. 1-D ring: resource / Fmax table (measured, from nextpnr logs)

| N | ICESTORM_LC used | / 5280 | util % | Fmax (final, MHz) | 12 MHz timing | Fits? | Build result |
|---|---|---|---|---|---|---|---|
| 16  | 767  | 5280 | 14% | 23.92 | PASS | YES | OK (matches original N=16 experiment's 762 LC within rounding — this build's 3-digit D field adds a few LUTs) |
| 32  | 1122 | 5280 | 21% | 18.18 | PASS | YES | OK |
| 64  | 1818 | 5280 | 34% | 14.30 | PASS | YES | OK |
| 128 | 3265 | 5280 | 61% | 12.01 | **PASS, 0.01 MHz margin** | YES | OK |
| 256 | 6429 | 5280 | **121%** | — (placement never completed) | — | **NO** | **FAILED**: `ERROR: Unable to place cell '...ICESTORM_LC', no BELs remaining to implement cell type 'ICESTORM_LC'` |

N=256's 6429-LC figure is yosys's pre-placement cell count (post-techmap,
before legalization); nextpnr aborts once it exceeds the 5280 BELs on the
device, so no post-place/route number exists for that row — reported
honestly as absent, not estimated.

**Important honest finding:** N=128 fits by area (61% LC utilization) but
Fmax = 12.01 MHz is a 0.01 MHz margin over the board's 12 MHz oscillator —
effectively the practical timing ceiling on this exact RTL/constraints, even
though area headroom would suggest more room. Area-limited and
timing-limited "max N" are different numbers (see §4).

## 3. 2-D torus: exact build commands and resource/Fmax table

```
yosys -l evidence\phase_lattice_scale\2d_M<val>\yosys.log -p \
  "read_verilog uart_tx.v phase_lattice_uart.v phase_lattice_2d_scale.v; \
   chparam -set M <val> phase_lattice_2d_top; \
   synth_ice40 -top phase_lattice_2d_top -json evidence\phase_lattice_scale\2d_M<val>\out.json"

nextpnr-ice40 --up5k --package sg48 \
  --json evidence\phase_lattice_scale\2d_M<val>\out.json \
  --pcf icebreaker.pcf \
  --asc evidence\phase_lattice_scale\2d_M<val>\out.asc \
  --report evidence\phase_lattice_scale\2d_M<val>\report.json
```

Driver: `powershell -File fpga\build-phase-lattice-scale.ps1 -Skip1D`
(loops `M ∈ {4,8}`, i.e. 16 and 64 torus cells).

Exact 2-D rule implemented per cell (`lattice_cell_2d` in
`phase_lattice_2d_scale.v`), matching the two rules named in
`AVENUE_02_lattice_2d.md`:
- rule (a), row-then-column: `mid=(W∘x)∘E; y_a=(N∘mid)∘S` — this is the
  next-state (analogous to using L as next-state in the 1-D module).
- rule (b), fixed von Neumann grouping: `y_b=((N∘x)∘S)∘((E∘x)∘W)`.
- on-chip defect bit = `(y_a != y_b)` — 9 `compose_gate` instances per cell
  (4 for y_a, 5 for y_b), vs. 4 per cell for the 1-D ring.

| M | cells (M²) | ICESTORM_LC used | / 5280 | util % | Fmax (final, MHz) | Fits? |
|---|---|---|---|---|---|---|
| 4 | 16 | 935  | 5280 | 17% | 21.11 | YES |
| 8 | 64 | 2663 | 5280 | 50% | 13.82 | YES |

## 4. Derived scaling law

**1-D ring**, linear regression over the 4 fitting points (N=16,32,64,128;
N=256 excluded — no valid post-route point):

```
LC(N) ≈ 22.3 · N + 405        (LC = ICESTORM_LC count)
```

Fit quality: pairwise per-cell deltas are 22.19, 21.75, 22.61 LC/cell across
the three doublings — flat, confirming genuine linearity (compose-gate cost
dominates; no super-linear routing blowup observed up to N=128).
`≈22.3 LC per lattice cell` (4 `compose_gate` instances per cell → ~5.6 LC
per `compose_gate`), `≈405 LC` fixed overhead (UART TX, FSM, line buffer,
decimal-defect-count divider).

**2-D torus** (only 2 data points — M=4,8 — so this is a much lower-confidence
fit than the 1-D one, reported as an order-of-magnitude estimate only):

```
LC(cells) ≈ 36.0 · cells + 359
```

≈36.0 LC per 2-D cell vs. ≈22.3 LC per 1-D cell — a **1.6×** per-cell cost
increase, consistent with the 9-vs-4 `compose_gate` count per cell (naive
gate-count ratio would predict 2.25×; the smaller observed ratio reflects
shared/optimized logic and roughly-constant fixed overhead being subtracted
out between just two points).

## 5. Largest lattice that fits UP5K

- **Area-only, linear extrapolation** (1-D fit, §4): `N_max ≈ (5280−405)/22.3
  ≈ 218` cells. This is an **extrapolation**, not a built/verified number —
  it also does not account for Fmax.
- **Practical (area + 12 MHz timing) verdict, from measured data alone**:
  N=128 is the largest *built, routed, and timing-passing* 1-D configuration
  (61% LC, Fmax 12.01 MHz — 0.01 MHz margin). N=256 fails outright on area
  (121% LC, unplaceable). Since Fmax dropped monotonically and near-linearly
  in 1/N-ish fashion (23.92 → 18.18 → 14.30 → 12.01 MHz across N=16→32→64→128)
  and is already at the 12 MHz floor at N=128, any larger N that *did* fit
  in area would very likely fail timing at the board's fixed 12 MHz
  oscillator without a clock divider or pipelining — so **N=128 is reported
  as the practical maximum for this exact unpipelined combinational-lattice
  design on this board's clock**, distinct from the ~218-cell area-only
  ceiling.
- 2-D: only 2 points measured (M=4 fits at 17%, M=8 at 50%, both with good
  Fmax margin). Linear extrapolation from those 2 points alone gives
  `cells_max ≈ (5280−359)/36.0 ≈ 137` → `M_max ≈ 11` (121 cells) — **low
  confidence, 2-point extrapolation, not independently confirmed.**

## 6. ECP5-85F extrapolation (rough, per the roadmap's own framing)

Using the 1-D slope (22.3 LC/cell) against ECP5-85F's ~84,000 LUT4s:

```
N_max(ECP5-85F) ≈ (84,000 − 405) / 22.3 ≈ 3,755 cells
```

**Caveat, stated plainly**: this is a unit-for-unit LUT4-count extrapolation
only. iCE40's ICESTORM_LC is 1 LUT4 + 1 DFF; ECP5's slice is 2 LUT4 + 2 FF
with different routing fabric, carry chains, and BRAM/DSP balance — Fmax
would not scale the same way, and no ECP5 toolchain build was run (none is
available in this environment). Treat ~3,700 cells as an order-of-magnitude
number, not a verified figure.

## 7. Sim-verified configurations (against extended `verify_lattice_scale.py`)

Both required configurations (mid-size + largest-that-fits) were compiled
with `iverilog -g2012` and run with `vvp`, UART bytes captured, and checked
byte-for-byte against the Python reference:

- **N=32** (mid-size): `PASS: sim phase lattice scale N=32 all_pass=1
  chars=584` → `python verify_lattice_scale.py 32
  evidence/phase_lattice_scale/sim_uart_N32.txt 8 4` → `MATCH: capture
  contains exact reference block for N=32 (8 generations + 4 clock
  generations)`.
- **N=128** (largest that fits UP5K): `PASS: sim phase lattice scale N=128
  all_pass=1 chars=1736` → `python verify_lattice_scale.py 128
  evidence/phase_lattice_scale/sim_uart_N128.txt 8 4` → `MATCH: capture
  contains exact reference block for N=128 (8 generations + 4 clock
  generations)`.

Both include the on-chip Phi1-homogeneous-field 1/2 clock self-check
(`all_pass=1`), which is generated and checked entirely on-chip/in-sim (not
just the datapath dump), so this is a genuine functional pass, not just a
byte-format match.

Reproduction:
```
iverilog -g2012 -o tb_scale.vvp -Ptb_phase_lattice_scale.N=<N> \
  uart_tx.v phase_lattice_uart.v phase_lattice_uart_scale.v tb_phase_lattice_scale.v
vvp tb_scale.vvp +OUT=evidence/phase_lattice_scale/sim_uart_N<N>.txt
python verify_lattice_scale.py <N> evidence/phase_lattice_scale/sim_uart_N<N>.txt 8 4
```

2-D configurations were **not** sim-verified against a Python reference
(not required by the brief, which scoped sim-verification to the 1-D case);
the 2-D RTL was confirmed to elaborate cleanly under `iverilog -g2012` and
its resource/Fmax numbers above come from real synthesis+PnR, but its
generation-by-generation output was not cross-checked bit-for-bit against a
host model. Flagged here as a gap, not glossed over.

## 8. 2-D feasibility verdict

**YES, with a real per-cell cost penalty.** Both tested 2-D torus
configurations (4×4 = 16 cells, 8×8 = 64 cells) synthesize, place, and route
successfully on UP5K with comfortable margin (17% / 50% LC utilization,
21.11 / 13.82 MHz Fmax vs. the 12 MHz board clock). The 2-D rule from
`AVENUE_02_lattice_2d.md` (row-then-column vs. the fixed von-Neumann
grouping, compared per-cell as the defect signal) costs **~1.6× the LC per
cell** of the 1-D ring (36.0 vs. 22.3 LC/cell), from the extra neighbor
fan-in (4 neighbors + center vs. 2 neighbors + center) driving 9
`compose_gate` instances per cell instead of 4. Extrapolating (low
confidence, 2 points only) puts the UP5K area ceiling around an 11×11 torus
(~121 cells); a 16×16 torus (256 cells) would need roughly 9,600+ LC by this
model and almost certainly will not fit UP5K's 5280 LCs — consistent with
the 1-D N=256 result failing outright. **Verdict: 2-D lattices are
practical on UP5K only at small-to-modest grid sizes (verified up to 8×8);
larger 2-D torus grids should target a bigger device (ECP5-class) rather
than UP5K.**
