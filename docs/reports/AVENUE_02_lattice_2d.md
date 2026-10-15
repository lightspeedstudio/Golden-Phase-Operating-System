# Avenue 2 — 2-D phase lattice torus dynamics

Script: `scripts/research/avenue02_lattice_2d.py` (new file, does not modify
any existing script). Reproduction: `python scripts/research/avenue02_lattice_2d.py`.

Compose table used: reproduced via `make_table(ALPHA_STAR)` (the exact
construction in `scripts/research/gpl_program.py`) and asserted equal to the
literal table in the roadmap standing rule:
`((0,1,2,3),(1,3,3,2),(2,3,3,1),(3,2,1,0))`. Assertion passes.

## Exact rule definitions

Let `o` denote compose (table lookup), `rho(x) = 3-x`.

**Rule (a) — row rule then column rule** (this is what evolves the grid):
```
rowpass(G)[i,j] = (G[i,j-1] o G[i,j]) o G[i,j+1]      # row, left-nested, periodic
colpass(G)[i,j] = (G[i-1,j] o G[i,j]) o G[i+1,j]      # column, left-nested, periodic
rule_a(G) = colpass(rowpass(G))                        # ROW PASS FIRST
```

**Rule (b) — fixed von Neumann grouping** (this evolves the grid, synchronous update):
```
x' = ((N o x) o S) o ((E o x) o W)     at every cell simultaneously
```

**Defect fields** (diagnostic only — never used to evolve the grid, exactly
mirroring `gpl_program.py`'s pattern where `cfg` evolves via `stepL` only and
`stepR(cfg)` is computed solely for the density measurement):

- `rule_a_mirror(G) = rowpass(colpass(G))` (column pass first). `defect_a` =
  cells where `rule_a(G) != rule_a_mirror(G)`.
  **Honesty note:** this is *not* a pure same-word/different-parenthesization
  pair in the Theorem-1 sense — `rule_a` reads its effective 3x3 Moore
  neighborhood in row-major symbol order, `rule_a_mirror` reads it
  column-major. Theorem 1 (which covers reparenthesizations of the *same*
  word) does not by itself guarantee disagreements are pure reflections here.
  This was checked exhaustively rather than assumed (see below).
- `rule_b_mirror(G)` = the fully right-nested parenthesization of the exact
  same word `(N, x, S, E, x, W)` in the exact same left-to-right order:
  `N o (x o (S o (E o (x o W))))`. `defect_b` = cells where
  `rule_b(G) != rule_b_mirror(G)`. This *is* a genuine same-word/
  different-parenthesization pair, so Theorem 1 directly predicts
  disagreements are always exactly `rho`.

## Exhaustive coherence checks (computed, not sampled)

| comparison | tuples tested | disagreements | always-reflection |
|---|---|---|---|
| rule (b) vs rule (b) mirror — same word, diff. parenthesization | 1,024 (4⁵, x-slot reused) | 336 (32.81%) | **True** |
| rule (a) vs rule (a) mirror — 3×3 block, row-major vs column-major | 262,144 (4⁹) | 110,592 (42.19%) | **True** |

Both are exact, full enumerations. Rule (b)'s "always reflection" result is
guaranteed by Theorem 1 (verified, not just assumed). Rule (a)'s result is
**not** guaranteed by Theorem 1 as stated (different symbol order, not just
different parenthesization) but empirically holds over the full 4⁹-point
enumeration — an honest, additional finding, not a re-derivation of the
theorem.

## Defect density vs time

10 trials/cell, random initial fields, 50 steps. `avg` = mean fraction of
cells where the rule disagrees with its mirror.

**M = 16**

| t | 0 | 1 | 2 | 5 | 10 | 20 | 30 | 49 |
|---|---|---|---|---|---|---|---|---|
| rule A | 0.4297 | 0.4035 | 0.4043 | 0.4035 | 0.4020 | 0.3973 | 0.4004 | 0.4148 |
| rule B | 0.3258 | 0.3305 | 0.3297 | 0.3359 | **0.0000** | 0.0000 | 0.0000 | 0.0000 |

**M = 32**

| t | 0 | 1 | 2 | 5 | 10 | 20 | 30 | 49 |
|---|---|---|---|---|---|---|---|---|
| rule A | 0.4271 | 0.4078 | 0.4096 | 0.4057 | 0.4042 | 0.4041 | 0.4062 | 0.3989 |
| rule B | 0.3336 | 0.3459 | 0.3534 | 0.3461 | 0.3527 | **0.0000** | 0.0000 | 0.0000 |

**Finding:** rule (a) is *persistently defective* — density plateaus around
0.40–0.43 and never decays over 50 steps, for both M=16 and M=32 (same
qualitative behavior as the 1-D N=64 case, ≈0.22–0.26, in
`THEOREMS_2026-07-23.md`). Rule (b) instead **collapses to zero defect
density in all 10/10 sampled trials**, at both M values — by t=10 (M=16) or
between t=10 and t=20 (M=32) the grid has fully relaxed to a state with no
disagreement at all.

## State histograms (100 steps, 10 trials)

| rule | Φ0 | Φ1 | Φ2 | Φ3 |
|---|---|---|---|---|
| A, M=16 | 0.1504 | 0.2543 | 0.2445 | 0.3508 |
| A, M=32 | 0.1501 | 0.2460 | 0.2389 | 0.3650 |
| B, M=16 | **1.0000** | 0.0000 | 0.0000 | 0.0000 |
| B, M=32 | **1.0000** | 0.0000 | 0.0000 | 0.0000 |

**Finding:** rule (b) drives every one of the 10 sampled random initial
fields, at both grid sizes, to the fully uniform Φ0 state (the "safe"
absorbing fixed point) — consistent with the zero defect density above (a
uniform field trivially has zero defect). Rule (a) never uniformizes; its
long-run histogram is close to the 1-D long-run bias reported in the paper
(Φ0 .168 / Φ1 .253 / Φ2 .257 / Φ3 .323), skewed toward Φ3.

## Spatial autocorrelation / correlation length

FFT (Wiener–Khinchin) periodic autocorrelation of the defect field at t=30,
averaged over the trials where a defect field still existed, radially
binned by minimum-image distance on the torus.

| rule, M | C(0) | C(1) | C(2) | C(3) | C(4) | ξ (1/e crossing) |
|---|---|---|---|---|---|---|
| A, 16 | 1.0 | 0.0031 | -0.0095 | 0.0099 | -0.0136 | **0.635** |
| A, 32 | 1.0 | 0.0120 | -0.0091 | -0.0005 | -0.0007 | **0.640** |
| B, 16 | — | — | — | — | — | undefined (field is exactly zero by t=30 in all trials — no variance to correlate) |
| B, 32 | — | — | — | — | — | undefined (same reason) |

**Finding:** rule (a)'s defect field has essentially **no spatial structure**
— C(r) falls to noise level (< 0.02 in magnitude) already at r=1, giving a
correlation length just above the lattice spacing (~0.64 cells) in both grid
sizes. The persistent defect density is spatially uncorrelated noise, not
domains or walls. For rule (b), the correlation-length question is moot in
the tested regime: the field has already gone to zero everywhere by t=30, so
there is nothing to correlate (reported honestly as "undefined", not
approximated).

## Localized / propagating structures — single-cell perturbation on uniform backgrounds

M=16, one cell at center flipped to `rho(v)` on an otherwise uniform
background `v`, evolved up to 40 steps (80 confirmed at M=32, see below).

| rule | background v | perturb to | outcome |
|---|---|---|---|
| A | 0 | 3 | periodic breather, footprint cycles 1→9→9→25→9→81→25→121→(repeat), period 8, **center fixed** at (8,8) |
| A | **1** | **2** | **footprint = 1 at every one of 40/40 steps; moves at constant velocity (−1,−1) per tick (confirmed to 80 steps on M=32)** |
| A | **2** | **1** | same as above (mirror case, ρ-symmetric) |
| A | 3 | 0 | breather, identical statistics to v=0 case (ρ-symmetric) |
| B | 0 | 3 | decays smoothly to 0 by step 8 (footprint 1→4→4→16→4→16→16→64→0); final grid uniform Φ0 |
| B | 1 | 2 | single perturbed cell triggers full-grid collapse to uniform Φ0 within 1 step (footprint jumps to 256 at t=1 and the grid is already uniformly Φ0) |
| B | 2 | 1 | same as above (mirror case) |
| B | 3 | 0 | gradual full-grid collapse to uniform Φ0 over ~8–9 steps (footprint 1→252→...→256) |

### Key positive result: a genuine glider under rule (a)

Perturbing a single cell on the uniform Φ1 (or Φ2) background under rule (a)
produces a **stable, single-cell, constant-velocity moving defect** — the
footprint stays exactly 1 for the entire run (40 steps at M=16, reconfirmed
80 steps at M=32) and its position moves diagonally by exactly (−1,−1) cells
per tick, wrapping around the torus (verified positions at M=32, every 10
steps: (16,16)→(6,6)→(28,28)[wrapped]→(18,18)→(8,8)→(30,30)[wrapped]→(20,20)→(10,10),
consistent with displacement −10 mod 32 each interval). This is a genuine
1-cell glider on this background under this rule — not a torus/finite-size
artifact, since it persists unchanged in character at both M=16 (8-step
torus period) and M=32 (32-step torus period).

ASCII evolution (M=16, background Φ1=':' , defect shown as '+'), t=0,1,2,3,5:
```
t=0                  t=1                  t=2
::::::::::::::::     ::::::::::::::::     ::::::::::::::::
::::::::::::::::     ::::::::::::::::     ::::::::::::::::
::::::::::::::::     ::::::::::::::::     ::::::::::::::::
::::::::::::::::     ::::::::::::::::     ::::::::::::::::
::::::::::::::::     ::::::::::::::::     ::::::::::::::::
::::::::::::::::     ::::::::::::::::     ::::::::::::::::
::::::::::::::::     ::::::::::::::::     ::::::+:::::::::
::::::::::::::::     :::::::+::::::::     ::::::::::::::::
::::::::+:::::::     ::::::::::::::::     ::::::::::::::::
::::::::::::::::     ::::::::::::::::     ::::::::::::::::
   (rest ':')            (rest ':')            (rest ':')

t=3                  t=5
::::::::::::::::     ::::::::::::::::
::::::::::::::::     ::::::::::::::::
::::::::::::::::     ::::::::::::::::
::::::::::::::::     :::+::::::::::::
::::::::::::::::     ::::::::::::::::
:::::+::::::::::     ::::::::::::::::
::::::::::::::::         (rest ':')
   (rest ':')
```
The single `+` marches diagonally up-left one cell per tick: (8,8)→(7,7)→(6,6)→(5,5)→…→(3,3) at t=5.

### Breather under rule (a) on the Φ0/Φ3 background

Not a small localized glider — footprint grows to 121/256 (~47% of the M=16
grid) before periodically collapsing back to a single cell, center pinned at
the seed site (8,8). Reported honestly as an **open question**: at M=16 the
peak footprint is a large fraction of the grid, so this may partly be a
finite-size (torus self-interaction) effect rather than a bounded-size
breather; it was not re-tested at M=32 for peak-size scaling and that is
flagged as unfinished, not claimed.

ASCII evolution (Φ0='.' background, `#`=Φ3 defect), t=0,1,2,3:
```
t=0                       t=1                       t=2
........#.......          .......###......          ......#.#.#.....
   (rest '.')              .......###......          ......#.#.#.....
                            .......###......          ......#.#.#.....
                              (rest '.')                 (rest '.')
```
(full grids in script output; growth continues to a 11x11-ish diamond by
t=5 before collapsing back toward a point later in the cycle.)

### Rule (b): healing, not gliding

On its own attractor background (v=0) a perturbation heals smoothly and
symmetrically back to uniform Φ0 by step 8 (expanding-then-contracting
diamond, see script output t=0..5). On the other three backgrounds the same
single-cell perturbation is enough to destabilize the *entire* grid, which
then converges to the global Φ0 attractor (footprint saturating at M² is a
"converged to the other fixed point" artifact, confirmed via the
`final_uniform=True, final_value=0` diagnostic — not evidence of chaos or of
a spreading pattern with real spatial content).

## Reproduction

```
cd C:\Users\richa\OneDrive\Desktop\FullOS\kairos
python scripts/research/avenue02_lattice_2d.py
```
Runtime: well under a minute on this machine (exhaustive checks vectorized
with numpy; grid experiments use numpy array ops throughout).

## Summary / honest verdict

- Two update rules were compared as specified; both rule definitions and
  their diagnostic "mirror" groupings are stated exactly above.
- The reflection-only-disagreement claim (Theorem 1) was verified
  exhaustively for rule (b) (a genuine same-word case) and, as an additional
  empirical finding beyond what Theorem 1 guarantees, also holds exhaustively
  for rule (a)'s row-major/column-major comparison.
- Rule (a) shows persistent, spatially uncorrelated defect density (~0.40–0.43,
  correlation length ≈ 0.6 cells) — no domains or walls found.
- Rule (b) relaxes completely to the uniform Φ0 fixed point in all sampled
  trials at both M=16 and M=32.
- **A genuine propagating localized structure (1-cell glider, constant
  diagonal velocity, indefinitely stable) was found under rule (a) on the
  uniform Φ1/Φ2 background**, confirmed at two grid sizes and run lengths
  (40 steps @ M=16, 80 steps @ M=32). This is the headline positive result.
- A larger, non-propagating periodic "breather" was also found under rule
  (a) on the Φ0/Φ3 background, but its size relative to M=16 leaves open
  whether it stays bounded at larger M — flagged as open, not claimed.
- No localized/propagating structures were found under rule (b); single-cell
  perturbations either heal (on the Φ0 attractor) or trigger full collapse
  to the Φ0 attractor (on the other three backgrounds).
