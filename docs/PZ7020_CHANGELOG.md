# PZ7020 Port Changelog

All notable changes to the PZ7020-SL-C bare-metal port.

## [pz7020-console-proof-20260730] — 2026-07-30

**Preservation commit:** `7b7d75490c06d38d9b45c1540f53aaed0db1e846`
**Branch:** `zynq-pz7020-port`
**Annotated tag:** `pz7020-console-proof-20260730`

### Physically verified

- Golden Phase OS kernel boots and runs on real PZ7020-SL-C (Zynq-7020,
  Cortex-A9) from microSD, over PS UART0 / COM12 (115200 8-N-1).
- Live interactive `gpos>` shell; typed `help` and `state` executed through the
  existing userland.
- G1P responds at `0x40000000` — ID `0x47504C34`, VERSION `0x00020000`.
- Proof: `fpga/zynq/evidence/kairos_zynq_boot/com12_capture_20260730_132633.txt`
  (sha `c52ff347…`).

### Added (platform-support layer + tooling)

- `src/arch/armv7/{startup.S, linker.ld, mod.rs, README.md}` — Cortex-A9 entry.
- `src/arch/mod.rs`, `src/board/mod.rs` — arch/board module roots.
- `src/board/zynq7000/{ps7_init.rs, ps7_init_data.rs (generated), sd.rs, fat.rs,
  kernel_main.rs, uart.rs, memory_map.rs, mod.rs, README.md}`.
- `src/bin/kairos_zynq.rs` — ARM boot binary.
- `zynq_stage1/` — OCM SD loader crate (future FSBL-replacement path; builds,
  not run on HW).
- `scripts/gen_ps7_init.py` — deterministic `.xsa` → `ps7_init_data.rs`.
- `scripts/capture_kairos_com12.ps1` — live COM12 capture harness.

### Changed (wiring only)

- `Cargo.toml` — `zynq-a9` feature + `kairos_zynq` bin.
- `src/lib.rs` — expose `arch`, `board` modules.
- `build.rs` — pick linker script by target (riscv64 vs armv7a).
- `.cargo/config.toml` — armv7a-none-eabi linker entry.
- `src/boot/heap.rs` — `addr_of!` for non-RISC-V heap externs.
- `src/kernel/mod.rs`, `src/userland/mod.rs`, `src/userland/commands/dispatch.rs`
  — gate `phase_transport`/`transport_cmd` behind `std` with a bare-metal
  fallback message.
- `src/core/recursive_shell.rs`, `src/protocol/hybrid_frame.rs` — `no_std`
  `libm` float shims (mirroring the existing `fsqrt` pattern).

### Removed

- `src/arch/armv7/heap.rs` — duplicate `#[global_allocator]` (kept the existing
  one in `src/boot/heap.rs`).

## [unreleased] — post-proof follow-ups

**Second commit on `zynq-pz7020-port` (after the preservation commit + tag).**

### Changed

- `src/userland/splash.rs` — non-hosted arch label selected from `target_arch`
  (`arm` → `baremetal-armv7-a-cortex-a9`, `riscv64` → `baremetal-riscv64`,
  `aarch64` → `baremetal-aarch64`, else `baremetal-unknown-arch`). Cosmetic;
  **software-verified** (compiles; string in ARM ELF), **not** re-flashed/
  re-captured.

### Added (documentation)

- `README_PZ7020.md`
- `docs/PZ7020_PORT_REPORT.md`
- `docs/PZ7020_ARCHITECTURE.md`
- `docs/PZ7020_BUILD_AND_FLASH.md`
- `docs/PZ7020_EVIDENCE.md`
- `docs/PZ7020_FEATURE_MATRIX.md`
- `docs/PZ7020_KNOWN_LIMITATIONS.md`
- `docs/PZ7020_RECOVERY.md`
- `docs/PZ7020_CHANGELOG.md`

### Not yet done (tracked in KNOWN_LIMITATIONS)

- GIC enable + G1P IRQ 61 delivery proof.
- `zynq_stage1` + PCAP bitstream on hardware.
- GEM Ethernet.
- Re-flash/re-capture with corrected label.
