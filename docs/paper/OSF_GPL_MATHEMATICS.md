# The Golden Phase Algebra: Coherence, Structure, and the Unique Window
## A four-state non-associative algebra with a controlled associator, its lattice dynamics, and an honest comparison with CRC

**Richard Careaga** — Metatron Research Project, Thousand Oaks, California
(with Claude, Anthropic — co-contributor, computation and drafting)

**Date:** 2026-07-23 · **Status:** preprint draft for OSF
**Reproducibility:** all numerical claims regenerate from
`kairos/scripts/research/gpl_next_level.py` and `gpl_program.py`
(Python 3, no dependencies), computed against the exact production table in
`kairos/src/core/gphase_core.rs`.

---

## Abstract

We study a commutative unital magma on four elements derived, with no
hand-tuned entries, from the biquadratic kernel **y⁴ − φy² + α = 0** (φ the
golden ratio, α the fine-structure constant) via a fold-and-snap map onto the
bounded rail [0, π/φ]. We prove three results. **(1) Coherence:** every
parenthesization of every word evaluates into a single reflection orbit
{v, 3−v} — the value of any computation is well-defined up to one parity
bit. **(2) Structure:** the algebra is an extension of the group Z₂ by the
reflection involution; compose descends to the reflection orbits, where it is
exactly XOR, and the quotient map coincides with the algebra's energy
threshold. All non-associativity is confined to the fiber, the same pattern
by which twisted group algebras (e.g. the octonions) organize their
associator. **(3) Uniqueness of the window:** sweeping α across its full
admissible range (0, φ²/4) yields exactly three algebras; only the one
containing the physical fine-structure value is coherent. We then report the
first dynamics of phase lattices (cellular automata over the algebra),
an honest negative result for a six-state analog, and a Monte Carlo
comparison against CRC-8/16 that confirms CRC's superiority for raw error
*detection* while identifying the algebra's distinct contribution: semantic
*diagnosis* of path-order corruption and a per-computation causal parity bit
suitable for audit logs.

---

## 1. The algebra

Let φ = (1+√5)/2, rail R = π/φ, and α = 7.2973525693×10⁻³. The four roots of
y⁴ − φy² + α = 0 are ±√z± with z± = (φ ± √(φ²−4α))/2. Mapped affinely onto
[0, R], they give four positions s₀ < s₁ < s₂ < s₃ with snap boundaries at
adjacent midpoints. Define

> a ∘ b = snap(fold(s_a + s_b)),  fold(x) = x if x ≤ R else 2R − x.

Nothing below is hardcoded; the operation table *emerges*:

| ∘ | 0 | 1 | 2 | 3 |
|---|---|---|---|---|
| **0** | 0 | 1 | 2 | 3 |
| **1** | 1 | 3 | 3 | 2 |
| **2** | 2 | 3 | 3 | 1 |
| **3** | 3 | 2 | 1 | 0 |

Basic facts (all machine-checked): commutative; Φ0 is the identity; the row
of Φ3 implements the reflection ρ(x) = 3 − x, an involution; the algebra is
**not** associative. The prior Associator–Reflection Theorem states that for
any triple, left and right groupings agree or differ by exactly ρ
(52 "same" and 12 "reflection" triples of 64; no third kind).

## 2. Theorem 1 — Coherence (all words, all groupings)

**Theorem.** For every word w over {Φ0..Φ3} of any length, the set of values
obtainable by all parenthesizations of w lies inside a single reflection
orbit {v, 3−v}.

**Proof.** Two finite facts suffice.
*(i) Compose respects orbits:* for all a, b, the set
{a∘b, a∘ρb, ρa∘b, ρa∘ρb} lies in one orbit. This is a 16-case check; it
passes (script §1).
*(ii) The quotient is associative:* by Theorem 2 the orbit-level operation is
Z₂, a group. Hence the orbit of any evaluation is independent of grouping
(it equals the XOR of the letters' orbits), and by (i) and induction on word
length every evaluation's value lies in that single orbit. ∎

**Exhaustive verification.** All words of lengths 3–7 (21,824 words; all
Catalan-many parenthesizations each): zero violations. At length 7, 1,644
words are grouping-invariant outright and 14,740 occupy a full ρ-orbit pair.

**Interpretation.** Re-association can never corrupt a Golden Phase
computation by more than one *recorded* bit. This is the mathematical basis
for the `assoc_parity` audit field (§7).

## 3. Theorem 2 — Structure (Z₂ twisted by reflection)

The reflection orbits are O = {Φ0, Φ3} and I = {Φ1, Φ2}. By Theorem 1(i),
∘ descends to orbits. The descended table is

| ∘̄ | O | I |
|---|---|---|
| **O** | O | I |
| **I** | I | O |

— exactly **Z₂ under XOR** (machine-checked over all 16 lifted pairs). The
quotient map O↦1, I↦0 is precisely the algebra's existing
`threshold_energy`. Thus:

> **GPL is an extension of the group Z₂ by the reflection involution ρ; the
> associator takes values only in the fiber.**

This places the algebra in the structural family of *twisted group
algebras*, where non-associativity is controlled by a torsion twist — the
same organizing pattern as the octonions (a Z₂³-graded twisted algebra). GPL
occupies the smallest nontrivial rung: base Z₂, fiber Z₂.

## 4. Theorem 3 — The α atlas and the unique coherent window

The construction is a one-parameter family: every α ∈ (0, φ²/4 ≈ 0.6545)
yields *some* 4×4 table. Scanning 400,000 points and bisecting each boundary
to machine precision (script §1) gives the complete atlas:

| Window (α) | Table (rows 1–2 shown) | Topology (same/refl/other) | Coherent | Quotient |
|---|---|---|---|---|
| (0, **0.096820784**) | 1∘1=3, 1∘2=3 (the GPL table) | 52 / 12 / **0** | **yes** | **Z₂** |
| (0.096820784, **0.418885438**) | 1∘1=2, 2∘2=2 | 54 / 0 / 10 | no | — |
| (0.418885438, 0.6545) | 1∘1=1, 2∘2=1 | 54 / 0 / 10 | no | — |

All three algebras share commutativity, the identity, and the reflection
row; but siblings #2 and #3 admit associator defects that are *not* the
reflection (10 "other" triples each), destroying both coherence and the
group quotient. At α = 0 the construction degenerates (Φ1, Φ2 merge).

**Honest reading.** The fine-structure constant is not numerically special:
any α in (0, 0.09682) gives the identical algebra, and the physical value
sits inside with 13.3× headroom. What *is* special is the window itself —
it selects the **unique coherent member of the family**. The bifurcation
point α*₁ = 0.096820784… is a new derived constant of the system. The
sibling algebras remain of independent interest precisely because they
demonstrate that coherence is not automatic: it is a property the GPL window
*has* and its neighbors *lack*.

## 5. Phase lattices (the mathematics under the "phase GPU")

Take a ring of N cells with states in {Φ0..Φ3} and the local update
x′ᵢ = (xᵢ₋₁ ∘ xᵢ) ∘ xᵢ₊₁ (rule L; rule R uses the right grouping). Each cell
is one compose gate — this is the object an FPGA computes thousands of in
parallel. First results (script §2):

- **Vacua and a clock.** Homogeneous Φ0 and Φ3 are fixed points (safe and
  locked are stable); homogeneous Φ1 ↔ Φ2 oscillates with period 2 — the
  algebra contains an intrinsic clock.
- **Fast relaxation.** Exhaustive analysis at N = 8 (65,536 configurations):
  only 1,632 states (2.5%) lie on attractors; cycle lengths are
  {1: 38, 2: 39, 4: 135, 8: 110, 12: 8}; transients are short
  (max 9, mean 4.24). Short transients and small attractors are what a
  hardware supervisor wants: perturbations die out in a handful of ticks.
- **Persistent causal curvature.** At N = 64 from random initial fields, the
  density of sites where rules L and R disagree (by Theorem 1, always by ρ)
  stabilizes at ≈ 0.22–0.26 and does not decay. The associator defect is a
  *bulk, persistent* observable of the lattice — measurable, and therefore
  usable as a baseline (§7).
- **Long-run bias.** The stationary state histogram is ≈ (Φ0 0.168,
  Φ1 0.253, Φ2 0.257, Φ3 0.323): the lattice prefers the locked state,
  consistent with Φ3's absorbing role in the table's interior.

Open problems: exact characterization of the period-12 orbits; defect-field
correlation length; 2-D lattices; and the FPGA realization (one LUT layer
per tick) with the defect density read out as a live curvature signal.

## 6. GPL(6): an honest negative result

Does the pattern generalize to six states — Z₃ twisted by reflection — from
a degree-6 kernel? We probed the natural family
(y²−z₁)(y²−z₂)(y²−z₃) with the golden trace constraint z₁+z₂+z₃ = φ,
scanning 391 admissible grid points (script §3). Result: **one** point
satisfies identity + reflection-row + orbit-coherence, and **none** has a Z₃
quotient. The reason is visible in the proof of Theorem 1: coherence needs
the quotient to be an associative *group*, and with three orbits the
descended operation is generically a non-associative 3-element magma. The
4-state algebra works because its quotient, Z₂, cannot fail to associate.

We state this as evidence, not a theorem: within this family and grid, no
GPL(6) analog exists, and the 4-state algebra appears genuinely special.
A complete impossibility proof (or a coherent 6-state construction from a
different family) is open.

## 7. The black box: parity, replay, and causal curvature

Theorem 1 converts the associator from a nuisance into an audit primitive:

1. **`assoc_parity`.** Every statechain entry records the number of
   reflection defects incurred by the actual evaluation path, mod 2. By
   coherence this single bit, together with the value, pins the computation
   within its orbit.
2. **Replay verifies the route.** Re-evaluating a logged tree must reproduce
   value *and* parity: "the system got this answer *by this causal path*."
   A CRC log can attest bytes; it cannot attest evaluation order.
3. **Causal curvature.** Define the reflection-defect rate per tick. The
   lattice baseline for random fields (≈ 0.22–0.26, §5) supplies a null
   distribution, making watchdog anomaly detection a falsifiable PASS/FAIL
   statistic in the style of the project's five propositions.

## 8. GPL vs CRC: the honest comparison

The kairos protocol layer computes a two-part checksum — the compose fold
acc = (…(Φ0∘x₁)∘x₂…)∘x_L and the sheet sum χ = Σ √α/y(xᵢ) — plus a
sliding-triple topology signature. We compared detection rates against
CRC-8 (poly 0x07) and CRC-16/CCITT on the packed bytes. Streams of 64
symbols (16 bytes), 20,000 trials per error class:

| Error class | GPL (acc+χ, ≈16 bits) | GPL + topology trace | CRC-8 (8 bits) | CRC-16 (16 bits) |
|---|---|---|---|---|
| Single substitution | 1.0000 | 1.0000 | 1.0000 | 1.0000 |
| Adjacent transposition | 0.0126 | 0.5860 | 1.0000 | 1.0000 |
| Single reflection (x→3−x) | 1.0000 | 1.0000 | 1.0000 | 1.0000 |
| Burst (4 symbols) | 0.9286 | 0.9833 | 1.0000 | 1.0000 |
| Full permutation | 0.4377 | **1.0000** | 0.9918 | 0.9999 |
| χ-evading pair (ρ on a Φ1 and a Φ2) | 0.0326 | 0.6515 | 1.0000 | 1.0000 |

Overhead accounting: the χ value lives on an integer lattice carrying
≈ 14 bits at this length, so compact GPL ≈ 16 bits — comparable to CRC-16.
The topology signature is L−2 bits of *reference trace*, not a compact
checksum, and is scored separately.

**Findings, stated plainly.**
- **CRC wins raw detection.** In five of six classes CRC-8 alone is at or
  near 1.0000, including the adversarial χ-evading class we constructed
  (the sheet weights satisfy s(Φ2) = −s(Φ1), so a paired reflection is
  invisible to χ; CRC sees the changed bits regardless). Any deployment
  needing plain error detection should carry a CRC.
- **GPL's genuine contribution is diagnosis, not detection.** The topology
  trace achieves 1.0000 on full permutations (edging CRC-8's 0.9918) and,
  uniquely, *classifies* what happened: a defect is either "same" or
  "reflection," localized to a triple index, in the phase domain with no
  binary packing. CRC returns one opaque bit; GPL returns *where* and
  *what kind*.
- **The two are complements.** The honest architecture is CRC for integrity
  plus GPL topology + `assoc_parity` for causal semantics — which is
  precisely the black-box role (§7).

## 9. Conclusions and program

A single derived equation yields the smallest nontrivial member of the
twisted-algebra family: Z₂ twisted by reflection, coherent for all words,
alone in its one-parameter family in possessing that coherence, with lattice
dynamics that relax fast, oscillate where they should, and expose the
associator as a measurable bulk field. The algebra will not replace CRC and
does not need to: its niche is *supervisory semantics* — well-defined values
up to one auditable bit, replayable causal routes, and a curvature statistic
with a computable null.

**Open problems.** (1) Impossibility or construction of coherent GPL(2n) for
n ≥ 3. (2) Classification of the period-12 lattice orbits and 2-D lattices.
(3) Cohomological formulation: is the GPL associator a coboundary datum in a
suitable quasi-algebra sense? (4) Hardware curvature readout on iCE40.

## Acknowledgments

The mathematics originated with Richard Careaga; the operating system,
computations, and this write-up were produced in collaboration with AI
systems (Claude/Anthropic, ChatGPT/OpenAI, Auto/Cursor) under his direction.
Per the project's standing rule: *the engineering is real; the honest
version is the strong version.*

## Reproducibility

```
python kairos/scripts/research/gpl_next_level.py   # Theorems 1–2 evidence, alpha window
python kairos/scripts/research/gpl_program.py      # atlas, CA, GPL(6), CRC Monte Carlo
```

Rust ground truth: `kairos/src/core/gphase_core.rs` (tests include the
derived-not-hardcoded root check), FPGA physical verification:
`kairos/fpga/evidence/PROOF_REPORT_2026-07-13.md`.
