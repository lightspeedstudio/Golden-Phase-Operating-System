# Avenue 5 — Associator Cocycle & Loop-Theory Placement

**Status:** computed, honest negative on the cocycle question, taxonomy
placement is a *near-miss*, not an identification.
**Script:** `scripts/research/avenue05_cocycle.py`
**Reproduction:** `python scripts/research/avenue05_cocycle.py`
(pure Python 3, no dependencies; table asserted against
`gpl_program.py`'s derivation from α* before anything else runs).

---

## 0. Table assertion

```
table = ((0,1,2,3), (1,3,3,2), (2,3,3,1), (3,2,1,0))
```
Matches the derivation from α* = 7.2973525693e-3 exactly (`assert` in the
script, not reimplemented). Compose is not a quasigroup operation: rows 1
and 2 (and columns 1 and 2) each contain the value 3 twice, so the table is
**not a Latin square** — GPL is a commutative unital **magma**, not a
quasigroup/loop. This matters below: the Bol/Moufang varieties are normally
defined and interpreted on loops; we test them as raw magma identities and
report the raw PASS/FAIL, flagged as such.

## 1. ω(a,b,c) — well-definedness

ω(a,b,c) = 0 if (a∘b)∘c = a∘(b∘c) ("same"); = 1 if they differ by the
reflection ρ(x)=3−x ("reflection"); undefined ("OTHER") otherwise.

| checked | same (ω=0) | reflection (ω=1) | OTHER | well-defined |
|---|---|---|---|---|
| 64 triples | 52 | 12 | 0 | **PASS** |

(This reproduces the Associator–Reflection fact already stated in
`THEOREMS_2026-07-23.md` §1 — restated here as the ω function used for
everything that follows.)

## 2. Identity-by-identity PASS/FAIL (all applicable tuples, exhaustive)

| identity | formula tested | tuples | pass | fail | verdict |
|---|---|---:|---:|---:|---|
| commutativity (anchor) | a∘b = b∘a | 16 | 16 | 0 | **PASS** |
| flexible | (a∘b)∘a = a∘(b∘a) | 16 | 16 | 0 | **PASS**† |
| left alternative | (a∘a)∘b = a∘(a∘b) | 16 | 12 | 4 | **FAIL** |
| right alternative | (b∘a)∘a = b∘(a∘a) | 16 | 12 | 4 | **FAIL** |
| power-associativity | all parenthesizations of aⁿ, n=2..7, single-valued | 4 elts × 6 lengths = 24 | 16 | 8 | **FAIL** |
| left Bol | a∘(b∘(a∘c)) = (a∘(b∘a))∘c | 64 | 48 | 16 | **FAIL** |
| right Bol | ((c∘a)∘b)∘a = c∘((a∘b)∘a) | 64 | 48 | 16 | **FAIL** |
| Moufang M1 (left) | ((a∘b)∘a)∘c = a∘(b∘(a∘c)) | 64 | 48 | 16 | **FAIL** |
| Moufang M2 (right) | c∘(a∘(b∘a)) = ((c∘a)∘b)∘a | 64 | 48 | 16 | **FAIL** |
| Moufang M3 (middle) | (a∘b)∘(c∘a) = a∘((b∘c)∘a) | 64 | 48 | 16 | **FAIL** |
| Moufang M4 (middle, alt.) | (a∘b)∘(c∘a) = (a∘(b∘c))∘a | 64 | 48 | 16 | **FAIL** |

† **Caveat, stated honestly:** for a commutative magma the flexible law is a
free corollary — x(yx) = x(xy) = (xy)x follows from commutativity alone,
independent of any other structure. Its PASS here is not independent
evidence; it is commutativity restated.

**Power-associativity detail.** a=0 and a=3 (the two identity/absorbing-ish
elements of orbit O) are power-associative for n=2..7 (single value every
time). a=1 and a=2 (orbit I) both first fail at n=4: the word "aaaa" has two
distinct fully-parenthesized values, {0,3} — i.e. power-associativity fails
*exactly on the orbit whose homogeneous ring dynamic is the period-2 clock*
noted in `THEOREMS_2026-07-23.md` (homogeneous Φ1↔Φ2 oscillation). This is
a plausible structural link, not claimed as proven.

**Bottom line on the identity table:** of eight classical Bol–Moufang-type
identities plus alternativity plus power-associativity (10 non-trivial
checks), only the free corollary (flexible) survives. GPL sits *outside*
the entire classical Bol–Moufang variety hierarchy.

## 3. Cocycle probe

Orbit map (= existing `threshold_energy`): O = {0,3} ↦ 0, I = {1,2} ↦ 1,
target group Z₂.

**3a. Is ω fiber-constant over the orbit projection?** I.e. is
ω(a,b,c) determined solely by (orb(a),orb(b),orb(c))? This is the
prerequisite for ω to induce *any* canonical cochain ω̄: Z₂³→Z₂.

| orbit-triples | conflicting | verdict |
|---|---|---|
| 8 | 3 | **FAIL** |

Conflicting orbit-triples and their split (all fiber elements shown):

```
orbits(0,1,1): (0,1,1)->0 (0,1,2)->0 (0,2,1)->0 (0,2,2)->0
               (3,1,1)->1 (3,1,2)->1 (3,2,1)->1 (3,2,2)->1
orbits(1,1,0): (1,1,0)->0 (1,2,0)->0 (2,1,0)->0 (2,2,0)->0
               (1,1,3)->1 (1,2,3)->1 (2,1,3)->1 (2,2,3)->1
orbits(1,1,1): (1,1,1)->0 (1,2,1)->0 (2,1,2)->0 (2,2,2)->0
               (1,1,2)->1 (1,2,2)->1 (2,1,1)->1 (2,2,1)->1
```

Reading the split: within each conflicting orbit-triple, ω is constant on
whichever *specific* representative is used for the O-slot (0 vs 3) — the
identity element 0 always gives ω=0, the reflection generator 3 always
gives ω=1 — and (in orbits(1,1,1)) also depends on the exact I-representative
parity. **There is no canonical orbit-level cochain ω̄: Z₂³→Z₂ induced
directly from ω.** This is a clean negative result, not an ambiguity to be
argued away.

**3b. Pentagon / inhomogeneous 3-cocycle identity, tested anyway via
canonical-lift representatives.** Exact identity tested (G=Z₂ additive,
trivial action, standard group-cohomology 3-cochain pentagon condition — the
same style of condition that classifies associativity constraints for
group-graded quasialgebras, mod 2 throughout):

```
ω̄(h,k,l) + ω̄(g,h⊕k,l) + ω̄(g,h,k)  ==  ω̄(g,h,k⊕l) + ω̄(g⊕h,k,l)
   for all g,h,k,l ∈ Z2   (16 tuples)
```

Tested against four representative-pair choices (all combinations of
{rep(O)∈{0,3}} × {rep(I)∈{1,2}}), since 3a shows these give genuinely
different cochains, not the same map computed four ways:

| representative choice | tuples | fail | verdict |
|---|---:|---:|---|
| rep(O)=0, rep(I)=1 | 16 | 0 | **PASS** |
| rep(O)=0, rep(I)=2 | 16 | 0 | **PASS** |
| rep(O)=3, rep(I)=1 | 16 | 6 | **FAIL** |
| rep(O)=3, rep(I)=2 | 16 | 6 | **FAIL** |

**Explanation, not just result.** ω(a,b,c) = 0 whenever any argument is 0,
because Φ0 is the two-sided identity: (0∘b)∘c = b∘c = 0∘(b∘c) always. Any
representative choice with rep(O)=0 therefore forces many pentagon terms to
vanish identically, which is *why* it passes — this is a triviality of the
identity element leaking into the lift, not evidence of a genuine 3-cocycle
governing the reflection structure. The rep(O)=3 lift (using the *other*
element of the same orbit, the reflection generator, which has no such
absorbing property) fails 6 of 16 checks. Since the two representative
choices for the same orbit give opposite verdicts, **the PASS at rep(O)=0
is an artifact of the identity element, not a property of the orbit
Z₂-grading**, and does not constitute evidence that GPL's associator is
governed by a bona fide 3-cocycle on its Z₂ quotient.

### Cocycle verdict

**FAIL, honestly.** ω does not descend to a well-defined orbit-level
3-cochain (3a). The one representative choice under which the pentagon
identity holds does so only because it happens to route every failing case
through the identity element, which trivializes the check rather than
confirming it. **GPL's associator is not shown to be, and on this evidence
is not, a pullback of a genuine group 3-cocycle on its Z₂ reflection
quotient.** This does not contradict Theorem 1 (coherence) or Theorem 2
(Z₂ quotient) from `THEOREMS_2026-07-23.md` — those are weaker, orbit-level
statements about the *value* of a computation, not about ω itself
satisfying a cohomological coherence law.

## 4. Taxonomy placement

**What GPL is:** a commutative, unital (Φ0 = identity), non-associative,
non-alternative, non-power-associative, non-quasigroup (not a Latin square)
magma on 4 elements whose associator takes values in a fixed 2-element set
{id, ρ} for every triple (Theorem 1's ω), with the coarse quotient by the
fixed-point-free involution ρ being the group Z₂. Flexibility holds, but
only as a free consequence of commutativity.

**What it is not, precisely.** The classical loop-theoretic hierarchy
(alternative algebras ⊃ Moufang loops ⊃ Bol loops ⊃ flexible loops, per the
Bol–Moufang taxonomy — see Phillips & Vojtěchovský, *"The varieties of
quasigroups of Bol–Moufang type: an equational reasoning approach"*,
arXiv:math/0701715, and Kinyon–Kunen–Phillips-style commutative-Moufang
surveys) does not apply cleanly to GPL: it is not even a quasigroup, and
every non-trivial identity in that hierarchy (alternative, Bol, Moufang, in
all standard forms) **fails** computationally here (§2). GPL sits outside
this hierarchy rather than at some named rung of it.

**The closer family, and why it's a near-miss rather than a match.** The
structural resemblance drawn in `OSF_GPL_MATHEMATICS.md` §3 — "Z₂ twisted
by reflection... the same organizing pattern as the octonions" — points at
the right neighborhood: **quasialgebras / twisted group algebras**, in the
sense of Albuquerque & Majid, *"Quasialgebra Structure of the Octonions"*
(J. Algebra 220 (1999) 188–224, arXiv:math/9802116) and *"Clifford algebras
obtained by twisting of group algebras"* (J. Pure Appl. Algebra 171 (2002)
133–148). In that construction the octonions are exhibited as the group
algebra of (Z₂)³, twisted by an explicit **group 3-cocycle** valued in the
scalars — associativity fails only up to an isomorphism controlled by that
cocycle, and Z₂ⁿ-graded quasialgebras of this type are now a small
recognized family (see also the 2024–25 follow-up on "Z₂ⁿ-graded
quasialgebras and the Hurwitz problem on compositions of quadratic forms").
That is the exact template GPL's Theorem 2 language evokes: a group base
(here Z₂, not Z₂³) with associativity failure confined to a twist.

**Where the analogy breaks, computationally.** The Albuquerque–Majid
picture requires the associator to genuinely be (a coboundary/cocycle
datum) **pulled back from the grading group** — i.e., defined on and only
depending on the group-graded degree of the elements involved. §3a shows
this fails for GPL: ω is *not* determined by the orbit degrees alone; two
elements of the same orbit (0 and 3, or 1 and 2) can produce different ω
values against the same partners. GPL's associator lives partly *within*
the fibers, not purely on the base group — which is precisely what
`OSF_GPL_MATHEMATICS.md` already says informally ("all non-associativity is
confined to the fiber") but which here is shown to *break* the specific
cohomological reading (a fiber-independent 3-cocycle) rather than confirm
it. GPL is also not literally a twisted group algebra at all: its elements
are four point-like states composed by a magma operation, not formal linear
combinations over a group multiplied by a twisted convolution — the
Albuquerque–Majid construction operates one categorical level up from where
GPL lives.

**Honest placement.** GPL is best described, on current evidence, as a
**commutative, unital, flexible magma with an involution-bounded associator
image** ({id, ρ}, never anything else — Theorem 1) whose *coarsest* quotient
is associative (Z₂) but whose associator is not itself a group 3-cocycle
over that quotient. This is a genuine structural cousin of the
Albuquerque–Majid quasialgebra family — same slogan ("associativity
controlled by a Z₂-valued twist") — but not a literal instance of it, and
not a named point in the standard Bol–Moufang loop taxonomy (it fails every
non-trivial identity in that hierarchy). We did not find, in the literature
consulted, a named class matching "commutative unital magma, associator
image = fixed involution orbit pair, quotient group associative, associator
not a pullback cocycle." Absent a better match, the fair statement is: GPL
is a **new point** near, but outside, the quasialgebra family — this should
be read as an open naming/classification question, not a claim of novelty
grander than the computation supports.

## 5. Machine-readable summary (from script §4)

```
OMEGA_WELL_DEFINED=True
SAME=52 REFLECTION=12 OTHER=0 (of 64)
IDENTITY[commutativity]=PASS checked=16 fail=0
IDENTITY[flexible]=PASS checked=16 fail=0
IDENTITY[left alternative]=FAIL checked=16 fail=4
IDENTITY[right alternative]=FAIL checked=16 fail=4
IDENTITY[power-associativity]=FAIL checked=24 fail=8
IDENTITY[left Bol]=FAIL checked=64 fail=16
IDENTITY[right Bol]=FAIL checked=64 fail=16
IDENTITY[Moufang M1 (left)]=FAIL checked=64 fail=16
IDENTITY[Moufang M2 (right)]=FAIL checked=64 fail=16
IDENTITY[Moufang M3 (middle)]=FAIL checked=64 fail=16
IDENTITY[Moufang M4 (middle, alt bracketing)]=FAIL checked=64 fail=16
FIBER_CONSTANT_OMEGA_BAR=False
```

## References

- Albuquerque, H., Majid, S. "Quasialgebra Structure of the Octonions."
  *J. Algebra* 220(1), 1999, 188–224. arXiv:math/9802116.
- Albuquerque, H., Majid, S. "Clifford algebras obtained by twisting of
  group algebras." *J. Pure Appl. Algebra* 171, 2002, 133–148.
- Phillips, J. D., Vojtěchovský, P. "The varieties of quasigroups of
  Bol–Moufang type: an equational reasoning approach." arXiv:math/0701715.
- `Z₂ⁿ`-graded quasialgebras and the Hurwitz problem on compositions of
  quadratic forms (ResearchGate preprint, follow-up literature on the
  Albuquerque–Majid family).
