# G64 Physical COM12 Test Report — 2026-07-30

## Correction

Earlier docs incorrectly labeled G64 on COM12 as **NOT TESTED**. That was wrong.

Physical evidence exists and **PASS**ed:

[`fpga/zynq/evidence/kairos_zynq_boot/run_20260730_175025/`](../fpga/zynq/evidence/kairos_zynq_boot/run_20260730_175025/)

## Verdict

**PHYSICALLY VERIFIED** on PZ7020 COM12 (115200 8-N-1).

| Check | Hardware result |
|-------|-----------------|
| Boot → `gpos>` | PASS |
| `g1p info` (unchanged @ `0x40000000`) | ID `0x47504C34` VERSION `0x00020000` |
| `g64 info` @ `0x40001000` | ID `0x47363453` (`G64S`) VERSION `0x00010000` |
| `g64 oracle` (software census) | identity=52 reflection=12 error=0 |
| `g64 compose` | **PASS 16/16** |
| `g64 reflect` | **PASS 4/4** |
| `g64 atlas` (HW vs `gphase_core`) | **PASS identity=52 reflection=12 error=0** |
| `g64 benchmark` | **8/8** atlas iterations |
| `state` / shell live | PASS |

The **64-triple atlas** on hardware matched the canonical software census (**52 / 12 / 0**). Full per-triple table remains in [`G64_ORACLE_TEST_REPORT_20260730.md`](G64_ORACLE_TEST_REPORT_20260730.md) (host oracle authority used for the HW compare).

## Capture metadata

| Item | Value |
|------|--------|
| Run ID | `run_20260730_175025` |
| Port | COM12 @ 115200 |
| Plan | `scripts/pz7020_test_plan_g64.txt` |
| Started | 2026-07-30T17:50:25-06:00 |
| Ended | 2026-07-30T17:50:32-06:00 |
| Transcript SHA-256 | `10db227cabbcf6dbd5730163dfc4efe9180c96a31c4112a9ec9bb51dd2e074d0` |
| Harness report | `report.md` / `results.json` / `transcript.txt` |

## What this proves / does not prove

| Claim | Status |
|-------|--------|
| G64 sidecar MMIO responds with expected ID/version beside G1P | PHYSICALLY VERIFIED |
| Compose 16, reflect 4, atlas 64 classifications match `gphase_core` on board | PHYSICALLY VERIFIED |
| Shell remained responsive after G64 ops | PHYSICALLY VERIFIED |
| Every individual triple left/right printed on UART | Not required — atlas path reports mask census vs software |
| Max-fit 64-lane spatial synthesis sweep | Separate (synthesis), not this COM12 gate |
| HP/ACP / G64 IRQ delivery | NOT TESTED in this run |

Original console-proof capture `com12_capture_20260730_132633.txt` was **not** overwritten.
