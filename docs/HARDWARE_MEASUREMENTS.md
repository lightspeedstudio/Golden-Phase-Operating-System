# KairOS Hardware Measurements

**Date:** 2026-07-07  
**Host:** Windows 10 (build 26200), x86_64-pc-windows-msvc  
**CPU:** 32 logical processors  
**RAM:** 63.8 GB  
**Toolchain:** Rust 2021, kairos v0.1.0  

All figures below are from measured artifacts in `release_snapshot/` unless noted.

---

## Hosted release binaries (measured)

| Artifact | Bytes | MB | Notes |
|----------|------:|---:|-------|
| `binaries/kairos_host.exe` | 4,356,096 | 4.15 | Interactive hosted runtime + shell |
| `binaries/kairos_ssh.exe` | 4,830,208 | 4.61 | Hosted runtime + SSH operator server |

Source: `release_snapshot/docs/BUILD_SIZE_REPORT.txt` (cross-checked against on-disk sizes 2026-07-07).

---

## Baremetal RISC-V images (measured)

Target: `riscv64gc-unknown-none-elf`, feature tier `bm-full`.

| Artifact | Bytes | Notes |
|----------|------:|-------|
| `baremetal/kairos_boot.elf` | 1,489,624 | Full debug ELF |
| `baremetal/kairos_boot.bin` | 570,340 | Flash/load image |
| `baremetal/kairos_boot.stripped.elf` | 1,400,536 | Stripped ELF |
| `baremetal/kairos_boot.nodebug.elf` | 1,400,640 | No-debug variant |

### Tier size comparison (`baremetal/tier_sizes.json`)

| Tier | Features | ELF bytes | BIN bytes |
|------|----------|----------:|----------:|
| minimum | `bm-minimum` | 1,077,544 | 389,964 |
| embedded | `bm-embedded` | 1,199,968 | 448,020 |
| full | `bm-full` | 1,489,624 | 570,340 |

### QEMU smoke (hosted runner, not physical board)

`baremetal/qemu_smoke.json`:

| Signal | Value |
|--------|------:|
| `has_splash` | true |
| `has_prompt` | true |
| `has_halt` | true |
| `prompt_ms` | 825 |

Serial capture: `baremetal/qemu_smoke_serial.log` (6,693 bytes).

**Blocker for physical baremetal:** No on-device boot timing, power draw, or flash-program latency has been measured. QEMU smoke confirms image boots to operator prompt; real RISC-V hardware (e.g. QEMU `virt` vs custom board) still needs separate characterization.

---

## Phase benchmark — fibonacci scaling (`benchmarks/phase_graph_data.json`)

Profile: hosted-debug, 20 repetitions per point. Engines: linear, binary (fast-doubling), phase.

| n | Digits | Linear ms | Binary ms | Phase ms | Phase speedup vs linear |
|--:|-------:|----------:|----------:|---------:|------------------------:|
| 50 | 11 | 0.0030 | 0.0018 | 0.0025 | 0.72× |
| 100 | 21 | 0.0067 | 0.0024 | 0.0035 | 0.69× |
| 500 | 105 | 0.0693 | 0.0065 | 0.0080 | 0.82× |
| 1,000 | 209 | 0.1625 | 0.0103 | 0.0125 | 0.82× |
| 5,000 | 1,045 | 1.4393 | 0.0427 | 0.0478 | 0.89× |
| 10,000 | 2,090 | 4.3240 | 0.1169 | 0.1270 | 0.92× |
| 100,000 | 20,899 | 295.83 | 4.58 | 4.71 | 0.97× |
| 1,000,000 | 208,988 | 29,234.18 | 164.96 | 239.55 | 0.69× |

At fib(1M), linear is ~122× slower than binary; phase trails binary by ~45% on this host.  
**Note:** `phase_benchmark_graph_data` test was **not re-run** during preservation (system crash risk); JSON cleaned from prior `.raw.txt` export.

---

## Phase benchmark — parallel scale (`benchmarks/phase_scale_data.json`)

Profile: release, target fib(1,000,000), 5 repetitions, 32 logical CPUs.

| Workers | Binary ms | Phase ms | Binary jobs/s | Phase jobs/s | Binary speedup | Phase speedup | Result match |
|--------:|----------:|---------:|--------------:|-------------:|---------------:|--------------:|:------------:|
| 1 | 149.47 | 166.04 | 6.7 | 6.0 | 1.02 | 0.94 | yes |
| 2 | 146.03 | 145.44 | 13.7 | 13.8 | 2.01 | 2.01 | yes |
| 4 | 183.28 | 175.69 | 21.8 | 22.8 | 3.44 | 4.36 | yes |
| 8 | 178.42 | 175.27 | 44.8 | 45.6 | 6.51 | 6.60 | yes |
| 32 | 354.38 | 362.68 | 90.3 | 88.2 | 13.34 | 13.18 | yes |

Peak throughput at 8 workers (~45 jobs/s phase). Scaling degrades at 32 workers (contention).

Re-validated 2026-07-07: `phase_benchmark_scale_data` test passed in **29.4 s** (`CARGO_BUILD_JOBS=1`, `--test-threads=1`).

---

## Integration test results (incremental safe run)

Strategy: `CARGO_BUILD_JOBS=1`, `CARGO_INCREMENTAL=0`, one test crate at a time, `--test-threads=1`.  
Prior crash preserved in `release_snapshot/logs/test_output_partial.txt` (died on `export_graph_data` fib 1M).

| Test crate | Exit | Tests passed | Notes |
|------------|-----:|-------------:|-------|
| lib (unit) | 0 | 20 | Partial run |
| kairos_host (bin) | 0 | 0 | Partial run |
| kairos_ssh (bin) | 0 | 0 | Partial run |
| ai_bridge_tests | 0 | 19 | Partial run |
| algebra_tests | 0 | 5 | Partial run |
| anom_tests | 0 | 27 | Partial run |
| boot_tests | 0 | 5 | Partial run |
| fpga_golden_tests | 0 | 5 | Partial run |
| fs_tests | 0 | 10 | Partial run |
| isa_tests | 0 | 9 | Partial run |
| memory_tests | 0 | 4 | Partial run |
| object_protection_tests | 0 | 32 | Partial run |
| object_protection_transcript | 0 | 1 | Partial run |
| engineering_tools_tests | 0 | 9 | Requires `--features bm-engineering-tools` |
| phase_benchmark_tests | 0 | 7 | Incremental |
| phase_math_tests | 0 | 5 | Incremental |
| phaselink_tests | 0 | 17 | Incremental |
| phaselink_transcript | 0 | 1 | Incremental |
| phasenet_tests | 0 | 10 | Incremental |
| phi_stability_tests | 0 | 8 | Incremental |
| process_tests | 0 | 4 | Incremental |
| protocol_tests | 0 | 6 | Incremental |
| runtime_tests | 0 | 8 | Incremental |
| scheduler_tests | 0 | 5 | Incremental |
| ssh_server_tests | 0 | 9 | Incremental |
| state_domain_tests | 0 | 11 | Incremental |
| stateclock_tests | 0 | 19 | Incremental |
| statechain_tests | 0 | 16 | Incremental |
| terminal_shell_tests | 0 | 10 | Incremental |
| terminal_tests | 0 | 11 | Incremental |
| topology_tests | 0 | 4 | Incremental |
| userland_tests | 0 | 10 | Incremental |
| vm_tests | 0 | 16 | Incremental |
| phase_benchmark_scale_data | 0 | 1 | Incremental, 29.4 s |
| phase_benchmark_graph_data | SKIP | — | Use existing JSON; do not re-export fib 1M |

**Totals:** 32 integration crates + lib/bin tests; **314 tests passed**, **0 failed**, **1 crate skipped** (graph export).

---

## Build size summary (from BUILD_SIZE_REPORT)

| Category | Size |
|----------|-----:|
| `src/` Rust | 1.00 MB (246 files) |
| `tests/` Rust | 0.16 MB (32 files) |
| `libkairos.rlib` (release) | 10.45 MB |

---

## Still needs hardware measurement

1. **Physical RISC-V board** — boot time, UART throughput, virtio-net/phaselink on real silicon (QEMU smoke only so far).
2. **FPGA phase_math backend** — golden tests pass in simulation; no on-chip timing or resource utilization captured.
3. **Release-profile graph benchmark** — `phase_graph_data.json` is hosted-debug; release re-export deferred.
4. **Power / thermal** — not measured on any target.

---

## Charts and board spec

| Resource | Path |
|----------|------|
| Interactive charts (HTML) | `docs/hardware_charts.html` |
| Board v1 requirements | `docs/KAIROS_BOARD_V1_REQUIREMENTS.md` |
| BSP skeleton | `boards/kairos_board_v1/` |
| Benchmark JSON (graph source data) | `release_snapshot/benchmarks/phase_graph_data.json` |
| Benchmark JSON (scale source data) | `release_snapshot/benchmarks/phase_scale_data.json` |

---

## Preservation paths

| Resource | Path |
|----------|------|
| This document | `docs/HARDWARE_MEASUREMENTS.md` |
| Snapshot checksums | `release_snapshot/MANIFEST.sha256` |
| Test log (partial crash) | `release_snapshot/logs/test_output_partial.txt` |
| Test log (incremental) | `release_snapshot/logs/test_output_incremental.txt` |
| Exit codes | `release_snapshot/logs/test_exit_codes.json` |
