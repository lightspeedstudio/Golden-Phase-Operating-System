# PHASE-DISK Mathematical Model (PHASEGUARD Gate 1)

**Docket:** CAREGAGA-GPOS-2026-001
**Gate:** 1 of 8 — host math reference model
**Status:** HYPOTHESIS / SIMULATED (host `f64` reference model). This is
falsification research. Nothing here is a proven or implemented kernel-integrity
mechanism. `core::gphase_core` remains the sole algebra authority; this model is
a **separate parameterized solver** that never feeds the canonical `params()`.

Source: [`src/stateclock/phase_disk.rs`](../../src/stateclock/phase_disk.rs)
(feature `phaseguard-research`). Reproduce every number below with:

```bash
cargo test --features phaseguard-research phase_disk                     # invariants (5 tests)
cargo test --features phaseguard-research phase_disk::tests::dump_reference -- --ignored --nocapture   # this trace
```

All residuals below were emitted by that command on 2026-08-01.

---

## 1. The signed-branch biquadratic

The canonical kernel is `y⁴ − φ·y² + α = 0`. The phase-disk model generalizes the
sign of the constant term to a **branch parameter** σ ∈ {+1, −1}:

```
y⁴ − φ·y² + σ·α = 0
```

with the constants fixed by `gphase_core` (never redefined here):

| constant | value |
|---|---|
| φ | 1.618033988749895 |
| α | 0.007297352569300 |
| √α | 0.085424543131936 |
| α^(1/4) | 0.292274773341690 |

Solving for `y²` gives `y² = (φ ± √D)/2` with **discriminant** `D = φ² − 4σα`.

### σ = +1 — canonical branch (cross-check only)

`D = φ² − 4α = 2.588844578472695` (> 0), both `y²` roots positive → four real roots:

| quantity | value |
|---|---|
| y²_outer | 1.613511335323455 |
| y²_inner | 0.004522653426440 |
| y_outer | ±1.270240660396074 |
| y_inner | ±0.067250676029611 |

Invariants (must match the canonical algebra):

- **sum** `y_outer² + y_inner² = φ` — residual **0.000e0**
- **product** `y_outer² · y_inner² = α` (Vieta) — residual **1.648e-17**

This reproduces `gphase_core`'s outer/inner rail values, confirming the parameterized
solver is faithful at σ = +1.

### σ = −1 — research branch (imaginary inner pair)

`D = φ² + 4α = 2.647223399027095`. Now `√D > φ`, so `y²_inner` goes **negative**:

| quantity | value |
|---|---|
| y²_outer | 1.622531499320479 |
| y²_inner | −0.004497510570584 |
| y_outer | ±1.273786284790537 (real) |
| y_inner | ±0.067063481646753 · **i** |

The outer pair stays real; the inner pair becomes **imaginary** `±i·b` with
`b = 0.067063481646753`. Vieta product of squared roots equals `σα = −α` —
residual **8.760e-17**. This is the same real-rail / imaginary-inner structure
seen in prior `ib` exploration, here produced by a single sign flip.

> **Finding (Gate 1):** the negative-α branch is the minimal parameter change that
> moves the inner state pair off the real line onto the imaginary axis while
> leaving the outer (real-rail) pair essentially unchanged (Δy_outer ≈ 3.5e-3).

---

## 2. The squeeze / release family

Define a one-parameter, **product-conserving** family on the radial coordinate `s`:

```
A(s) = α^(1/4) · e^{+s}      B(s) = α^(1/4) · e^{−s}      A(s)·B(s) = √α   ∀ s
```

`s` is the pure squeeze coordinate (the log half-ratio `s = ½·ln(A/B)`). `s = 0`
is the isotropic point `A = B = α^(1/4) = 0.29227…`. The conserved product
`A·B = √α` held to **< 1e-12** across all 401 steps of a full squeeze cycle
(`s: 0 → +2 → −2 → 0`).

### Direction bit

The **squeezing vs. releasing** bit is `sign(ds/dt)` with a dead-band:
`ds > ε → Squeezing`, `ds < −ε → Releasing`, else `Hold`.

---

## 3. The link between the squeeze family and the canonical solver

The canonical roots satisfy **two** constraints — product `y_o·y_i = √α` **and**
sum `y_o² + y_i² = φ`. The squeeze family enforces only the product. Therefore the
canonical operating point is the **single** squeeze member that also satisfies the
sum constraint, at

```
s* = ¼·ln(y²_outer / y²_inner) = 1.469267293479488
```

Verified: `A(s*) = y_outer` and `B(s*) = y_inner` to < 1e-9
(test `canonical_squeeze_coordinate_reproduces_root_ratio`).

> **Finding (Gate 1):** the squeeze coordinate is exactly the extra degree of
> freedom the biquadratic's sum constraint removes. The "phase disk" radial axis
> is not a new invention — it is the product-invariant manifold on which the
> canonical algebra sits at a single point `s*`. This is a *reproduction*, not yet
> an *explanation* of anything the canonical algebra does not already encode.

---

## 4. Reference squeeze-cycle trace (s_max = 1.0, 8 steps)

Emitted by `dump_reference`; `AB−√α` is the conserved-product residual.

| step | s | A(s) | B(s) | AB−√α | direction |
|---|---|---|---|---|---|
| 0 | +0.000000 | 0.292274773 | 0.292274773 | 0.00e0 | Hold |

*(Full 9-row trace regenerates from the command above; row 0 shown as the isotropic
anchor. Later rows sweep +1 then back through 0 to −1, staying product-conserved.)*

---

## 5. Gate-1 verdict and what it does NOT yet establish

**Established (SIMULATED, host):**
1. A faithful parameterized signed-branch solver (σ=+1 reproduces canonical to 1e-17).
2. The negative branch produces the imaginary inner pair from one sign flip.
3. A product-conserving squeeze family with an exact link (`s*`) to the canonical roots.
4. A direction (squeeze/release) classifier.

**NOT established (deferred to later gates):**
- That any of this **detects** a fault the existing algebra/statechain does not
  (Gate 3 fault injection).
- That the oscillator **explains** the 12 reflection triples rather than merely
  reproducing the 52/12/0 census (Gate 2).
- Any hardware or physical claim (Gates 6, 8).

No result is claimed beyond the labeled host simulation. Next: **Gate 2** — run the
extended model through the 16/4/64 census and compare bit-exact to `gphase_core`,
asking whether the oscillator/branch structure *explains* the 12 reflection cases.
