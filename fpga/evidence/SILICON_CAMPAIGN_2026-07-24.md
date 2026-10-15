# Silicon Test Campaign — 2026-07-24

**Board:** iCEBreaker V1.1a (Lattice iCE40 UP5K)  
**UART:** COM24 · 115200 8N1  
**Verdict:** **10/10 campaign PASS** + **#11 glider golden M=4 PASS** on silicon

Skipped (per plan): N=128, OS curvature watchdog. M=8 glider: timing FAIL (~2.9 MHz), not sealed.

---

## Results

| # | Test | Batch | Result | Evidence | Bitstream SHA256 |
|---|------|-------|--------|----------|------------------|
| 1 | Complex gate suite regression | A | **PASS** — COMPOSE 16/16; REFLECT 4/4; TOPOLOGY 64/64; T_SIGN 4/4; T_ENERGY 4/4 | `complex_gates/` | `f499032615d2284b90fccfcf98207fae4f2dd2617868eba68549cc3102ae6dfc` |
| 6 | GPL VM complex regs | A | **PASS** — GPL VM PASS (`REGS A=1 B=0 C=2 D=1`) | `complex_gates/` | same as #1 |
| 7 | Parallel 4× ALU | A | **PASS** — PARALLEL 4/4 | `complex_gates/` | same as #1 |
| 2 | Lattice N=16 + curvature | B | **PASS** — UART MATCH `verify_lattice.py`; 40 gens defect readout | `phase_lattice/` | `9f6206a50754d0cc9f5b81e17ca664070c877c504bd4c478c85e4052fe89c0cd` |
| 5 | Φ1↔Φ2 clock | B | **PASS** — CLOCK PASS (Phi1 alternation) | `phase_lattice/` | same as #2 |
| 3 | Homogeneous Φ0 vacuum | B2 | **PASS** — VACUUM PASS; fixed Phi0, D=00 all gens | `phase_lattice_phi0/` | `f38168986ae32760b1f321ce5d907ace3e2c340aad8b9288df2b7e9bf52b8353` |
| 4 | Homogeneous Φ3 lock | B2 | **PASS** — VACUUM PASS; fixed Phi3, D=00 all gens | `phase_lattice_phi3/` | `f0341b24c661f5c0e0d6a8451ef216c0f020891e9fe4795f243551e26dd72911` |
| 8 | Lattice N=32 | C | **PASS** — MATCH `verify_lattice_scale.py`; CLOCK PASS | `phase_lattice_scale/N32_physical/` | `527a400843e9cbb218a1b0f1580e2be3dc9ef9189efbf1292a31816152361488` |
| 9 | Lattice N=64 | C | **PASS** — MATCH `verify_lattice_scale.py`; CLOCK PASS | `phase_lattice_scale/N64_physical/` | `06777ca8cf0c8013e9e20ceb6ce162bddfed72005bbc18ad4b722b2b660218bf` |
| 10 | 2-D torus M=4 smoke | D | **PASS** — 04×04 fabric alive (4 gen lines); not a glider claim | `phase_lattice_2d_M4/` | `68dd43ed77d72bec6ac191f3dbeae2dc9da2a0ad13d8320537c146c27ac020f9` |
| 11 | Avenue 2 glider golden M=4 | follow-up | **PASS** — rule_a 1-cell glider MATCH host; F=01 × 8 gens; GLIDER PASS | `phase_lattice_2d_glider_M4/` | `8c97780d67102159df9c5e367ccfc0bdff863bdd822fe72bd709d40797ece81f` |

M=8 glider: synth OK, PnR timing FAIL (Fmax ≈ 2.9 MHz @ 12 MHz) — not sealed.

---

## Batch notes

### A — Algebra regression
Re-flashed complex suite via `flash-capture-complex-gates.ps1`. Single bitstream covers tests 1, 6, and 7.

### B — Lattice N=16
`flash-capture-phase-lattice.ps1` → Python MATCH + CLOCK PASS. Covers tests 2 and 5.

### B2 — Vacua
Parameterized vacuum tops (`phase_lattice_vac.v`, PHI=0 / PHI=3). Both sealed with homogeneous field and zero defect.

### C — Scale
`flash-capture-phase-lattice-scale.ps1 -N 32|64`. Both sizes match host reference and pass clock check at 12 MHz.

### D — 2-D smoke
`flash-capture-phase-lattice-2d.ps1 -M 4` (PowerShell `$M`/`$m` case clash fixed to `$rx`/`$gridLabel`). UART shows `PHASE LATTICE 2D 04x04` … `LATTICE DONE`.

---

## Seal files

Each evidence directory contains `PHYSICALLY_VERIFIED.txt`, `uart_capture.txt` (or equivalent), and `icebreaker.bin.sha256` where applicable. Paths are relative to `kairos/fpga/evidence/`.
