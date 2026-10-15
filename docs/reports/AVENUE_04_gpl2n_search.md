# Avenue 4 — Wider GPL(2n) Search

**Date:** 2026-07-24
**Script:** `scripts/research/avenue04_gpl2n_search.py` (new; does not reimplement
the primary GPL(4) compose — imports/replicates and asserts it against the
exact table `((0,1,2,3),(1,3,3,2),(2,3,3,1),(3,2,1,0))` from
`gpl_program.py`, same grounding pattern as `avenue01_attractor_census.py`).
**Reproduction:** `python scripts/research/avenue04_gpl2n_search.py`

## Verdict (headline)

**Coherent GPL(6) and GPL(8): NOT FOUND**, in the generalized ± -root-pair /
affine-fold-and-snap family, unconstrained (no golden-trace requirement), with
1,000,000 random samples each. This **strengthens** the existing honest
negative result in `docs/paper/OSF_GPL_MATHEMATICS.md` §6 (which used a
golden-trace-constrained 391-point grid and found the same qualitative
outcome). The broader, unconstrained, 1,000,000-point search still finds
**zero** candidates whose orbit quotient is an associative group.

| Family | Points tested (non-degenerate) | Orbit-coherent (all 4 basic checks minus quotient) | Coherent **and** quotient-is-group | Distinct coherent-but-non-group tables found |
|---|---|---|---|---|
| GPL(6), n=3 free root-squares | 1,000,000 | 1,624 (0.162%) | **0** | **1** |
| GPL(8), n=4 free root-squares | 1,000,000 | 67 (0.0067%) | **0** | **1** |

Every one of the 1,624 (GPL6) / 67 (GPL8) orbit-coherent candidates collapsed
onto a **single distinct integer table** in each family (see below) — a
numerically degenerate structure, not a spread of genuinely different
near-coherent algebras.

## Method

Generalization of `make6()` (`gpl_program.py` §3) to 2n states, dropping the
golden-trace constraint entirely:

- n free positive "root-squares" z₁..z_n → r_i = √z_i → root positions
  s = (−r_n,…,−r_1, r_1,…,r_n) (2n values, the ± pairs).
- Affine map of s onto the rail [0, π/φ]; snap boundaries at the midpoints
  of adjacent mapped positions (2n−1 boundaries, 2n bins).
- `a∘b = snap(fold(s_a + s_b))`, `fold(x) = x if x ≤ RAIL else 2·RAIL − x`.
- GPL(6): n=3 free z's. GPL(8): n=4 free z's.

**Honest note on the sampling family.** The construction is invariant under
any common positive rescaling of the z_i (the affine normalization divides
out both the additive offset and the overall scale of the root positions).
So the resulting table depends only on the **ratios** z₁:z₂:…:z_n — a point
on the (n−1)-simplex — not on the n raw magnitudes. Drawing z_i i.i.d.
Uniform(0,1) and sorting, as done here, is not a perfectly uniform sampling
of that simplex, but it is the "random sampling ≥1,000,000 points"
alternative the roadmap brief explicitly offers in place of a fine grid, and
it covers a very wide range of ratios (down to ~10⁻³–10⁻⁵ scale
separations, as seen in the near-miss z's below). This is disclosed, not
hidden: a different sampling measure on the simplex is a natural follow-up
if this result is to be tightened into an impossibility claim rather than a
strengthened negative.

Every candidate table is checked, purely computationally, for:
1. **identity** — `t[0][x] == x` for all x
2. **reflection row** — row 2n−1 equals the exact reverse `(2n−1,…,0)`
3. **orbit-coherence** — for all a,b, `{a∘b, a∘ρb, ρa∘b, ρa∘ρb}` lies inside
   one reflection orbit `{v, 2n−1−v}` (this is the 16-case-style check from
   Theorem 1(i), generalized)
4. **quotient-is-group** — the orbit quotient (n elements, canonical
   low-half representatives 0..n−1) is associative with identity and
   inverses. For n=3 the only group of order 3 up to isomorphism is Z₃; for
   n=4 the only groups of order 4 up to isomorphism are Z₄ and Z₂×Z₂ — so
   "is a group" and "matches a named target" are mathematically equivalent
   for these n. The script verifies the explicit isomorphism (permutation
   search fixing the identity) rather than assuming it.

A **near-miss** = a candidate failing **exactly one** of the four checks.

## Result 1 — identity and reflection-row are structurally automatic

Across all 2,000,000 draws (both families), **identity and reflection-row
held 100% of the time** whenever the table was non-degenerate (0 degenerate
draws out of 1,000,000 in each family, at the tolerance used). This is not a
surprise post hoc: state 0 always maps to the minimum root position (which
normalizes to coordinate 0, landing back in its own bin by construction),
and state 2n−1 always maps to the maximum (coordinate RAIL), whose
fold-and-snap under the ± -symmetric layout is forced into the mirror bin of
whatever it's composed with. Both properties are therefore near-tautological
consequences of the ± -pair/affine-normalization construction, not
discriminating filters — the roadmap's checks (3) orbit-coherence and (4)
quotient-associativity are where the real selection happens, exactly as
foreshadowed by the original GPL(6) probe's finding that 4-state coherence
"needs the quotient to be an associative group."

## Result 2 — property-combination counts

GPL(6) (n=3, 1,000,000 tested, seed=406):

| (ident, refl, coh, is_group) | count |
|---|---|
| (True, True, False, False) | 998,376 |
| (True, True, True, **False**) | 1,624 |
| (True, True, True, **True**) | **0** |

GPL(8) (n=4, 1,000,000 tested, seed=408):

| (ident, refl, coh, is_group) | count |
|---|---|
| (True, True, False, False) | 999,933 |
| (True, True, True, **False**) | 67 |
| (True, True, True, **True**) | **0** |

100% of near-misses in both families fail on exactly the same check:
`is_group` (the quotient fails associativity). Zero near-misses fail
orbit-coherence alone or the trivial checks alone.

## Result 3 — the near-misses are one degenerate table, not a spread

Deduplicating the 1,624 (GPL6) / 67 (GPL8) coherent-but-non-group hits by
their actual integer compose table shows **exactly one distinct table** in
each family accounts for **all** of them:

**GPL(6) near-miss table** (example z ≈ (0.00442, 0.01034, 0.28800), i.e.
two root-squares close together and small, one much larger):

```
      0  1  2  3  4  5
  0   0  1  2  3  4  5
  1   1  5  5  5  5  4
  2   2  5  5  5  5  3
  3   3  5  5  5  5  2
  4   4  5  5  5  5  1
  5   5  4  3  2  1  0
```
topology over 216 triples: same=136, reflection=48, other=32.

Quotient (representatives 0,1,2 → orbits {0,5},{1,4},{2,3}):

```
q(i,j):  0 1 2
      0: 0 1 2
      1: 1 0 0
      2: 2 0 0
```
First associativity failure: `(i,j,k)=(1,1,2)`: `q(q(1,1),2)=q(0,2)=2` but
`q(1,q(1,2))=q(1,0)=1` — 2 ≠ 1.

**GPL(8) near-miss table** (example z ≈ (0.0153, 0.0186, 0.0277, 0.7672)):

```
      0  1  2  3  4  5  6  7
  0   0  1  2  3  4  5  6  7
  1   1  7  7  7  7  7  7  6
  2   2  7  7  7  7  7  7  5
  3   3  7  7  7  7  7  7  4
  4   4  7  7  7  7  7  7  3
  5   5  7  7  7  7  7  7  2
  6   6  7  7  7  7  7  7  1
  7   7  6  5  4  3  2  1  0
```
topology over 512 triples: same=260, reflection=108, other=144.

Quotient (representatives 0..3 → orbits {0,7},{1,6},{2,5},{3,4}):

```
q(i,j):  0 1 2 3
      0: 0 1 2 3
      1: 1 0 0 0
      2: 2 0 0 0
      3: 3 0 0 0
```
Same failure shape: `(1,1,2)`: `q(q(1,1),2)=q(0,2)=2 ≠ q(1,q(1,2))=q(1,0)=1`.

**Reading.** Both "best near-misses" are the same numerically-driven
degenerate structure: whenever two or more of the free root-squares are
close together and much smaller than the rest, the middle interior states
collapse — every pairwise composition of two non-boundary, non-identity
states saturates to the top state (2n−1), because the folded sum of two
small-magnitude, closely-spaced coordinates still overshoots past RAIL and
snaps to the last bin. The resulting quotient is "everything nonzero
absorbs to identity except against 0," which is a commutative but
structurally trivial-looking magma that is *not* associative (shown above).
This is a genuine, reproducible near-miss by the letter of the four checks,
but it is a corner-of-parameter-space degeneracy, not evidence of a
richer nearby coherent structure — no other topology or quotient shape
appeared among the 1,624 + 67 orbit-coherent hits.

## Comparison to the existing GPL(6) negative result

`docs/paper/OSF_GPL_MATHEMATICS.md` §6 / `docs/THEOREMS_2026-07-23.md`
report: golden-trace-constrained family (z₁+z₂+z₃=φ), 391 grid points, 1
coherent point, 0 with Z₃ quotient. This avenue drops the constraint
entirely and samples 1,000,000 points (≈2,560× more coverage) — coherence
becomes much *more* common (1,624 hits vs. 1) once the trace constraint is
lifted, but the **Z₃-quotient count stays at exactly zero**, and the
GPL(8) extension (67 coherent hits, still an unconstrained 1,000,000-point
search) likewise finds zero Z₄/Z₂×Z₂ quotients. This is consistent with
and strengthens the existing conjecture: within the ± -root-pair /
affine-fold-and-snap family, going from 4 states (quotient Z₂, cannot fail
to associate — the only group structure on 2 elements) to 6 or 8 states
(quotient must associate over 3 or 4 elements, which is not automatic)
appears to be the actual obstruction, not the golden-trace constraint.

## Caveats (honest, not overclaiming)

- This is a **strengthened negative in the searched family**, not an
  impossibility proof. The z_i ~ Uniform(0,1) sampling is not a rigorously
  uniform measure on the ratio simplex; a different measure, a directed
  grid refinement around the boundary between "coherent" and "not," or an
  algebraic (rather than numerical) approach to the associativity
  obstruction could still be tried.
- Only **one** construction family was searched (the exact ± -root-pair,
  affine-normalize, midpoint-snap, fold-onto-[0,RAIL] recipe that produces
  the primary GPL(4) table). A structurally different degree-2n kernel
  (e.g., not built from ± pairs, or with an asymmetric fold) is out of
  scope here and remains open, as already flagged in the paper's Open
  Problems.
- No degenerate (near-tied root magnitude) draws occurred in either 1M-point
  run, so the "degenerate" rejection path is exercised but did not affect
  these particular counts.

## Reproduction

```
python scripts/research/avenue04_gpl2n_search.py
```

Deterministic given the fixed seeds in the script (`seed=406` for GPL(6),
`seed=408` for GPL(8)); total runtime ≈107s on the dev machine (37.0s +
69.9s) for 2,000,000 total candidate evaluations.
