# Source File Index — Kernel / OS / Accelerators

**Docket:** CAREGAGA-GPOS-2026-001  
**Repo root:** `kairos/`  
**Package:** `kairos` 0.1.0  

Paths relative to `kairos/` unless noted.

---

## Algebra / core

| Path | Notes |
|------|--------|
| `src/core/gphase_core.rs` | Canonical compose, reflect, topology, φ/α rail |
| `src/core/mod.rs` | Core module |
| `src/lib.rs` | Crate root / re-exports |

---

## Boot / board

| Path | Notes |
|------|--------|
| `src/board/zynq7000/kernel_main.rs` | A9 bare-metal entry / shell loop |
| `src/board/zynq7000/memory_map.rs` | G1P / G64 / UART / DDR bases |
| `src/board/zynq7000/g1p.rs` | G1P polled MMIO driver |
| `src/board/zynq7000/g64.rs` | G64 polled MMIO driver (`g64-research`) |
| `src/board/zynq7000/mod.rs` | Board modules |
| `src/board/zynq7000/ps7_init.rs` | PS init (as present) |
| `src/bin/kairos_zynq.rs` | Production Zynq bin |
| `src/boot/` | RISC-V / alternate boot paths |

---

## Kernel

| Path | Notes |
|------|--------|
| `src/kernel/scheduler.rs` | Scheduler |
| `src/kernel/process.rs` | Processes |
| `src/kernel/memory.rs` | Memory |
| `src/kernel/kernel.rs` | Kernel glue |
| `src/kernel/authority.rs` | Authority tiers |
| `src/kernel/state_root.rs` | State / equation root |
| `src/kernel/craft/` | Mission FDIR / craft |
| `src/kernel/phase_math/` | Phase math controller / backends |
| `src/kernel/trace.rs` | Trace (as present) |

---

## StateClock / VM / FS / userland

| Path | Notes |
|------|--------|
| `src/stateclock/` | Dual clock / events / geometry |
| `src/vm/` | COMPOSE / REFLECT execution |
| `src/isa/` | Encoding / assembler |
| `src/fs/phasefs.rs` | PhaseFS |
| `src/fs/` | Block / inode / directory |
| `src/userland/shell.rs` | Interactive shell |
| `src/userland/commands/dispatch.rs` | Command catalog |
| `src/userland/commands/g1p_cmd.rs` | G1P shell |
| `src/userland/commands/g64_cmd.rs` | G64 shell (feature-gated) |
| `src/brand.rs` | GPOS branding / `gpos>` |

---

## Oracle / tooling

| Path | Notes |
|------|--------|
| `src/zynq_oracle/` | Host oracle generation |
| `src/zynq_oracle/g64_export.rs` | G64 research oracle emit |
| `src/bin/gpl_oracle.rs` | CLI oracle tool |

---

## FPGA project RTL (selected)

| Path | Notes |
|------|--------|
| `fpga/zynq/research/g64_phase_fabric_sidecar/rtl/g64_axi_lite_sidecar.v` | Sidecar top |
| `fpga/zynq/research/g64_phase_fabric_sidecar/rtl/g64_compose.v` | Compose |
| `fpga/zynq/research/g64_phase_fabric_sidecar/rtl/g64_reflect.v` | Reflect |
| `fpga/zynq/research/g64_phase_fabric_sidecar/rtl/g64_triple_unit.v` | Triple unit |
| `fpga/zynq/research/g64_phase_fabric_sidecar/scripts/build_g64_sidecar_bd.tcl` | BD build |

---

## Products

| Path | Notes |
|------|--------|
| `products/craft-mainframe/` | Mission product build-out on GPOS |
| `Cargo.toml` | Features: `zynq-a9`, `g64-research`, `craft`, `mission`, … |

---

## IP / notices

| Path | Notes |
|------|--------|
| `COPYRIGHT_NOTICE` | © 2026 unpublished proprietary |
| `docs/ip/` | This IP pack |
