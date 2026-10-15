# KairOS Board v1 — Hardware Requirements

**Project:** Metatron Research · Richard Careaga · Thousand Oaks, California  
**Date:** 2026-07-07  
**Status:** Frozen from measured `kairos_boot` builds + 314-test OS gate  
**Companion:** `HARDWARE_MEASUREMENTS.md` · `hardware_charts.html` · `release_snapshot/`

---

## Executive summary

KairOS Board v1 is a **generous first prototype** — not a minimum-cost board. Measured baremetal images fit comfortably in **512 KB–1 MB flash** for the kernel alone; plan **32–64 MB QSPI NOR** for A/B images, recovery, and config. RAM budget from linker maps shows **~512–560 KB BSS** for full tier at boot layout — target **256 MB DDR minimum, 512 MB preferred** for development headroom (PhaseFS, statechain, SSH, AI bridge on hosted path; baremetal tiers are smaller).

**Boot mode (measured):** RISC-V **machine mode (M-mode)**, linked at `0x80000000`, QEMU `-bios none` — **no OpenSBI** in current `kairos_boot`. Future supervisor-mode port may add OpenSBI; v1 hardware must support direct M-mode entry from ROM/flash.

**ISA target:** `riscv64gc-unknown-none-elf` — RV64I + M + A + F + D + C (GC).

---

## 1. Measured software footprint (inputs to BOM)

### Hosted operator binaries (x86_64 Windows release)

| Artifact | Size |
|----------|-----:|
| `kairos_host.exe` | 4.15 MB |
| `kairos_ssh.exe` | 4.61 MB |

### Baremetal `kairos_boot` tiers (release, RISC-V)

| Tier | Cargo features | `.bin` (flash) | `.elf` | `.text` | `.rodata` | `.bss` |
|------|----------------|---------------:|-------:|--------:|----------:|-------:|
| Minimum | `bm-minimum` | **381 KB** | 1.03 MB | 296 KB | 80 KB | 512 KB |
| Embedded | `bm-embedded` | **438 KB** | 1.14 MB | 345 KB | 88 KB | 547 KB |
| Full | `bm-full` | **557 KB** | 1.42 MB | 442 KB | 109 KB | 547 KB |

Artifacts: `release_snapshot/baremetal/tiers/{minimum,embedded,full}/`

### QEMU smoke (not physical silicon)

- Splash + `kairos>` prompt: **825 ms** (`release_snapshot/baremetal/qemu_smoke.json`)
- Target: QEMU `virt`, M-mode, UART only

### Test gate

- **314 integration tests passed**, 0 failed (2026-07-07 incremental safe run)
- Log: `release_snapshot/logs/test_output_incremental.txt`

---

## 2. Processor

| Requirement | v1 spec | Rationale |
|-------------|---------|-----------|
| Architecture | **64-bit RISC-V** | Matches `riscv64gc-unknown-none-elf` |
| Hart count | **≥ 4 cores** | Phase scale benchmark shows parallel gain to 8 workers; 4 harts exercises multi-stream kernel |
| Privilege | **M-mode required**; S-mode optional later | Current boot is direct M-mode |
| Atomic | **A** extension | Required in `riscv64gc` |
| Multiply/divide | **M** extension | Required |
| Float | **F/D** | GC target |
| Compressed | **C** | GC target |
| MMU | **Recommended** | Strong process isolation / future Sv39 port |
| Timer + IRQ | **Mandatory** | `mtime` / `mtimecmp` or CLINT; external interrupt controller |
| Performance counters | **Desirable** | PHI-STABILITY, phaseperf on silicon |

---

## 3. Memory

| Resource | v1 target | Notes |
|----------|-----------|-------|
| DDR | **256 MB min · 512 MB preferred** | BSS ~547 KB full tier; headroom for heap, PhaseFS cache, logs |
| QSPI NOR | **32–64 MB** | A/B kernel + recovery + config (see §8) |
| microSD / eMMC | **Yes (dev)** | Logs, benchmark exports, OSF bundles |
| FPGA BRAM | **Optional** | Boot-critical tables, future phase LUT |

**ROM budget per image slot:** allocate **≥ 1 MB** per slot (full `kairos_boot.bin` = 557 KB today + growth).

---

## 4. Networking

| Interface | v1 | Notes |
|-----------|-----|-------|
| Ethernet | **1 GbE** | Management, SSH (hosted sidecar or future baremetal stack), logs, AI control |
| 10 GbE / PCIe | **Modular expansion** | Not required for boot; reserve connector |
| PHY | Dedicated, reset, ref clock | |
| MAC storage | EEPROM / flash | |
| LEDs | Link + activity | |
| Magnetics | Integrated RJ45 | |

PhaseLink / PhaseNet on baremetal: stub today; v1 board wires **Ethernet PHY** for future `phaselink_eth` bring-up.

---

## 5. Debug and recovery (mandatory)

- UART console header (3.3 V TTL, documented pins)
- **JTAG** (OpenOCD-compatible; see `boards/kairos_board_v1/openocd.cfg`)
- Reset button (accessible)
- Boot-mode strap / jumper (ROM · flash A · flash B · recovery)
- Test points: clocks, reset, 3.3 V / 1.8 V / 1.2 V
- Status LEDs: power, boot, fault, heartbeat
- **Hardware watchdog** + reset reason register
- Independent reset supervisor

---

## 6. Timers and StateClock hardware

KairOS StateClock (measured in `stateclock_tests`, 19 tests pass) uses kernel time abstraction. Map to hardware:

| Function | v1 hardware |
|----------|-------------|
| Monotonic system timer | `mtime` / CLINT or SoC timer block |
| Per-hart timer IRQ | `mtimecmp` per hart |
| RTC | External RTC chip (I²C) for wall clock |
| Watchdog | SoC WDT → reset reason register |
| StateClock skew/dilation | **Software** on CPU clock; log corrections to statechain |
| Precision reference | Optional TCXO + **PPS input** for network time sync |

**v1 default:** main CPU clock for scheduling; RTC for wall time; PPS optional on expansion header.

---

## 7. Storage and boot layout

```
Boot ROM (immutable first stage, verify signature)
    |
    +-- Image A: known-good kairos_boot.bin  (≤1 MB slot)
    +-- Image B: update candidate
    +-- Recovery image (read-only)
    +-- Config partition (boot counters, rollback)
    +-- PhaseFS / operator data (eMMC or 2nd flash region)
    +-- Statechain / checkpoint partition (wear-leveled)
```

| Field | Requirement |
|-------|-------------|
| A/B images | Yes — failed update must not destroy only bootable copy |
| Image hashes | SHA-256 in config partition |
| Rollback counter | Persistent |
| Boot-success marker | Set by KairOS after `kairos>` healthy |
| Failed-boot counter | Triggers automatic fallback to Image A |
| Power-loss-safe update | Two-phase commit: write B, verify, swap pointer |

Measured kernel image to protect: **570,340 bytes** (`bm-full` `.bin`).

---

## 8. Security hardware (development board)

| Capability | v1 |
|------------|-----|
| Secure key storage | SE / OTP or TPM-style I²C |
| Unique device ID | Factory-programmed |
| TRNG | Hardware RNG for ANOM / SSH host key seed |
| Secure boot | Verify signature on `kairos_boot.bin` in ROM stage |
| Write-protected boot region | ROM + protected flash sector |
| SSH host keys | **Not plain files on NOR** — derive from secure store |
| Checkpoint signing key | Secure store, operator-initiated |

---

## 9. FPGA-specific (optional mezzanine)

Reserve block for future **native phase accelerator** (not v1 software dependency):

| Block | Purpose |
|-------|---------|
| Phase compose LUT | Hot `compose` |
| Reflection unit | `reflect` |
| Legal transition check | Scheduler assist |
| Phase tag encode/decode | PhaseLink assist |
| Statechain hash feed | SHA-256 pipeline |
| Worker queue | Lane 0/1/2 dispatch |
| Performance counters | Match `phaseperf` |

**v1 goal:** prove CPU `kairos_boot` first; FPGA metrics (LUTs, Fmax, slack) **TBD on silicon**.

`fpga_golden_tests`: 5/5 pass in simulation only.

---

## 10. Board interfaces (minimum)

UART · JTAG · Ethernet · QSPI · DDR · microSD/eMMC · USB (device for console) · GPIO · SPI · I²C · IRQ inputs · **expansion connector**

**Craft / control reserved (future):** CAN-FD · redundant UART/Ethernet · RS-422/485 · PWM · ADC · encoder · E-stop · isolated actuator

---

## 11. Power design (document before layout)

- Input: **12 V DC** (barrel) or **5 V USB-C** (dev)
- Rails: 3.3 V (I/O), 1.8 V (PHY), 1.2 V (DDR / core), 1.0 V (core)
- Sequencing: core → DDR → PHY → I/O
- Measure **every rail** at test point
- Brownout: hold reset until DDR stable
- Thermal: board temp sensor (I²C)

**Not measured yet:** idle/boot/max power — requires physical board.

---

## 12. PCB requirements (freeze before fab)

- 6-layer minimum if DDR + GbE
- Controlled impedance: DDR, Ethernet pairs
- JTAG + UART at board edge
- FPGA config pins accessible
- Boot straps readable on silkscreen
- Independent schematic review before fabrication

---

## 13. Software BSP

Directory: `boards/kairos_board_v1/` (see README there)

Keep **board description separate** from kernel semantics (architecture → SoC → board).

---

## 14. Bring-up order

1. Power rails → 2. Reset timing → 3. FPGA/SoC config → 4. JTAG ID → 5. Halt/step CPU → 6. UART single char → 7. RAM test → 8. Timer IRQ → 9. Boot KairOS to prompt → 10. PhaseFS mount → 11. Phase compose tests → 12. Statechain/checkpoint → 13. Ethernet → 14. PhaseLink loopback → 15. SSH (hosted sidecar) → 16. AI bridge → 17. 4-worker benchmark → 18. 24 h stability → 19. Watchdog recovery → 20. Bad-image recovery → 21. Power-loss FS test

---

## 15. Board acceptance criteria

Board v1 accepted when demonstrated on **physical hardware**:

- [ ] Repeated cold boot / warm reset to `kairos>` in **< 2 s** (target; QEMU = 825 ms)
- [ ] JTAG + UART recovery
- [ ] RAM + timer verified
- [ ] Filesystem persistence across power cycle
- [ ] Statechain + checkpoint verify clean
- [ ] Ethernet link + ping
- [ ] 4-hart phase benchmark matches hosted results within tolerance
- [ ] Watchdog reset recorded in reset-reason register
- [ ] Fallback from corrupted Image B to Image A

---

## Chart and data files

| Asset | Path |
|-------|------|
| Fibonacci scaling JSON | `release_snapshot/benchmarks/phase_graph_data.json` |
| Parallel scale JSON | `release_snapshot/benchmarks/phase_scale_data.json` |
| **Interactive charts (HTML)** | `docs/hardware_charts.html` |
| Tier sizes JSON | `release_snapshot/baremetal/tier_sizes.json` |
| QEMU smoke | `release_snapshot/baremetal/qemu_smoke.json` |

---

*Metatron Research Project · KairOS v0.1.0 milestone · commit `b0627b5` (local)*
