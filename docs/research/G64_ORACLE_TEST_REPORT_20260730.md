# G64 Oracle Test Report — 2026-07-30

## Verdict

**Host canonical oracle generation and algebra census: PASS.**

Classification: **SIMULATION VERIFIED** / host-side oracle (from live `gphase_core`).

**Hardware follow-on (same census on COM12):** see [`G64_PHYSICAL_COM12_REPORT_20260730.md`](G64_PHYSICAL_COM12_REPORT_20260730.md) — run `run_20260730_175025` is **PHYSICALLY VERIFIED** (`g64 atlas: PASS identity=52 reflection=12 error=0 (hw==gphase_core)`).

| Gate | Result |
|------|--------|
| Compose pairs | **16 / 16 PASS** |
| Reflect map | **4 / 4 PASS** |
| Ordered triples (atlas) | **64 / 64 PASS** |
| Census | **IDENTITY 52 · REFLECTION 12 · ERROR 0** |
| Unexplained / illegal parity | **0** |

---

## What was run

| Item | Value |
|------|--------|
| Tool | `gpl_oracle --g64-research-out fpga/zynq/research/g64_phase_fabric_sidecar/oracle` |
| Token gate | `GATE=G0_MATH_ORACLE` → `RESULT=G0_MATH_ORACLE_PASS` |
| Math authority | `kairos::core::gphase_core` only (no hand-copied tables) |
| Source commit recorded in oracle | `0e89807c3876328562e0a07a301c2cc57970f6ea` |
| Branch (research) | `research/g64-phase-fabric-sidecar` |
| Timestamp (artifact mtime) | 2026-07-30 ~16:31 local |

Also covered by unit test `core::gphase_core::tests::topology_64_triples` (same 52/12 census).

### Artifact digests

From `fpga/zynq/research/g64_phase_fabric_sidecar/oracle/SHA256SUMS.txt`:

| File | SHA-256 |
|------|---------|
| `golden_phase_oracle.bin` | `99eca89134347850322269f6c6e55d6ab3f7dd1cdaa1237988c6c5b94448183b` |
| `golden_phase_oracle.json` | `c8a31184e90454ecffb7b3c751635cf1870d4b2ea0bd028405b7156f083a7645` |
| `golden_phase_oracle.md` | `a0a22d214b5b5a4487c95b815595bfc9e1f49147b5a78c1882935852a11356d7` |

Token echo also stored at `fpga/zynq/evidence/g0_math_oracle/G0_TOKEN.txt`.

### Encoding

| Symbol | Bits / value |
|--------|----------------|
| Φ0 | 0 |
| Φ1 | 1 |
| Φ2 | 2 |
| Φ3 | 3 |

For each triple `(a,b,c)`:

- `left  = (a ∘ b) ∘ c`
- `right = a ∘ (b ∘ c)`
- **IDENTITY** if `left == right`
- **REFLECTION** if `right == ρ(left)` with `ρ(x) = 3 - x`
- **ERROR** otherwise (none observed)

Index order: `idx = a*16 + b*4 + c` (a outer, then b, then c), matching G64 slot load order.

---

## Compose table (16)

| a | b | a ∘ b |
|---|---|-------|
| 0 | 0 | 0 |
| 0 | 1 | 1 |
| 0 | 2 | 2 |
| 0 | 3 | 3 |
| 1 | 0 | 1 |
| 1 | 1 | 3 |
| 1 | 2 | 3 |
| 1 | 3 | 2 |
| 2 | 0 | 2 |
| 2 | 1 | 3 |
| 2 | 2 | 3 |
| 2 | 3 | 1 |
| 3 | 0 | 3 |
| 3 | 1 | 2 |
| 3 | 2 | 1 |
| 3 | 3 | 0 |

---

## Reflect map (4)

| a | ρ(a) |
|---|------|
| 0 | 3 |
| 1 | 2 |
| 2 | 1 |
| 3 | 0 |

---

## Full 64-triple atlas

| idx | a | b | c | left | right | class |
|-----|---|---|---|------|-------|-------|
| 0 | 0 | 0 | 0 | 0 | 0 | IDENTITY |
| 1 | 0 | 0 | 1 | 1 | 1 | IDENTITY |
| 2 | 0 | 0 | 2 | 2 | 2 | IDENTITY |
| 3 | 0 | 0 | 3 | 3 | 3 | IDENTITY |
| 4 | 0 | 1 | 0 | 1 | 1 | IDENTITY |
| 5 | 0 | 1 | 1 | 3 | 3 | IDENTITY |
| 6 | 0 | 1 | 2 | 3 | 3 | IDENTITY |
| 7 | 0 | 1 | 3 | 2 | 2 | IDENTITY |
| 8 | 0 | 2 | 0 | 2 | 2 | IDENTITY |
| 9 | 0 | 2 | 1 | 3 | 3 | IDENTITY |
| 10 | 0 | 2 | 2 | 3 | 3 | IDENTITY |
| 11 | 0 | 2 | 3 | 1 | 1 | IDENTITY |
| 12 | 0 | 3 | 0 | 3 | 3 | IDENTITY |
| 13 | 0 | 3 | 1 | 2 | 2 | IDENTITY |
| 14 | 0 | 3 | 2 | 1 | 1 | IDENTITY |
| 15 | 0 | 3 | 3 | 0 | 0 | IDENTITY |
| 16 | 1 | 0 | 0 | 1 | 1 | IDENTITY |
| 17 | 1 | 0 | 1 | 3 | 3 | IDENTITY |
| 18 | 1 | 0 | 2 | 3 | 3 | IDENTITY |
| 19 | 1 | 0 | 3 | 2 | 2 | IDENTITY |
| 20 | 1 | 1 | 0 | 3 | 3 | IDENTITY |
| 21 | 1 | 1 | 1 | 2 | 2 | IDENTITY |
| 22 | 1 | 1 | 2 | 1 | 2 | REFLECTION |
| 23 | 1 | 1 | 3 | 0 | 3 | REFLECTION |
| 24 | 1 | 2 | 0 | 3 | 3 | IDENTITY |
| 25 | 1 | 2 | 1 | 2 | 2 | IDENTITY |
| 26 | 1 | 2 | 2 | 1 | 2 | REFLECTION |
| 27 | 1 | 2 | 3 | 0 | 3 | REFLECTION |
| 28 | 1 | 3 | 0 | 2 | 2 | IDENTITY |
| 29 | 1 | 3 | 1 | 3 | 3 | IDENTITY |
| 30 | 1 | 3 | 2 | 3 | 3 | IDENTITY |
| 31 | 1 | 3 | 3 | 1 | 1 | IDENTITY |
| 32 | 2 | 0 | 0 | 2 | 2 | IDENTITY |
| 33 | 2 | 0 | 1 | 3 | 3 | IDENTITY |
| 34 | 2 | 0 | 2 | 3 | 3 | IDENTITY |
| 35 | 2 | 0 | 3 | 1 | 1 | IDENTITY |
| 36 | 2 | 1 | 0 | 3 | 3 | IDENTITY |
| 37 | 2 | 1 | 1 | 2 | 1 | REFLECTION |
| 38 | 2 | 1 | 2 | 1 | 1 | IDENTITY |
| 39 | 2 | 1 | 3 | 0 | 3 | REFLECTION |
| 40 | 2 | 2 | 0 | 3 | 3 | IDENTITY |
| 41 | 2 | 2 | 1 | 2 | 1 | REFLECTION |
| 42 | 2 | 2 | 2 | 1 | 1 | IDENTITY |
| 43 | 2 | 2 | 3 | 0 | 3 | REFLECTION |
| 44 | 2 | 3 | 0 | 1 | 1 | IDENTITY |
| 45 | 2 | 3 | 1 | 3 | 3 | IDENTITY |
| 46 | 2 | 3 | 2 | 3 | 3 | IDENTITY |
| 47 | 2 | 3 | 3 | 2 | 2 | IDENTITY |
| 48 | 3 | 0 | 0 | 3 | 3 | IDENTITY |
| 49 | 3 | 0 | 1 | 2 | 2 | IDENTITY |
| 50 | 3 | 0 | 2 | 1 | 1 | IDENTITY |
| 51 | 3 | 0 | 3 | 0 | 0 | IDENTITY |
| 52 | 3 | 1 | 0 | 2 | 2 | IDENTITY |
| 53 | 3 | 1 | 1 | 3 | 0 | REFLECTION |
| 54 | 3 | 1 | 2 | 3 | 0 | REFLECTION |
| 55 | 3 | 1 | 3 | 1 | 1 | IDENTITY |
| 56 | 3 | 2 | 0 | 1 | 1 | IDENTITY |
| 57 | 3 | 2 | 1 | 3 | 0 | REFLECTION |
| 58 | 3 | 2 | 2 | 3 | 0 | REFLECTION |
| 59 | 3 | 2 | 3 | 2 | 2 | IDENTITY |
| 60 | 3 | 3 | 0 | 0 | 0 | IDENTITY |
| 61 | 3 | 3 | 1 | 1 | 1 | IDENTITY |
| 62 | 3 | 3 | 2 | 2 | 2 | IDENTITY |
| 63 | 3 | 3 | 3 | 3 | 3 | IDENTITY |

### Reflection triples only (12)

| idx | (a,b,c) | left | right | check `right == ρ(left)` |
|-----|---------|------|-------|---------------------------|
| 22 | (1,1,2) | 1 | 2 | ρ(1)=2 |
| 23 | (1,1,3) | 0 | 3 | ρ(0)=3 |
| 26 | (1,2,2) | 1 | 2 | ρ(1)=2 |
| 27 | (1,2,3) | 0 | 3 | ρ(0)=3 |
| 37 | (2,1,1) | 2 | 1 | ρ(2)=1 |
| 39 | (2,1,3) | 0 | 3 | ρ(0)=3 |
| 41 | (2,2,1) | 2 | 1 | ρ(2)=1 |
| 43 | (2,2,3) | 0 | 3 | ρ(0)=3 |
| 53 | (3,1,1) | 3 | 0 | ρ(3)=0 |
| 54 | (3,1,2) | 3 | 0 | ρ(3)=0 |
| 57 | (3,2,1) | 3 | 0 | ρ(3)=0 |
| 58 | (3,2,2) | 3 | 0 | ρ(3)=0 |

### Masks (bit `idx` set when class matches)

| Mask | Value |
|------|--------|
| identity | `0xF99FF55FF33FFFFF` (52 bits) |
| reflection | `0x06600AA00CC00000` (12 bits) |
| error | `0x0000000000000000` (0 bits) |

---

## Honest scope

| Claim | Status |
|-------|--------|
| Canonical software atlas is 52/12/0 with 0 errors | **PASS** (this report) |
| GPLV1 84-record blob matches `gphase_core` | **PASS** |
| G64 FPGA sidecar matches these masks on COM12 | **PHYSICALLY VERIFIED** — [`G64_PHYSICAL_COM12_REPORT_20260730.md`](G64_PHYSICAL_COM12_REPORT_20260730.md) / `run_20260730_175025` |
| Constant-folded “fake 64-lane” synthesis | **N/A** (this host report is math only) |

Machine-readable source of truth remains:

- `fpga/zynq/research/g64_phase_fabric_sidecar/oracle/golden_phase_oracle.json`
- `fpga/zynq/research/g64_phase_fabric_sidecar/oracle/golden_phase_oracle.bin`
