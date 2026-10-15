# iCE Max Silicon Campaign — 2026-07-24

**Board:** iCEBreaker V1.1a · **UART:** COM24 115200 8N1 · **Clock:** 12 MHz  
**Standing rule:** host golden every gen; honest seals only; no craft-FDIR / statechain SHA claim on silicon.

Run log: `fpga/evidence/ice_max_run_log.txt`

## Wave A — Regress / endurance / fabric

| Gate | Result | SHA-256 (prefix) |
|------|--------|------------------|
| compose / algebra / glider matrix (`run-ice-regression.ps1`) | PASS | see gate folders |
| M=4 GENS=62500 soak (~10⁶ cell updates) | **PHYSICALLY_VERIFIED** | `ff75b487…7956927` |
| 1-D N=128 physical | **PHYSICALLY_VERIFIED** | `76d454b5…987b8d` |
| 2-D M=8 smoke (not glider) | **PHYSICALLY_VERIFIED** | `83ec0933…6a7f3b` |
| Host golden CI (`fpga_golden_tests`) | PASS | — |

## Wave B — Collision

| Gate | Class | Result | SHA-256 |
|------|-------|--------|---------|
| `phase_lattice_2d_collision_S0_G16` | **PASS_THROUGH** | host MATCH, 0 mismatches | `814774cc…397343` |

Production router remains store-and-forward until further collision classes are sealed.

## Wave C — M=8 glider

| Gate | Fmax | Result | SHA-256 |
|------|------|--------|---------|
| `phase_lattice_2d_glider_serial_M8_G16_V1D2_S44` | ~24.9 MHz PASS @ 12 MHz | host MATCH, GLIDER PASS, F=01 | `a32c67eb…73c30b` |

Architecture: row/col-serial + registered serial footprint (no combo popcount). GPL math unchanged.

## Wave D — Fault + one-packet transport

| Gate / test | Result | Notes |
|-------------|--------|-------|
| `phase_lattice_2d_fault_M1_G8` (delete@gen2) | **PHYSICALLY_VERIFIED** | DETECT latency=03 · SHA `fed94d87…bd7e1c` |
| `phase_lattice_2d_fault_M2/M3/M4_G8` | **PHYSICALLY_VERIFIED** | duplicate / Φ2→Φ3 flip / skip update |
| Host `FpgaGliderTransport::one_packet_smoke` | PASS | sealed-law adapter |
| Live UART I/M/A `phase_lattice_2d_packet_G2` | **PHYSICALLY_VERIFIED** | inject→move→RECV→ACK on COM24 · SHA `0f7009cb…11bbd0` |

## Follow-on seals (same evening)

| Gate | Class / result |
|------|----------------|
| Collision S1 | PASS_THROUGH |
| Collision S2 | DOMAIN_GROWTH |
| Collision S3 | DOMAIN_GROWTH |

**Explicit non-claims:** full craft FDIR RTL on FPGA; statechain hashing on silicon; N=256 / M≥16; collision routing in production transport (gated).

## Roadmap P7–P10 (Avenue 10B on-chip)

| Phase | Result |
|-------|--------|
| P7 live `PROP_ARM` (I/M/A/P) | **PHYSICALLY_VERIFIED** `phase_lattice_2d_packet_prop_arm_G2` |
| P8 craft / COM24 adapter (`fpga-uart` + host twin) | **PHYSICALLY_VERIFIED** `fpga_uart_craft_com24` |
| P9 collision policy S0–S3 | host module; `COLLISION_ROUTING_ENABLED=false` |
| P10 UART soak 5× I/M/A/P | **PHYSICALLY_VERIFIED** `phase_lattice_2d_packet_prop_arm_G2_soak5` (5/5) |

## Evidence folders

- `phase_lattice_2d_glider_M4_G62500_V1D2_S22/`
- `phase_lattice_scale/N128_physical/`
- `phase_lattice_2d_M8/`
- `phase_lattice_2d_collision_S0_G16/`
- `phase_lattice_2d_glider_serial_M8_G16_V1D2_S44/`
- `phase_lattice_2d_fault_M1_G8/`
- `phase_lattice_2d_packet_prop_arm_G2/`
- `phase_lattice_2d_packet_prop_arm_G2_soak5/`
- `fpga_uart_craft_com24/`
