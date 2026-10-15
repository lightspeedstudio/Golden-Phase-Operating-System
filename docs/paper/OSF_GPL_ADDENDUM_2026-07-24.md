# The Golden Phase Algebra — Results Addendum (Avenues 1–9)
## Companion to `OSF_GPL_MATHEMATICS.md` (2026-07-23)

**Richard Careaga** — Metatron Research · Thousand Oaks, California  
**Date:** 2026-07-24 · **Status:** computed addendum (reproducible scripts + Rust harnesses)  
**Full narrative tables:** `docs/reports/FULL_RESULTS_2026-07-24.md`

---

## Abstract (addendum)

Nine follow-on investigations were executed against the exact GPL(4) table of
`gphase_core`. We (i) classify 1-D ring attractors through N=14 and resolve the
N=8 period-12 orbits into six symmetry classes; (ii) discover a stable
single-cell **glider** on the 2-D torus under a row-then-column update rule;
(iii) show sibling algebras above the α-bifurcation remain involution-controlled
but lose reflection-orbit coherence; (iv) strengthen the GPL(6)/GPL(8) negative
to 2×10⁶ unconstrained samples with zero associative orbit-quotients;
(v) place the associator outside Bol–Moufang and outside a fiber-constant
Z₂ 3-cocycle, while remaining a structural cousin of Albuquerque–Majid
quasialgebras; (vi–viii) harden the OS/protocol stack with a statechain fuzz
campaign, a hybrid CRC+GPL frame, and a causal-curvature watchdog with measured
false-positive rate 0 under four clean workloads; (ix) map UP5K resource
scaling for phase lattices to a practical maximum of N=128 (1-D) and verified
8×8 (2-D).

---

## 1. Dynamics

**1-D.** Attractor fraction falls with N (≈2.5% at N=8). Period-12 cycles at
N=8 number eight orbits in six classes under ring symmetries × ρ. Transients
remain short.

**2-D.** Rule (a) (row then column) sustains defect density ≈0.40–0.43 with
essentially zero spatial correlation — and hosts a **glider**: one reflected
cell on a uniform Φ1 (or Φ2) background propagates at velocity (−1,−1)
indefinitely (verified at M=16 and M=32). Rule (b) (fixed von Neumann
parenthesization) drives random fields to uniform Φ0 and extinguishes defects.

## 2. Uniqueness and non-existence

Siblings #2/#3: ten “other” associators each, all controlled by a fixed
involution σ≠ρ, with associative 2-block quotients — yet reflection-orbit
coherence failure climbs from ~16% (length 3) to ~81% (length 7). The physical
α-window remains the unique *ρ-coherent* member of the family.

GPL(6) and GPL(8) in the ±-root affine-snap family: 1,000,000 draws each;
orbit-coherence occurs (1,624 / 67) but **never** with an associative group
quotient. Near-misses are a single degenerate absorbing table per family.

## 3. Taxonomy

GPL fails alternative, Bol, Moufang, and power-associativity identities
(exhaustive). The associator ω is well-defined on {same, ρ} but is **not**
constant on orbit fibers, so it does not induce a canonical group 3-cocycle on
the Z₂ quotient. Fair statement: *near* twisted/quasialgebra organization,
not a named Bol–Moufang loop, not a literal Albuquerque–Majid instance.

## 4. Systems consequences

- Statechain internal verify: 1.0000 on bit-flips, parity, reorder, splice;
  **0.0000** on pure tail deletion (checkpoint layer’s job).
- Hybrid frame: CRC for detection; GPL topology for diagnosis — excellent on
  reflection and shuffle; weak on transposition/burst given 1-bit triple kinds.
- Curvature watchdog: workload-specific nulls (idle 0; spawn ≈3; VM ≈5;
  fs ≈2.7); |z|≥3.29 clean FPR = 0; injected 39-defect burst caught.
- FPGA: LC ≈ 22.3N + 405; N=128 fits timing at 12.01 MHz; N=256 does not place;
  2-D 8×8 fits at ~1.6× LC/cell vs 1-D.

## 5. Revisions to open problems (paper §9)

| Prior open | Status after avenues |
|------------|----------------------|
| Period-12 classification | **Partially closed** (6 classes listed) |
| 2-D lattices | **Opened positively** (glider + rule dichotomy) |
| GPL(2n) | **Negative strengthened** (still open as impossibility) |
| Cohomological formulation | **Negative for naive Z₂ cocycle**; naming open |
| Hardware curvature | **OS watchdog live**; lattice-on-FPGA curvature readout still open |

## Reproducibility

See `docs/reports/FULL_RESULTS_2026-07-24.md` §“Reproduce everything”.
Ground truth table remains `((0,1,2,3),(1,3,3,2),(2,3,3,1),(3,2,1,0))`.
