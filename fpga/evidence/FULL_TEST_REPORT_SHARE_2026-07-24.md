# Kairos FPGA / Phase-Transport — Full Test & Results Report

**Purpose:** Hand-off brief for another AI / reviewer. Honest seals only; do not inflate claims.
**Generated:** 2026-07-24 (campaign evening + P7–P10)
**Repo:** `kairos` · tip commit `3f2d371` (P7–P10 seal)
**Board:** iCEBreaker V1.1a UP5K · **Clock:** 12 MHz · **UART:** COM24 115200 8N1
**Standing rules:** host golden every generation where applicable; `PHYSICALLY_VERIFIED` only with bitstream SHA + UART capture; no claim of full craft FDIR RTL, statechain SHA on silicon, N=256 / M≥16, or production collision routing.

---

## 1. Executive verdict

All iCE Max Waves A–D, follow-on collision/fault, live UART packet I/M/A, and roadmap P7–P10 seals **PASS** on silicon or host as stated below. Production transport router remains **store-and-forward**; `COLLISION_ROUTING_ENABLED = false`.

| Layer | Status |
|-------|--------|
| GPL algebra / compose / reflect / topology on FPGA | PASS (sealed) |
| 1-D lattice scale N=32/64/128 | PASS (sealed) |
| 2-D M=4 glider matrix + soaks (10k / 62.5k gens) | PASS (sealed) |
| 2-D M=8 fabric smoke + serial glider | PASS (sealed) |
| Two-glider collisions S0–S3 | PASS (sealed classes) |
| Fault inject M1–M4 | PASS (sealed) |
| Live UART one-packet I/M/A | PASS (sealed) |
| Live UART I/M/A/P `PROP_ARM` + 5-round soak | PASS (sealed) |
| Craft adapter over COM24 (`fpga-uart`) | PASS (sealed) |
| Host collision policy module | PASS (unit); routing gated OFF |
| Host golden CI (`fpga_golden_tests` + `phase_transport_tests`) | listed PASS suite below |

---

## 2. Hardware / method

- **Flow:** Yosys → nextpnr-ice40 → icepack → iceprog (SPI flash) → UART capture → Python/host golden verify → write `PHYSICALLY_VERIFIED.txt` + `icebreaker.bin.sha256`.
- **Acceptance:** host golden match every gen (glider/lattice); `MISMATCH_COUNT=0`; expected UART tokens (e.g. `GLIDER PASS`, `PKT …`, `ALGEBRA PASS`).
- **Evidence root:** `fpga/evidence/` (61 `PHYSICALLY_VERIFIED.txt` seals).
- **Campaign rollups:** `fpga/evidence/ICE_MAX_CAMPAIGN_2026-07-24.md`, `fpga/evidence/ice_max_run_log.txt`, `docs/reports/NEXT_FPGA_PROGRAM.md`.

---

## 3. Run log (chronological campaign)

```
ICE MAX RUN 2026-07-24T19:57:49.9879697-06:00
PASS algebra64
PASS glider_V1D2
PASS glider_V2D1
PASS N128
PASS M8_smoke
PASS soak_62500
RESUME 2026-07-24T20:09:32.3433823-06:00
PASS collision_S0
PASS serial_M8
PASS fault_delete
ICE MAX COMPLETE
PASS collision_S1
PASS collision_S2
PASS collision_S3
PASS fault_M2
PASS fault_M3
PASS fault_M4
PASS live_packet_IMA
FOLLOWON COMPLETE
PASS packet_prop_arm rounds=1
PASS packet_prop_arm rounds=5
PASS craft_com24
PASS uart_soak rounds=5
```

---

## 4. Sealed silicon results (by family)

### 4.1 Primitive / algebra / vacuum / 1-D

| Gate folder | Result | Bitstream SHA-256 |
|-------------|--------|-------------------|
| `compose_16_16` | COMPOSE 16/16 | `b94e81830c15f928d1ae56841f138ead4755a75d8748cd6beea1012bd11fd7a6` |
| `reflect_topology` | COMPOSE+REFLECT+TOPOLOGY | `81232032801c5c630147dbc434b5d4382197e0bfa3ee5fb1e3c7628aeed35b0e` |
| `gpl_vm` / `complex_gates` | VM + complex suite | see seals |
| `gpl_algebra_64` | **64/64** triples MATCH; 0 mismatch; ~24.9 s | `730d6e546382667a35416c43f852665584579a0bffe2151dd4f84e7299ff40bb` |
| `phase_lattice` | mixed-field + CLOCK PASS | `9f6206a50754d0cc9f5b81e17ca664070c877c504bd4c478c85e4052fe89c0cd` |
| `phase_lattice_phi0` / `phi3` | VACUUM PASS | see seals |
| `N32_physical` | CLOCK PASS | `527a400843e9cbb218a1b0f1580e2be3dc9ef9189efbf1292a31816152361488` |
| `N64_physical` | CLOCK PASS | `06777ca8cf0c8013e9e20ceb6ce162bddfed72005bbc18ad4b722b2b660218bf` |
| `N128_physical` | CLOCK PASS | `76d454b599dbc81018d5438c97cb5dcc911f2efbf701a6156902292d7e987b8d` |

### 4.2 2-D fabric + M=4 glider campaign

| Gate | Result | Notes / SHA |
|------|--------|-------------|
| `phase_lattice_2d_M4` | fabric alive (not glider claim) | `68dd43ed…ac020f9` |
| `phase_lattice_2d_M8` | fabric smoke 8×8 | `83ec0933…6a7f3b` |
| M=4 G8 V1D2 all starts S00–S33 | **16/16 PASS**, F=01, 0 mismatch | per-seed seals |
| M=4 G8 V2D1 all starts S00–S33 | **16/16 PASS**, F=01, 0 mismatch | per-seed seals |
| `…_G10000_V1D2_S22` | PASS · 160000 cell updates · 76.61 s | `a4dc86c0…8efd52c` |
| `…_G62500_V1D2_S22` | PASS · **10⁶** cell updates · 364.31 s | `ff75b487…7956927` |
| `phase_lattice_2d_glider_serial_M8_G16_V1D2_S44` | serial M=8 glider PASS · F=01 · Fmax~25 MHz | `a32c67eb…73c30b` |

### 4.3 Collisions (M=4, GENS=16)

| Scenario | Name (host policy) | Class | SHA-256 |
|----------|--------------------|-------|---------|
| S0 | head_on | **PASS_THROUGH** | `814774cc67ec74701d0b0fb61e15c4a0b679778663c55db866c94c72ae397343` |
| S1 | phase_offset | **PASS_THROUGH** | `35146c93f7f59c0a11c6803db76d60062a4b84232cf63870de9bd7c31f767251` |
| S2 | vs_reflect | **DOMAIN_GROWTH** | `767a336402cf6a301bca7e5f9305fdaaa912a7c7aacd1ca3a19d2a8d9d1ebf4c` |
| S3 | domain_boundary | **DOMAIN_GROWTH** | `5ae1408464299d8a7f6a6d0479e23caf8acafdad64d7578604058117629f9365` |

Host module: `src/kernel/phase_transport/collision_policy.rs` — `COLLISION_ROUTING_ENABLED = false` (router stays store-and-forward).

### 4.4 Fault injection (M=4, GENS=8)

| Mode | Injection | Gate | SHA-256 |
|------|-----------|------|---------|
| M1 | delete @ gen2 | `phase_lattice_2d_fault_M1_G8` | `fed94d87272127b71eae5e84580c8e9c93a92bbd72af63d564e17911c5bd7e1c` |
| M2 | duplicate | `phase_lattice_2d_fault_M2_G8` | `5afce41c0e7d9e115c5f3cfbbdb4df243761a724f7513d2223274c0bf8097a34` |
| M3 | Φ2→Φ3 flip | `phase_lattice_2d_fault_M3_G8` | `f11322066fe462bce08cc3e4fffcd283bb43f157f4bb3805b257511b93715a19` |
| M4 | skip update | `phase_lattice_2d_fault_M4_G8` | `bba5844eaf73b874eb01fd36cd9185187d9b952232b8e2bc915db5af2707eaea` |

All four: UART MATCH host detect check · PHYSICALLY_VERIFIED.

### 4.5 Live UART packet / Avenue 10B on-chip (P7–P10)

| Phase | Gate / artifact | Result | SHA / note |
|-------|-----------------|--------|------------|
| Live I/M/A (pre-PROP) | `phase_lattice_2d_packet_G2` | inject→move→recv→ACK MATCH | `0f7009cb0bebf62cde5ccf0024cf0e65833f5eeb7eecd932c37faf69a811bbd0` |
| **P7** PROP_ARM | `phase_lattice_2d_packet_prop_arm_G2` | I/M/A/P · rounds 1/1 | `ae7041aba901e1030dd4b1d53268194e9d9ab5a9904515b20eb35ab727307974` |
| **P10** soak | `phase_lattice_2d_packet_prop_arm_G2_soak5` | I/M/A/P · **5/5** rounds | `f82f363292e9921e78883eb5fefa1b621496d27d8889803fdcea0f63be7b233c` |
| **P8** craft COM24 | `fpga_uart_craft_com24` | host twin + live UART; feature `fpga-uart` | packet SHA `f82f3632…7b233c` |
| **P9** collision policy | host-only | sealed S0–S3 table; routing gated | `collision_policy.rs` |

RTL: `fpga/phase_lattice_2d_packet.v` (soft re-arm ~2s after DONE). Scripts: `flash-capture-phase-lattice-2d-packet.ps1`, `run-craft-com24.ps1`, `run-packet-uart-soak.ps1`.

---

## 5. Host / CI tests (not silicon)

### 5.1 `tests/fpga_golden_tests.rs` (7)

- `fpga_compose_table_matches_core`
- `fpga_reflect_matches_core`
- `fpga_thresholds_match_core`
- `fpga_topology_matches_core`
- `fpga_algebra_64_uart_format_matches_host`
- `fpga_glider_uart_format_matches_host`
- `vm_trace_opcode_layout_matches_decoder`

### 5.2 `tests/phase_transport_tests.rs` (20)

- Transport / router / checksum / TTL / lanes / statechain / FDIR detection helpers
- `one_packet_smoke_over_sealed_glider_law`
- `one_packet_prop_arm_host_twin`
- `collision_policy_sealed_and_gated`
- `verified_m4_glider_golden_eight_gens`
- Live UART opt-in via `KAIROS_FPGA_LIVE=1` + feature `fpga-uart` (not required for default CI)

---

## 6. Key commits

| Commit | Summary |
|--------|---------|
| `4b08afc` | glider campaign source + golden tests |
| `d0eb95b` | seal iCE Max Waves A–D evidence |
| `42942f4` | seal collision S1–S3, fault M2–M4, live I/M/A |
| `3f2d371` | seal P7–P10 PROP_ARM / craft UART / collision policy / soak5 |

---

## 7. Explicit non-claims (do not assert)

1. Full craft **FDIR RTL** running on the FPGA.
2. **Statechain hashing** computed on silicon.
3. Lattice sizes **N=256** or **M≥16** on this UP5K.
4. **Production collision routing** in `TransportRouter` (gate is false).
5. That M=8 smoke fabric alone proves a glider (glider claim is serial M=8 seal only).

---

## 8. Appendix — all PHYSICALLY_VERIFIED seals (machine dump)

### `complex_gates`

```
PHYSICALLY_VERIFIED=YES
GATE=compose+reflect+topology+t_sign+t_energy+parallel4+gpl_vm_complex
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
BITSTREAM_SHA256=f499032615d2284b90fccfcf98207fae4f2dd2617868eba68549cc3102ae6dfc
RESULT=COMPOSE 16/16; REFLECT 4/4; TOPOLOGY 64/64; T_SIGN 4/4; T_ENERGY 4/4; PARALLEL 4/4; GPL VM PASS
```

### `compose_16_16`

```
PHYSICALLY_VERIFIED=YES
GATE=compose_16_16
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CAPTURED=2026-07-13
BITSTREAM_SHA256=b94e81830c15f928d1ae56841f138ead4755a75d8748cd6beea1012bd11fd7a6
SOURCE_COMMIT=ad89c3a48244bab2e918fdea2f98080cad1a9e9c
RESULT=COMPOSE 16/16 PASS
NOTE=Printed compose digits are driven by phase_alu compose_out; expected table is compare-only.
POWER_CYCLE=bitstream programmed to SPI flash; re-runs after POR / soft reset without iceprog.
```

### `fpga_uart_craft_com24`

```
PHYSICALLY_VERIFIED=YES
GATE=fpga_uart_craft_com24
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
ADAPTER=FpgaGliderTransport host twin + live I/M/A/P
FEATURE=fpga-uart
PACKET_BITSTREAM_SHA256=f82f363292e9921e78883eb5fefa1b621496d27d8889803fdcea0f63be7b233c
RESULT=craft-over-COM24 PROP_ARM PASS; LocalCraftTransport remains fallback
```

### `gpl_algebra_64`

```
PHYSICALLY_VERIFIED=YES
GATE=gpl_algebra_64
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=730d6e546382667a35416c43f852665584579a0bffe2151dd4f84e7299ff40bb
TEST_DURATION_S=24.91
MISMATCH_COUNT=0
RESULT=64/64 triples ab,L,R,p MATCH host; ALGEBRA PASS
```

### `gpl_vm`

```
PHYSICALLY_VERIFIED=YES
GATE=compose+reflect+topology+gpl_vm
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
BITSTREAM_SHA256=c08e123b1056eae212c1c7b3deb3369baecab69557d0b83c89931d2224fc10b6
RESULT=COMPOSE 16/16 PASS; REFLECT 4/4 PASS; TOPOLOGY 64/64 PASS; GPL VM PASS
```

### `phase_lattice`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_causal_curvature_n16
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
BITSTREAM_SHA256=9f6206a50754d0cc9f5b81e17ca664070c877c504bd4c478c85e4052fe89c0cd
RESULT=40 generations mixed-field defect readout + Phi1 clock alternation CLOCK PASS; capture matches verify_lattice.py reference exactly
```

### `phase_lattice_2d_collision_S0_G16`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_collision_S0_G16
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=814774cc67ec74701d0b0fb61e15c4a0b679778663c55db866c94c72ae397343
SCENARIO=0
CLASS=PASS_THROUGH
MISMATCH_COUNT=0
RESULT=collision host MATCH; class=PASS_THROUGH
```

### `phase_lattice_2d_collision_S1_G16`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_collision_S1_G16
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=35146c93f7f59c0a11c6803db76d60062a4b84232cf63870de9bd7c31f767251
SCENARIO=1
CLASS=PASS_THROUGH
MISMATCH_COUNT=0
RESULT=collision host MATCH; class=PASS_THROUGH
```

### `phase_lattice_2d_collision_S2_G16`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_collision_S2_G16
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=767a336402cf6a301bca7e5f9305fdaaa912a7c7aacd1ca3a19d2a8d9d1ebf4c
SCENARIO=2
CLASS=DOMAIN_GROWTH
MISMATCH_COUNT=0
RESULT=collision host MATCH; class=DOMAIN_GROWTH
```

### `phase_lattice_2d_collision_S3_G16`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_collision_S3_G16
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=5ae1408464299d8a7f6a6d0479e23caf8acafdad64d7578604058117629f9365
SCENARIO=3
CLASS=DOMAIN_GROWTH
MISMATCH_COUNT=0
RESULT=collision host MATCH; class=DOMAIN_GROWTH
```

### `phase_lattice_2d_fault_M1_G8`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_fault_M1_G8
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
BITSTREAM_SHA256=fed94d87272127b71eae5e84580c8e9c93a92bbd72af63d564e17911c5bd7e1c
FAULT_MODE=1
RESULT=fault inject UART MATCH host detect check
```

### `phase_lattice_2d_fault_M2_G8`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_fault_M2_G8
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
BITSTREAM_SHA256=5afce41c0e7d9e115c5f3cfbbdb4df243761a724f7513d2223274c0bf8097a34
FAULT_MODE=2
RESULT=fault inject UART MATCH host detect check
```

### `phase_lattice_2d_fault_M3_G8`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_fault_M3_G8
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
BITSTREAM_SHA256=f11322066fe462bce08cc3e4fffcd283bb43f157f4bb3805b257511b93715a19
FAULT_MODE=3
RESULT=fault inject UART MATCH host detect check
```

### `phase_lattice_2d_fault_M4_G8`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_fault_M4_G8
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
BITSTREAM_SHA256=bba5844eaf73b874eb01fd36cd9185187d9b952232b8e2bc915db5af2707eaea
FAULT_MODE=4
RESULT=fault inject UART MATCH host detect check
```

### `phase_lattice_2d_glider_M4`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
BITSTREAM_SHA256=8c97780d67102159df9c5e367ccfc0bdff863bdd822fe72bd709d40797ece81f
RESULT=Avenue2 rule_a 1-cell glider MATCH host golden; F=01 for 8 gens; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G10000_V1D2_S22`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G10000_V1D2_S22
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=a4dc86c0c1cf2864b88a3057a1c0d523401c797d148d68f622220c8978efd52c
M=4
GENS=10000
VACUUM=1
DEFECT=2
SEED_R=2
SEED_C=2
CELL_UPDATES=160000
TEST_DURATION_S=76.61
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G62500_V1D2_S22`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G62500_V1D2_S22
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=ff75b4875194e5a464139c8c94f3f7389d62e3f775380b5465a10d8097956927
M=4
GENS=62500
VACUUM=1
DEFECT=2
SEED_R=2
SEED_C=2
CELL_UPDATES=1000000
TEST_DURATION_S=364.31
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V1D2_S00`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V1D2_S00
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=11c5f96ff71fb885f353c0000ea8dbe8d1acb02142469ec8f1f45944c3459c45
M=4
GENS=8
VACUUM=1
DEFECT=2
SEED_R=0
SEED_C=0
CELL_UPDATES=128
TEST_DURATION_S=24.59
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V1D2_S01`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V1D2_S01
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=21f3d6cf8fe057cba02538d9b2c377c03daac54975e40eac8d8833fd35f45b27
M=4
GENS=8
VACUUM=1
DEFECT=2
SEED_R=0
SEED_C=1
CELL_UPDATES=128
TEST_DURATION_S=24.5
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V1D2_S02`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V1D2_S02
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=b61aa6bf685924b81ca41ab71d06a56cf63c1567d7a6dbaf9fddd417c5501e88
M=4
GENS=8
VACUUM=1
DEFECT=2
SEED_R=0
SEED_C=2
CELL_UPDATES=128
TEST_DURATION_S=24.52
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V1D2_S03`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V1D2_S03
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=2d405664a350120fdc43782b4ba3fa92bdf6fb424ae99e0fb107dfed607d33cd
M=4
GENS=8
VACUUM=1
DEFECT=2
SEED_R=0
SEED_C=3
CELL_UPDATES=128
TEST_DURATION_S=24.54
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V1D2_S10`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V1D2_S10
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=0a699e70ccde536e7d4c4a26ae61dc712e179efd571f7e1f108c78f95c48d299
M=4
GENS=8
VACUUM=1
DEFECT=2
SEED_R=1
SEED_C=0
CELL_UPDATES=128
TEST_DURATION_S=24.52
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V1D2_S11`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V1D2_S11
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=4ca69e86f52b175ae66090bc4e9353751d9181b1bd4b9b89dfb01ea19ddc41ff
M=4
GENS=8
VACUUM=1
DEFECT=2
SEED_R=1
SEED_C=1
CELL_UPDATES=128
TEST_DURATION_S=24.63
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V1D2_S12`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V1D2_S12
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=b4011eaf5cafc3fd3eb20584aafe2a7dbc30c56d5cf02e83e260d6b18865a2c5
M=4
GENS=8
VACUUM=1
DEFECT=2
SEED_R=1
SEED_C=2
CELL_UPDATES=128
TEST_DURATION_S=24.59
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V1D2_S13`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V1D2_S13
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=966baeb6d39ba0f3f8b2dd8f3b43ab6d29b8321fc16631cf2de1a0f9679bc359
M=4
GENS=8
VACUUM=1
DEFECT=2
SEED_R=1
SEED_C=3
CELL_UPDATES=128
TEST_DURATION_S=24.55
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V1D2_S20`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V1D2_S20
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=ccb188080d12997fc5c5e06bd6a40d0a276ed0463b62446960afd8b89d009f07
M=4
GENS=8
VACUUM=1
DEFECT=2
SEED_R=2
SEED_C=0
CELL_UPDATES=128
TEST_DURATION_S=24.54
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V1D2_S21`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V1D2_S21
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=ea2e39990ae9ada765f76295c2a70ba350afc24cf1af13e2d2d88539288958d0
M=4
GENS=8
VACUUM=1
DEFECT=2
SEED_R=2
SEED_C=1
CELL_UPDATES=128
TEST_DURATION_S=24.5
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V1D2_S22`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V1D2_S22
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=bfdd97923aa8289f81910b03dc3917e6a3ad7d23f6927b1af8919429dfeda534
M=4
GENS=8
VACUUM=1
DEFECT=2
SEED_R=2
SEED_C=2
CELL_UPDATES=128
TEST_DURATION_S=24.94
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V1D2_S23`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V1D2_S23
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=de44157df062bf0f57c1bc656c1f29bfc19a45df44e25df7c0e08c6ad974a176
M=4
GENS=8
VACUUM=1
DEFECT=2
SEED_R=2
SEED_C=3
CELL_UPDATES=128
TEST_DURATION_S=24.45
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V1D2_S30`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V1D2_S30
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=6374098e669a6d67b2ad5b1e00ea587ae7bdbbf6da9bf3f243bd2c97b0180eab
M=4
GENS=8
VACUUM=1
DEFECT=2
SEED_R=3
SEED_C=0
CELL_UPDATES=128
TEST_DURATION_S=24.51
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V1D2_S31`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V1D2_S31
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=b9980c837188cc271b71dd37c02b5e1a3a1b14d32c0c5e06ad1a5063be507e2e
M=4
GENS=8
VACUUM=1
DEFECT=2
SEED_R=3
SEED_C=1
CELL_UPDATES=128
TEST_DURATION_S=24.57
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V1D2_S32`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V1D2_S32
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=e0aea38250d296340d5b262c04cfd47cc46a9d15a8c8b26bb388712dfc56391d
M=4
GENS=8
VACUUM=1
DEFECT=2
SEED_R=3
SEED_C=2
CELL_UPDATES=128
TEST_DURATION_S=24.51
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V1D2_S33`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V1D2_S33
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=9934fb97f9beb75b1015644c6ab6c5bc2eaf8dd8751eaf78b3df05fa6e362dff
M=4
GENS=8
VACUUM=1
DEFECT=2
SEED_R=3
SEED_C=3
CELL_UPDATES=128
TEST_DURATION_S=24.47
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V2D1_S00`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V2D1_S00
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=104f34dd29b5eb539c14bf49bd8bb6385b5b5a8d75660d5a4daef5ccd55ed671
M=4
GENS=8
VACUUM=2
DEFECT=1
SEED_R=0
SEED_C=0
CELL_UPDATES=128
TEST_DURATION_S=24.49
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V2D1_S01`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V2D1_S01
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=ba2fbf47b3920308434e4fda095020e3627f2008310b86a97703f4c8dc57df58
M=4
GENS=8
VACUUM=2
DEFECT=1
SEED_R=0
SEED_C=1
CELL_UPDATES=128
TEST_DURATION_S=24.39
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V2D1_S02`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V2D1_S02
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=72e92173237730e6bc21732b4c87a1a64372445f75bcb7d8f17f5b27f56c02ec
M=4
GENS=8
VACUUM=2
DEFECT=1
SEED_R=0
SEED_C=2
CELL_UPDATES=128
TEST_DURATION_S=24.81
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V2D1_S03`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V2D1_S03
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=546c14952fe5751572216c82d51ed259df7f62d87b25d59cbd53a5e40b7a54ba
M=4
GENS=8
VACUUM=2
DEFECT=1
SEED_R=0
SEED_C=3
CELL_UPDATES=128
TEST_DURATION_S=24.59
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V2D1_S10`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V2D1_S10
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=9853d7e084ce6e382ec6eff8f41f83fcdef2327f31f99522c4d0b7aa63e33362
M=4
GENS=8
VACUUM=2
DEFECT=1
SEED_R=1
SEED_C=0
CELL_UPDATES=128
TEST_DURATION_S=24.77
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V2D1_S11`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V2D1_S11
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=3467e579653cad9d31878bafb8c6f11f82834afad4e1768e196fcafedcbeb628
M=4
GENS=8
VACUUM=2
DEFECT=1
SEED_R=1
SEED_C=1
CELL_UPDATES=128
TEST_DURATION_S=24.57
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V2D1_S12`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V2D1_S12
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=eb50709491c4db251ed925edb1f376e2191dc854f6249c76c1148ae92f0c42eb
M=4
GENS=8
VACUUM=2
DEFECT=1
SEED_R=1
SEED_C=2
CELL_UPDATES=128
TEST_DURATION_S=24.78
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V2D1_S13`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V2D1_S13
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=39d4246d75f7f4f17d60b63cc4a43bbb38301d1c185d921b76a6a3a97bd4a8cc
M=4
GENS=8
VACUUM=2
DEFECT=1
SEED_R=1
SEED_C=3
CELL_UPDATES=128
TEST_DURATION_S=24.54
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V2D1_S20`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V2D1_S20
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=2b4243e31af65507a101d0343821e1200e77cf1f4a94d8ae40ca643e2cc990f4
M=4
GENS=8
VACUUM=2
DEFECT=1
SEED_R=2
SEED_C=0
CELL_UPDATES=128
TEST_DURATION_S=24.7
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V2D1_S21`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V2D1_S21
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=d6a2ac922e104cce7067574a88b302e892ca13d197d2c613de0b500417e2c4cf
M=4
GENS=8
VACUUM=2
DEFECT=1
SEED_R=2
SEED_C=1
CELL_UPDATES=128
TEST_DURATION_S=24.48
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V2D1_S22`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V2D1_S22
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=259b7c861d1cd3e1ba50b87f6d9dfcd10581fa1cc30fe538f709bba68ee842a6
M=4
GENS=8
VACUUM=2
DEFECT=1
SEED_R=2
SEED_C=2
CELL_UPDATES=128
TEST_DURATION_S=24.62
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V2D1_S23`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V2D1_S23
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=1df4fbde90eca3e3655db126c9e8c1c8955663eb3f9f8589f4788882d67f8b7d
M=4
GENS=8
VACUUM=2
DEFECT=1
SEED_R=2
SEED_C=3
CELL_UPDATES=128
TEST_DURATION_S=24.75
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V2D1_S30`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V2D1_S30
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=e0a5aa4ac5344cb0b3bdb62c7ef61b61e1dce89ef4ca8e0d91a81748821eb33d
M=4
GENS=8
VACUUM=2
DEFECT=1
SEED_R=3
SEED_C=0
CELL_UPDATES=128
TEST_DURATION_S=24.49
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V2D1_S31`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V2D1_S31
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=aa4dce79677bda2cd8c627c2eebf9e885f37cc4221a2b4543ab5a27beb0d35e3
M=4
GENS=8
VACUUM=2
DEFECT=1
SEED_R=3
SEED_C=1
CELL_UPDATES=128
TEST_DURATION_S=24.54
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V2D1_S32`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V2D1_S32
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=6cb72be029d278c6a549f7f6b06ebed2118f1adf771d0f312eccc877df8b8f5e
M=4
GENS=8
VACUUM=2
DEFECT=1
SEED_R=3
SEED_C=2
CELL_UPDATES=128
TEST_DURATION_S=24.55
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_M4_G8_V2D1_S33`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_M4_G8_V2D1_S33
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=c7cb8d6609dba192a3d09d1c1e477c7b0661357e7ef34264ccfae4d574343eb5
M=4
GENS=8
VACUUM=2
DEFECT=1
SEED_R=3
SEED_C=3
CELL_UPDATES=128
TEST_DURATION_S=24.81
MISMATCH_COUNT=0
RESULT=Avenue2 glider MATCH host every gen; F=01; GLIDER PASS
```

### `phase_lattice_2d_glider_serial_M8_G16_V1D2_S44`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_glider_serial_M8_G16_V1D2_S44
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=a32c67ebfe9a6dbccb5c467bde623f85ae198615b11646700873fc089673c30b
M=8
GENS=16
ARCH=cell_serial
MISMATCH_COUNT=0
RESULT=serial glider MATCH host; F=01
```

### `phase_lattice_2d_M4`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_M4
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
BITSTREAM_SHA256=68dd43ed77d72bec6ac191f3dbeae2dc9da2a0ad13d8320537c146c27ac020f9
RESULT=2-D 04x04 fabric alive on silicon (4 gen lines); not a glider claim
```

### `phase_lattice_2d_M8`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_M8
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
BITSTREAM_SHA256=83ec093399b74ea14d02f1d0c6fbdf961e72ddcff10a3e6c8d8905d2de6a7f3b
RESULT=2-D 08x08 fabric alive on silicon (4 gen lines); not a glider claim
```

### `phase_lattice_2d_packet_G2`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_packet_G2
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=0f7009cb0bebf62cde5ccf0024cf0e65833f5eeb7eecd932c37faf69a811bbd0
PROTOCOL=I/M/A live UART
GENS=2
MISMATCH_COUNT=0
RESULT=live one-packet inject-move-recv-ACK MATCH host; before PROP_ARM
```

### `phase_lattice_2d_packet_prop_arm_G2`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_packet_prop_arm_G2
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=ae7041aba901e1030dd4b1d53268194e9d9ab5a9904515b20eb35ab727307974
PROTOCOL=I/M/A/P live UART PROP_ARM
GENS=2
ROUNDS=1
ROUNDS_PASS=1
MISMATCH_COUNT=0
RESULT=live inject-move-ACK-PROP_ARM MATCH host
```

### `phase_lattice_2d_packet_prop_arm_G2_soak5`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_2d_packet_prop_arm_G2_soak5
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
CLOCK_HZ=12000000
BITSTREAM_SHA256=f82f363292e9921e78883eb5fefa1b621496d27d8889803fdcea0f63be7b233c
PROTOCOL=I/M/A/P live UART PROP_ARM
GENS=2
ROUNDS=5
ROUNDS_PASS=5
MISMATCH_COUNT=0
RESULT=live inject-move-ACK-PROP_ARM MATCH host
```

### `phase_lattice_phi0`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_vacuum_phi0
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
BITSTREAM_SHA256=f38168986ae32760b1f321ce5d907ace3e2c340aad8b9288df2b7e9bf52b8353
RESULT=VACUUM PASS; homogeneous Phi0 fixed + D=00 for all gens
```

### `phase_lattice_phi3`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_vacuum_phi3
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
BITSTREAM_SHA256=f0341b24c661f5c0e0d6a8451ef216c0f020891e9fe4795f243551e26dd72911
RESULT=VACUUM PASS; homogeneous Phi3 fixed + D=00 for all gens
```

### `phase_lattice_scale/N128_physical`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_scale_n128
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
BITSTREAM_SHA256=76d454b599dbc81018d5438c97cb5dcc911f2efbf701a6156902292d7e987b8d
RESULT=N=128 physical UART matches verify_lattice_scale.py; CLOCK PASS
```

### `phase_lattice_scale/N32_physical`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_scale_n32
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
BITSTREAM_SHA256=527a400843e9cbb218a1b0f1580e2be3dc9ef9189efbf1292a31816152361488
RESULT=N=32 physical UART matches verify_lattice_scale.py; CLOCK PASS
```

### `phase_lattice_scale/N64_physical`

```
PHYSICALLY_VERIFIED=YES
GATE=phase_lattice_scale_n64
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
BITSTREAM_SHA256=06777ca8cf0c8013e9e20ceb6ce162bddfed72005bbc18ad4b722b2b660218bf
RESULT=N=64 physical UART matches verify_lattice_scale.py; CLOCK PASS
```

### `reflect_topology`

```
PHYSICALLY_VERIFIED=YES
GATE=compose+reflect+topology
BOARD=iCEBreaker V1.1a
UART=COM24 115200 8N1
BITSTREAM_SHA256=81232032801c5c630147dbc434b5d4382197e0bfa3ee5fb1e3c7628aeed35b0e
RESULT=COMPOSE 16/16 PASS; REFLECT 4/4 PASS; TOPOLOGY 64/64 PASS
```

