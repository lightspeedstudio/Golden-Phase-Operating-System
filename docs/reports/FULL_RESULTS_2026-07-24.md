# GPL Exploration — Full Results Report (Avenues 1–9)
**Date:** 2026-07-24  
**Project:** Golden Phase / GPOS (`kairos`)  
**Standing rule:** *The engineering is real; the honest version is the strong version.*  
**Sources:** `docs/reports/AVENUE_0*.md`, `docs/THEOREMS_2026-07-23.md`, `docs/paper/OSF_GPL_MATHEMATICS.md`

---

## Executive verdict

Nine parallel research avenues completed (8 fully reported; harness + report sealed today).
The math picture after this round:

1. **GPL(4) remains special** — unique coherent window in the α-family; siblings exist but lose reflection-orbit coherence.
2. **Dynamics are richer than hoped** — 1-D attractor census + a genuine **2-D glider**.
3. **GPL(6)/GPL(8) still absent** in the natural ±-root family (2M-sample strengthened negative).
4. **Taxonomy:** near Albuquerque–Majid quasialgebras, **not** a literal group 3-cocycle; outside Bol–Moufang.
5. **OS layer hardened** — statechain fuzz + hybrid CRC+GPL frame + **curvature watchdog that catches injected bursts at FPR=0**.
6. **FPGA:** unpipelined 1-D lattice practical to **N=128** on UP5K; 2-D to **8×8**.

---

## Scoreboard

| # | Avenue | Model | Result class | Headline |
|---|--------|-------|--------------|----------|
| 1 | Attractor census | Haiku | **Positive** | Period-12 at N=8 → 8 orbits / **6** symmetry classes; attractor fraction shrinks with N |
| 2 | 2-D lattice | Sonnet | **Positive** | **1-cell glider** (rule A, Φ1/Φ2 bg); rule B collapses to uniform Φ0 |
| 3 | Sibling algebras | Haiku | **Structural** | Defects controlled by σ≠ρ; coherence fails ~16%→81% as word length 3→7 |
| 4 | GPL(6)/GPL(8) | Sonnet | **Honest negative** | 1M+1M samples: **0** coherent group quotients |
| 5 | Cocycle / loops | Sonnet | **Honest near-miss** | Not Bol–Moufang; ω not fiber-constant → not a Z₂ 3-cocycle |
| 6 | Statechain fuzz | Sonnet | **Pass + finding** | 5 classes @ 1.0000; **tail deletion** undetected (by design) |
| 7 | Hybrid CRC+GPL | Sonnet | **Pass + limits** | Detection ≥ CRC; diagnosis excellent on reflection/shuffle; weak on swap/burst |
| 8 | Curvature watchdog | Sonnet | **Pass** | Workload nulls; FPR=0; injected 39-defect spike **CAUGHT** |
| 9 | FPGA scaling | Sonnet | **Measured** | N=128 timing ceiling @ 12 MHz; N=256 won't place; 8×8 2-D fits |

---

## Part I — Mathematics

### I.1 Background theorems (2026-07-23, still standing)

- **T1 Coherence:** every parenthesization of every word lands in one ρ-orbit `{v,3−v}`.
- **T2 Structure:** orbits O={Φ0,Φ3}, I={Φ1,Φ2}; quotient = **Z₂ (XOR)** = `threshold_energy`.
- **T3 Atlas:** exactly **3** algebras on α∈(0,φ²/4); only window (0, **0.096820784**) is coherent.

### I.2 Avenue 1 — Attractor census (1-D rule L)

Update: `x′ᵢ = (xᵢ₋₁∘xᵢ)∘xᵢ₊₁` on a ring.

- Exhaustive N=3..10 (and large N=11..14 sampled).
- N=8: attractor fraction **2.49%**; cycle lengths include **12:8**.
- Period-12 orbits: **8** cycles → **6** classes under (rotation × reflection × ρ).
- Transients short (mean often &lt; 5–8) — hardware-friendly.
- Conjecture: attractor fraction decays roughly exponentially in N.

### I.3 Avenue 2 — 2-D torus (headline discovery)

Two rules compared on M=16 and M=32:

| Rule | Long-run defect density | Long-run histogram | Localized structures |
|------|------------------------:|--------------------|----------------------|
| **(a)** row-then-col | ~0.40–0.43 persistent | biased toward Φ3 | **Glider** on Φ1/Φ2; breather(?) on Φ0/Φ3 |
| **(b)** von Neumann grouping | → **0** by t≈10–20 | **100% Φ0** | None (heals or full collapse to Φ0) |

**Glider (rule a, background Φ1 or Φ2):** single-cell defect, footprint ≡ 1 forever,
velocity **(−1,−1)** per tick, confirmed M=16 (40 steps) and M=32 (80 steps).

Exhaustive check: disagreements vs mirrors are **always ρ** (rule b by Theorem 1;
rule a empirically over all 4⁹ neighborhoods — extra finding).

### I.4 Avenue 3 — Siblings #2 / #3

Above α*: topology 54/0/10 (no reflection-class defects).

- Defects **are** controlled: σ₂=(2,3,0,1), σ₃=(1,0,3,2) (involutions ≠ ρ).
- Reflection-orbit coherence **fails** and worsens with word length (~81% fail at L=7).
- Each has a 2-block associative quotient under σ’s partition — **not** GPL’s Z₂ story.

⇒ Physical window uniqueness is about **ρ-coherence**, not “having some involution.”

### I.5 Avenue 4 — GPL(6) / GPL(8) wider search

Family: ± root-pairs, affine fold-and-snap, **no** golden-trace constraint.

| Family | Points | Orbit-coherent | Coherent ∧ quotient-is-group |
|--------|-------:|---------------:|-----------------------------:|
| GPL(6) | 1,000,000 | 1,624 | **0** |
| GPL(8) | 1,000,000 | 67 | **0** |

Near-misses collapse to one degenerate table per family (interior absorbs to top state;
quotient fails associativity). Strengthens the paper’s §6 negative; still not an impossibility proof outside this construction family.

### I.6 Avenue 5 — Cocycle & loop placement

Identity table (exhaustive): commutative PASS; flexible PASS (free from commutativity);
**FAIL** alternative, Bol, Moufang (all forms), power-associativity (fails at a⁴ for Φ1/Φ2).

Cocycle: ω well-defined (52 same / 12 reflection / 0 other), but **not fiber-constant**
on orbit triples → no canonical ω̄ : Z₂³→Z₂. Pentagon PASS only for lifts through identity
Φ0 (artifact), FAIL when lifting via Φ3.

**Placement:** commutative unital magma with involution-bounded associator image;
coarsest quotient Z₂; **cousin of** Albuquerque–Majid quasialgebras, **not an instance**;
outside Bol–Moufang. Open naming question — not a grander novelty claim.

---

## Part II — Systems / protocol / hardware

### II.1 Avenue 6 — Statechain spec + fuzz

Spec: `docs/STATECHAIN_SPEC.md`.

| Tamper class | Rate (10k trials) |
|--------------|------------------:|
| Payload-hash bit-flip | **1.0000** |
| Event-hash bit-flip | **1.0000** |
| `assoc_parity` flip | **1.0000** |
| Entry reorder | **1.0000** |
| Chain splice | **1.0000** |
| Entry deletion (overall) | **0.8957** |
| — non-tail deletion | **1.0000** |
| — **tail deletion** | **0.0000** |

Tail truncation leaves a shorter still-valid prefix — closed by **checkpoint** layer, not
`ObjectStatechain::verify()` alone. Also noted: checkpoint “HMAC” key is re-derivable from
public equation root (integrity tag, not secret MAC).

### II.2 Avenue 7 — Hybrid CRC + GPL frame

Frame: payload + CRC-16/CCITT + GPL acc+χ_q + `assoc_parity` + optional topology trace.
Module: `src/protocol/hybrid_frame.rs` (7 tests PASS).

Detection (L=64, 20k/class): hybrid ≥ CRC-16; in this run both hit **1.0000** all classes.

Diagnosis (when topology flags):

| Class | Classification accuracy |
|-------|------------------------:|
| Single reflection | **1.0000** |
| Full shuffle | **0.9999** |
| Single substitution | 0.7300 (ceiling entangled with ρ-collisions) |
| χ-evading pair | 0.2660 |
| Burst-4 | 0.0904 |
| Adjacent swap | **0.0471** |

Honest limit: 1-bit same/reflection topology is too coarse for multi-symbol edits.
Hybrid’s value = **CRC detect + GPL name/localize when the algebra exposes a signature**.

### II.3 Avenue 8 — Curvature watchdog (finished)

| Workload | Mean defects/tick | Clean FPR at \|z\|≥3.29 |
|----------|------------------:|------------------------:|
| idle | 0.000 | **0** |
| spawn_storm | ≈3.000 | **0** |
| vm_program_loops | ≈5.000 | **0** |
| filesystem_churn | ≈2.665 | **0** |

Injected anomaly: 39 defects after idle-0 baseline → **CAUGHT**.

Shell: `curvature`, `curvature inject <n>`.

### II.4 Avenue 9 — FPGA lattice scaling (synth/PnR, no flash)

**1-D ring on UP5K (12 MHz):**

| N | LC | Util | Fmax (MHz) | Fits |
|---|---:|-----:|-----------:|:----:|
| 16 | 767 | 14% | 23.92 | YES |
| 32 | 1122 | 21% | 18.18 | YES |
| 64 | 1818 | 34% | 14.30 | YES |
| **128** | 3265 | 61% | **12.01** | YES (timing edge) |
| 256 | 6429* | 121% | — | **NO** |

Scaling: LC ≈ **22.3·N + 405**. Sim-verified N=32 and N=128 vs Python.

**2-D:** 4×4 (17%, 21.11 MHz), 8×8 (50%, 13.82 MHz) fit; ~**1.6×** LC/cell vs 1-D.
ECP5-85F order-of-magnitude ~3.7k 1-D cells (LUT-count only; unverified).

---

## Part III — What changed in the program

| Layer | Update from this round |
|-------|------------------------|
| OSF paper | Add glider; strengthen GPL(2n) negative; soften cocycle claim to near-miss; cite sibling σ-control |
| Kernel | `assoc_parity` path + curvature log + `curvature` shell |
| Protocol | `hybrid_frame` (CRC detect + GPL diagnose) |
| Spec | `STATECHAIN_SPEC.md` + known tail-delete / checkpoint story |
| FPGA | Lattice scaling envelope for UP5K; 2-D feasible small |

---

## Part IV — What is *not* claimed

- Consciousness detection / φ-network interior scoring.
- Full GPOS on UP5K.
- Impossibility of all GPL(6) constructions forever.
- That hybrid diagnosis beats CRC on every error class.
- That OS curvature equals CA lattice curvature (~0.22–0.26).

---

## Reproduce everything

```
cd kairos
python scripts/research/avenue01_attractor_census.py
python scripts/research/avenue02_lattice_2d.py
python scripts/research/avenue03_siblings.py
python scripts/research/avenue04_gpl2n_search.py
python scripts/research/avenue05_cocycle.py
cargo test --test statechain_fuzz_tests
cargo test --lib protocol::hybrid_frame
cargo run --release --bin avenue08_curvature -- 2000
powershell -File fpga/build-phase-lattice-scale.ps1
```

Primary paper draft: `docs/paper/OSF_GPL_MATHEMATICS.md`  
This report: `docs/reports/FULL_RESULTS_2026-07-24.md`  
Per-avenue: `docs/reports/AVENUE_01_…` through `AVENUE_09_…`
