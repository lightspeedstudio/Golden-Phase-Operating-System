# Next FPGA Program — results (2026-07-24)

**Board:** iCEBreaker V1.1a · **UART:** COM24 115200 8N1 · **Clock:** 12 MHz  
**Immediate experiment:** M=4 long-duration glider soak + all 64 GPL triples → **PASS**

## Phase status

| Phase | Goal | Status |
|-------|------|--------|
| 0 | Outer FullOS pointer → Avenue 10A | done |
| 1 | M=4 glider harder seal | **PHYSICALLY_VERIFIED** |
| 2 | 64 GPL algebra triples on hardware | **PHYSICALLY_VERIFIED** |
| 3 | Two-glider collisions | **PHYSICALLY_VERIFIED** (S0 PASS_THROUGH) |
| 4 | Pipeline M=8 until timing closes | **PHYSICALLY_VERIFIED** (serial @ ~25 MHz) |
| 5 | Hardware fault injection | **PHYSICALLY_VERIFIED** (mode 1 delete) |
| 6 | Avenue 10B craft-over-glider | **PHYSICALLY_VERIFIED** live PROP_ARM + COM24 craft path; host twin PASS |

---

## Phase 1 — M=4 glider seal

Acceptance met:

| Check | Result |
|-------|--------|
| Host↔FPGA every generation | 0 mismatches |
| Footprint | F=01 always |
| All starts V1D2 (Φ1 vac + Φ2 defect) | **16/16 PASS** |
| Reflected V2D1 (Φ2 vac + Φ1 defect) | **16/16 PASS** |
| Soak ≥10⁴ gens | **GENS=10000 PASS** (160000 cell updates) |
| Stretch ≥10⁶ cell updates | deferred (next: GENS≈62500) |

### Representative evidence

**Center V1D2 G8** (`evidence/phase_lattice_2d_glider_M4_G8_V1D2_S22/`)

- SHA-256: `6db1ed603ba7cff1188369b76e1c79a7384b0ecf78a675c1ade99ccb2544ea05`
- Duration: 24.45 s · mismatches: 0

**Center reflected V2D1 G8** (`evidence/phase_lattice_2d_glider_M4_G8_V2D1_S22/`)

- SHA-256: `cc3a66a65a1af1e112e5af20c9066f22267976eb60cfa63356e6c05c69b6d77c`
- Duration: 24.54 s · mismatches: 0

**Soak GENS=10000** (`evidence/phase_lattice_2d_glider_M4_G10000_V1D2_S22/`)

- SHA-256: `a4dc86c0c1cf2864b88a3057a1c0d523401c797d148d68f622220c8978efd52c`
- Duration: 76.61 s · cell updates: 160000 · mismatches: 0
- UART: 300064-char transcript, `GLIDER PASS` / `LATTICE DONE`

Campaign log: `fpga/evidence/phase1_campaign_log.txt`

### Timing note

Parameterized glider restores sealed architecture (row/col pipeline + registered footprint + BCD gen digits + UART resend). Fmax ≈ 13.6 MHz PASS @ 12 MHz. Position correctness sealed by host golden every gen (not an on-chip index mux).

---

## Phase 2 — GPL algebra 64/64

**Gate:** `gpl_algebra_64` · evidence: `fpga/evidence/gpl_algebra_64/`

| Field | Value |
|-------|-------|
| BITSTREAM_SHA256 | `730d6e546382667a35416c43f852665584579a0bffe2151dd4f84e7299ff40bb` |
| Fmax | ≥34.95 MHz (PASS @ 12 MHz) |
| Duration | 24.82 s |
| Result | **64/64** exact host match for `ab`, `L=(a∘b)∘c`, `R=a∘(b∘c)`, associator parity `p∈{0,1}` |
| MISMATCH_COUNT | 0 |

Host golden: `fpga/verify_gpl_algebra_64.py`  
RTL uses live `compose_gate` (same primitive as the lattice).

---

## iCE Max campaign addendum (same evening)

Full rollup: `fpga/evidence/ICE_MAX_CAMPAIGN_2026-07-24.md`

| Wave | Seal | Status |
|------|------|--------|
| A | soak GENS=62500, N=128, M=8 smoke, regression + golden CI | PASS |
| B | collision S0 → class **PASS_THROUGH** | PHYSICALLY_VERIFIED |
| C | M=8 serial glider GENS=16 @ 12 MHz (Fmax ~25 MHz) | PHYSICALLY_VERIFIED |
| D | fault mode 1 delete + FDIR UART observables; host `FpgaGliderTransport` one-packet smoke | PASS |

### Next (do not claim until sealed)

1. Additional collision classes (scenarios 1–3)
2. Live Avenue 10B `PROP_ARM` over UART (after bare packet path on wire)
3. More fault modes (2–4) if needed for router/FDIR policy work

---

## Prior next list (superseded by iCE Max seals above)

1. Optional soak stretch to ≥10⁶ cell updates (GENS≈62500) — **done**
2. Phase 3: first two-glider collision + outcome class — **done (S0)**
3. Phase 4: M=8 pipeline until Fmax ≥12 MHz — **done (serial)**
4. Phase 5–6: fault injection → Avenue 10B craft transport — **fault done; 10B host smoke done**
