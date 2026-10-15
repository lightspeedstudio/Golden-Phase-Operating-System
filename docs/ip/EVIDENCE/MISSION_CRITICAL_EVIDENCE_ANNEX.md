# Mission-Critical Evidence Annex — Golden Phase OS (Kairos / GPOS)

**Docket:** CAREGAGA-GPOS-2026-001  
**Inventor:** Richard Alberto Careaga  
**Date:** 2026-07-30

---

## Disclaimer

Technical inventor annex. Not legal advice. Supports copyright deposit description and patent enablement narrative.

---

## 1. Why this is a mission-critical class of system

Mission-critical systems require a **stable definition of legal vs illegal state**. In Golden Phase OS that definition is not only “whatever the scheduler happened to do,” but a **closed four-state algebra** (`gphase_core`) at the **kernel root**:

```text
Sensors / robot / mission / operator
              ↓
     Golden Phase OS model (phase-tagged state)
              ↓
        gphase_core oracle (canonical law)
              ↓
     software result  ↔  hardware result (G1P / G64)
              ↓
        exact verification
              ↓
     commit legal system action (software authority)
```

Properties relevant to mission integrity:

1. **Math does not race** — compose/reflect/topology have definite meanings; concurrent code remains fallible, but the invariant is crisp.  
2. **Root vs layer-7** — the algebra lives in the OS core, not only in an application.  
3. **Dual witness** — FPGA G64 recomputes the same law; mismatch flags corruption, bad bitstream, memory error, or implementation bug.  
4. **Physical reduction** — sealed COM12 runs show real boot, shell, G1P, and G64 atlas **52/12/0**.  
5. **Mission layer** — craft/FDIR builds on the phase base (`src/kernel/craft/`, `craft-mainframe` product).

This annex indexes the **code** and **physical seals** that demonstrate the system is a real OS kernel with hardware-backed phase verification—not a slideshow architecture.

---

## 2. Package identity

| Item | Value |
|------|--------|
| Cargo package | `kairos` **0.1.0** |
| Description | Golden Phase Operating System (GPOS) — Kairos States research OS |
| Primary board path | Zynq-7000 / PZ7020 bare-metal (`kairos_zynq`) |
| Research G64 bin | `kairos_zynq_g64_research` (feature `g64-research`) |

---

## 3. Algebra root (canonical)

| Path | Role |
|------|------|
| `src/core/gphase_core.rs` | Sole compose/reflect/topology authority; φ/α rail; Φ0–Φ3 |
| `src/core/mod.rs` | Core module export |
| `src/kernel/state_root.rs` | Kernel integrity root bound to equation / state |

**Rule:** board drivers and accelerators must not ship a second handwritten algebra table.

---

## 4. Kernel / OS subsystems (real computer)

| Subsystem | Paths |
|-----------|--------|
| Boot (Zynq) | `src/board/zynq7000/kernel_main.rs`, `ps7_init.rs`, `src/bin/kairos_zynq.rs` |
| Scheduler / process | `src/kernel/scheduler.rs`, `process.rs`, `kernel.rs` |
| Memory | `src/kernel/memory.rs`, `src/board/zynq7000/memory_map.rs` |
| StateClock | `src/stateclock/` |
| VM / ISA | `src/vm/`, `src/isa/` |
| PhaseFS | `src/fs/phasefs.rs`, `fs/` |
| Authority / audit | `src/kernel/authority.rs`, statechain modules as present |
| Shell | `src/userland/shell.rs`, `commands/dispatch.rs`, prompt `gpos>` |
| Mission craft / FDIR | `src/kernel/craft/`, `products/craft-mainframe/` |
| G1P | `src/board/zynq7000/g1p.rs`, `userland/commands/g1p_cmd.rs` |
| G64 | `src/board/zynq7000/g64.rs`, `userland/commands/g64_cmd.rs` |
| Oracle | `src/zynq_oracle/`, `src/bin/gpl_oracle.rs` |

Address map (Zynq embodiment):

| Block | Base |
|-------|------|
| G1P | `0x40000000` |
| G64 | `0x40001000` |

---

## 5. Physical seals (PZ7020 COM12)

| Seal | Path | What it proves |
|------|------|----------------|
| Console / feature lineage | `docs/PZ7020_FEATURE_SEAL_20260730.md` | Feature completion narrative |
| Feature run | `fpga/zynq/evidence/kairos_zynq_boot/run_20260730_151124/` | G1P / VM / PhaseFS class seals |
| GEM run | `fpga/zynq/evidence/kairos_zynq_boot/run_20260730_155025/` | Ethernet ladder |
| **G64 run** | `fpga/zynq/evidence/kairos_zynq_boot/run_20260730_175025/` | compose 16/16, reflect 4/4, atlas **52/12/0**, bench 8/8 |
| Default BOOT image | `fpga/zynq/evidence/kairos_zynq_boot/BOOT_PZ7020_FEATURE_TEST/` | Production-style feature BOOT |
| G64 research bit | `fpga/zynq/research/g64_phase_fabric_sidecar/release/PZ7020_G64_SIDECAR/` | `system_g64_sidecar.bit` |

Detailed SHAs: `PHYSICAL_SEAL_INDEX.md`.  
Source file checklist: `SOURCE_FILE_INDEX.md`.

---

## 6. Architecture docs (mission framing)

- `fpga/zynq/GOLDEN_PHASE_ZYNQ_STACK.md` — bare-metal GPOS stack (no Linux required)  
- `docs/TWO_OS_ARCHITECTURE.md` — base OS vs mission OS  
- `docs/research/G64_FINDINGS_AND_MAINLINE_INTEGRATION.md` — HW == `gphase_core`  
- `docs/research/G64_PHYSICAL_COM12_REPORT_20260730.md`  
- `docs/research/G64_ORACLE_TEST_REPORT_20260730.md`  
- `docs/reports/DAY_REPORT_2026-07-30.md`  

---

## 7. Bottom line for examiners / counsel

Golden Phase OS is embodied as:

1. Original unpublished source for a phase-algebra kernel OS;  
2. Working bare-metal deployment on Zynq with interactive shell;  
3. Hardware accelerators that recompute the same algebra;  
4. Sealed physical tests showing bit-exact software/hardware agreement on the full 64-triple atlas.

That combination is the mission-critical integrity story: **invariant at the root, witnesses in software and silicon, commit only when they agree.**
