# G64 Research Log

## 2026-07-30 — End of day

- Fresh day report (does not overwrite 2026-07-24): [`docs/reports/DAY_REPORT_2026-07-30.md`](../reports/DAY_REPORT_2026-07-30.md)
- Default BOOT restored on production SD; restore script + package committed

## 2026-07-30 — Findings + mainline integration + default BOOT restore

- Report: [`G64_FINDINGS_AND_MAINLINE_INTEGRATION.md`](G64_FINDINGS_AND_MAINLINE_INTEGRATION.md)
- Default BOOT staged: `fpga/zynq/research/g64_phase_fabric_sidecar/RESTORE_DEFAULT_BOOT/BOOT.BIN` (feature-test SHA `e17a3ac5…`)
- Restore script: `scripts/restore_default_pz7020_boot.ps1` (no harness). SD `H:` had no media when attempted — insert card and re-run.

## 2026-07-30 — Physical COM12 correction

- Evidence `run_20260730_175025` shows G64 on COM12 **PASS** (compose 16/16, reflect 4/4, atlas 52/12/0 hw==gphase_core, benchmark 8/8)
- Report: [`G64_PHYSICAL_COM12_REPORT_20260730.md`](G64_PHYSICAL_COM12_REPORT_20260730.md)
- Prior “NOT TESTED” wording in docs was incorrect and has been corrected

## 2026-07-30 — Oracle host test report

- Ran / recorded `gpl_oracle --g64-research-out …` → **16/16 compose, 4/4 reflect, 64/64 atlas, census 52/12/0**
- Full triple table + digests: [`docs/research/G64_ORACLE_TEST_REPORT_20260730.md`](G64_ORACLE_TEST_REPORT_20260730.md)
- Classification: SIMULATION VERIFIED (host). G64 physical COM12 still NOT TESTED.

## 2026-07-30 — Kickoff

- Branch: `research/g64-phase-fabric-sidecar` from `0e89807` (feature/pz7020-completion tip)
- Preservation tag: `pz7020-console-proof-20260730` (`7b7d754`)
- Created `fpga/zynq/research/g64_phase_fabric_sidecar/` with baseline inventory, DO_NOT_TOUCH, RTL candidates C1–C8, AXI-Lite sidecar draft, oracle export path, `g64-research` Cargo feature + shell commands
- Status: scaffolding **SIMULATION/SYNTHESIS pending**; physical **NOT TESTED**
- Next: generate oracle artifacts; run host tests; Vivado OOC sweep; BD integration; second-SD physical gate
