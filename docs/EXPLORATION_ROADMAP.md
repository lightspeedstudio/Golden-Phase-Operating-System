# GPL Exploration Roadmap — 10 Delegable Avenues
**Purpose:** each avenue is a self-contained brief for a smaller/cheaper agent.
The head architect (Fable) integrates results. Every brief ends with a fixed
REPORT format — reports must contain *reproducible numbers*, never vibes.

**Standing rules for every agent (paste with each brief):**
- Repo: `C:\Users\richa\OneDrive\Desktop\FullOS\kairos`. Build Rust with
  `CARGO_TARGET_DIR=%TEMP%\kairos-target`. Never reimplement `compose` — import
  from `src/core/gphase_core.rs` (Rust) or replicate the exact table
  `((0,1,2,3),(1,3,3,2),(2,3,3,1),(3,2,1,0))` (Python) and assert it against
  `scripts/research/gpl_program.py` conventions.
- Read `docs/THEOREMS_2026-07-23.md` and `docs/paper/OSF_GPL_MATHEMATICS.md`
  first.
- Honest rule: negative results are results. Never overclaim. PASS/FAIL only
  from computed checks.
- Do not git commit. Leave work in the tree + write the report file.
- Reports go in `docs/reports/AVENUE_<nn>_<slug>.md`.

---

## Avenue 1 — Attractor census of the 1-D phase lattice (MATH · easy)
**Goal:** complete classification of rule-L ring dynamics for N = 3..14.
**Tasks:** exhaustive functional-graph analysis per N (extend the N=8 method in
`scripts/research/gpl_program.py` §2): count cycles by length, basin sizes,
transient max/mean. Special focus: the period-12 orbits at N=8 — list them
explicitly, check whether they are rotations/reflections/ρ-images of each
other (symmetry group of the ring × ρ).
**Report:** table N vs {cycle spectrum, attractor fraction, max transient};
explicit period-12 orbit list + their symmetry classification; conjecture on
how cycle lengths scale with N.

## Avenue 2 — 2-D phase lattice (MATH · medium)
**Goal:** first look at 2-D torus dynamics — does the curvature field form
structures (domains, walls, gliders)?
**Tasks:** M×M torus (M=16,32). Update rules to compare: (a) row rule then
column rule; (b) x′ = ((N∘x)∘S)∘((E∘x)∘W) fixed grouping; document the exact
rule used. Measure: defect density vs time, spatial autocorrelation of the
defect field, state histogram, snapshots (ASCII dumps) of interesting frames.
Search specifically for propagating or stable localized patterns.
**Report:** defect-density curves, correlation length estimate, any localized
structure with its ASCII evolution, and the exact rule definition used.

## Avenue 3 — Sibling algebras #2 and #3 deep map (MATH · easy)
**Goal:** understand what replaces coherence above α*.
**Tasks:** for both sibling tables (see THEOREMS doc §3): list all 10 "other"
triples and their (L,R) pairs; determine whether ANY involution σ on {0..3}
controls the defect (search all 4! permutations: does R = σ(L) always hold
for some fixed σ?); compute word-coherence failure rate for lengths 3–7
(fraction of words whose evaluation set spans >2 values / >1 orbit under
every candidate orbit partition); check for associative quotients under every
partition of {0,1,2,3}.
**Report:** defect tables, verdict "controlled by σ / uncontrolled", failure
statistics, any quotient found.

## Avenue 4 — Wider GPL(2n) search (MATH · medium)
**Goal:** find a coherent 6- or 8-state algebra, or strengthen the negative.
**Tasks:** generalize `make6` in `scripts/research/gpl_program.py` §3:
(a) drop the golden-trace constraint (3 free root params, finer grid ≥200³
via smart pruning or random sampling ≥10⁶ points); (b) 8 states (4 root
pairs); (c) for every candidate check identity, reflection row, orbit
coherence, and associativity of the quotient on orbits (quotients Z₃, Z₄,
Z₂×Z₂ for n=4). Log near-misses (fail exactly one property).
**Report:** counts per property combination, best near-misses with parameters,
verdict: coherent GPL(6)/GPL(8) found or not in families searched.

## Avenue 5 — Associator cocycle & loop-theory placement (MATH · medium, mostly literature+small compute)
**Goal:** place GPL precisely in the non-associative taxonomy.
**Tasks:** define ω(a,b,c) ∈ {0,1} (0=same, 1=reflection). Computationally
test standard identities: flexible law (a∘b)∘a = a∘(b∘a); alternative laws;
Bol and Moufang identities; power-associativity; whether ω satisfies the
3-cocycle-style identity ω(b,c,d)+ω(a,b∘c... use orbit-projected arguments
since GPL isn't a group — document the exact identity variant tested.
Literature: commutative magmas with involutive associator control; quasialgebras
(Albuquerque–Majid); report closest known class.
**Report:** identity-by-identity PASS/FAIL table (computed over all tuples),
the cocycle verdict with the exact formula tested, and a paragraph placing
GPL relative to known structures with citations.

## Avenue 6 — Statechain spec + adversarial fuzz (OS/KERNEL · medium)
**Goal:** a formal spec document and measured tamper-detection rates.
**Tasks:** reverse-engineer the statechain wire/hash format from source into a
spec (field-by-field, hash inputs, chain linkage, checkpoint sealing —
include `assoc_parity` once merged). Then property-based fuzz: random valid
chains, apply N=10,000 random tamperings per class (bit flip in payload, in
hash, in parity, entry deletion, entry reorder, splice of two chains), and
measure audit detection rate per class.
**Report:** `docs/STATECHAIN_SPEC.md` + detection-rate table per tamper class
(any class below 1.0000 is a finding, not a failure — report it honestly).

## Avenue 7 — Hybrid integrity frame for PhaseLink (PROTOCOL · medium)
**Goal:** implement the paper's honest conclusion: CRC for detection + GPL for
diagnosis, as one frame format.
**Tasks:** design frame = payload + CRC-16/CCITT + GPL acc+χ (16-bit
quantized) + assoc_parity + optional topology trace. Implement encode/verify
in `src/kernel/phaselink/` style (or standalone module + tests). Re-run the
Monte Carlo of `gpl_program.py` §4 against the hybrid: detection AND
classification accuracy (did the diagnosis correctly label
substitution/transposition/reflection and localize within ±2 symbols?).
**Report:** frame spec, detection table (should be ≥ CRC-16 alone in every
class), classification/localization accuracy table, overhead in bits.

## Avenue 8 — Causal-curvature watchdog statistics (OS/KERNEL · medium)
**Goal:** turn curvature into a falsifiable PASS/FAIL watchdog statistic.
**Tasks:** instrument the hosted runtime to log reflection-defect counts per
tick under 4+ workloads (idle, spawn-storm, VM program loops, fs churn).
Collect ≥10⁵ ticks each; fit/tabulate the null distribution per workload;
implement a prototype `curvature` shell command showing rate + rolling
z-score; choose and justify a threshold (e.g. p<0.001) and demonstrate one
injected anomaly (forced phase flips) being flagged.
**Report:** null-distribution tables/plots (ASCII ok), chosen thresholds,
demo transcript of detection, false-positive rate measured over clean runs.

## Avenue 9 — FPGA lattice scaling study (HARDWARE · medium)
**Goal:** how big can the phase lattice get on UP5K, and what does 2-D cost?
**Tasks:** parameterize `phase_lattice_uart.v` N ∈ {16, 32, 64, 128, 256}
(1-D) and 4×4/8×8 (2-D torus); build each with the existing yosys/nextpnr
flow; record ICESTORM_LC usage, Fmax, build success. No flashing required
(sim-verify one mid-size and the largest that fits against
`verify_lattice.py` extended). Extrapolate cells-per-LUT and the max lattice
on UP5K and on a hypothetical ECP5-85F.
**Report:** resource/Fmax table vs N, the scaling law, largest verified
build, 2-D feasibility verdict.

## Avenue 10 — Layer 11: lab craft fixture + FDIR replay (MISSION · hard)
**Goal:** the first mission-layer demo — phase-governed subsystems with a
scripted fault and audited recovery.
**Tasks:** implement `craft` objects (nav, prop, power) as kernel processes
with phase semantics per `ULTIMATE_VISION.md` §5; shell commands
`craft status`, `phase set <obj> <state>` (audited to statechain),
`phase correct <obj>` (watchdog-assisted to safe state); one scripted
scenario: prop stuck in Φ2 → watchdog detect → quarantine → recover →
checkpoint; then `replay` reconstructs the episode from the chain and the
transcript shows value+parity verification.
**Report:** files added, full demo transcript, test count added/passing, gaps
found in existing kernel APIs.

---

## Priority guidance (architect's ordering)
- **Fastest wins:** 1, 3 (pure compute, low risk) → feed the OSF paper rev 2.
- **Highest mission value:** 8, 10 (watchdog + craft demo) → the product story.
- **Highest math risk/reward:** 4, 5 (a coherent GPL(6) or a clean taxonomy
  placement would each be a headline).
- **Hardware continuity:** 9 keeps the silicon lane warm between board buys.
- 6 and 7 harden what already exists and generate spec documents reviewers ask for.

## Report intake checklist (for the architect on each report)
1. Do the numbers reproduce? (rerun the agent's script once)
2. Does any claim exceed what was computed? (strike it)
3. What does it change in: the paper · the kernel spec · the roadmap?
4. Fold into `docs/paper/OSF_GPL_MATHEMATICS.md` rev or `ROADMAP.md`.
