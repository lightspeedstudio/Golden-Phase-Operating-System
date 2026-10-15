# G64 Executive Summary (living)

**Status:** conservative G64 sidecar is **PHYSICALLY VERIFIED** on COM12 (`run_20260730_175025`).

## Intent

Map Golden Phase Φ0..Φ3 / 64-triple atlas into PZ7020 PL as a **non-invasive sidecar** beside frozen G1P, using `gphase_core` as the sole mathematical authority.

## Current artifacts

- Research tree: `fpga/zynq/research/g64_phase_fabric_sidecar/`
- Oracle generator: `gpl_oracle --g64-research-out …`
- Host oracle (full 64 triples): [`G64_ORACLE_TEST_REPORT_20260730.md`](G64_ORACLE_TEST_REPORT_20260730.md) — **52 / 12 / 0**
- **Physical COM12 seal:** [`G64_PHYSICAL_COM12_REPORT_20260730.md`](G64_PHYSICAL_COM12_REPORT_20260730.md) — evidence `run_20260730_175025`
- SW opt-in: Cargo feature `g64-research` → `g64` shell commands
- MMIO: G64 @ `0x40001000` beside G1P @ `0x40000000`

## Classifications so far

| Item | Class |
|------|--------|
| G1P + console OS baseline | PHYSICALLY VERIFIED (prior evidence; do not overwrite) |
| Host `gphase_core` 64-atlas oracle | SIMULATION VERIFIED |
| G64 compose/reflect/atlas on COM12 | **PHYSICALLY VERIFIED** (`run_20260730_175025`) |
| Architecture sweep max-fit / spatial 64-lane | RESEARCH / synthesis (separate from this COM12 gate) |
| G64 IRQ / HP / ACP | NOT TESTED |
