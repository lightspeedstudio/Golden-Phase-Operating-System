<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# System 9 — Utility handoff

Status: **SYS09_COM12_PASS**

COM12 PASS on handoff artifact `eb2f34c3052f9c1a17b0ab34d31385decb0a3cc1c9c75fabacce97e9e63d7b27` (6005972 bytes), source `40eaad4`. UART `com12_sys09.uart` SHA-256 `391218176cef09b7d642b71f5cca9990dec2d91b95cc73c9879185d9dfa0219d` (6994 bytes): `PL PASS` → `HANDOFF PASS` → `JUMP 00100060` → `gpos>`. No `HANDOFF FAIL`. No `PL FAIL`.

Starts after System 8 COM12 PASS `e80b93836b194e5a40db64d18363e8b6d2b8e9de`. Working Stage 1 (`zynq_stage1/src/main.rs`, hash `6637678d0e01b007caf3b5e150ed37e48ed5af6f55cc009a3b16e0d5c4ebd67d`, 9580 bytes) is not edited.

This revision is load + jump of the locked PATH1_G64 Utility only. No System 10. No eFuse. No BBRAM.

Inspected as behavioral reference only. No vendor FSBL C was copied into `zynq_stage1/owned`.

## Locked Utility ELF (gold PATH1_G64)

| Field | Value |
| --- | --- |
| Path | `fpga/zynq/release/PZ7020_G1P_PATH1_G64/kairos_zynq.elf` |
| SHA-256 | `447f9fbdec6bbdb03227c8620baa854f3a141d52ec71452251e2f63e1852eaf8` |
| Bytes | 2475164 |
| `readelf` | arm-gnu-toolchain 13.3.Rel1 `arm-none-eabi-readelf` |

`readelf -h -l -S` and symbol table (this tree):

| Item | Value |
| --- | ---: |
| EI_CLASS / EI_DATA | ELF32 little-endian |
| `e_entry` | `0x00100060` (`_start`) |
| PT_LOAD count | 1 (plus NOTE and GNU_STACK) |
| PT_LOAD p_offset | `0x001000` |
| PT_LOAD p_vaddr / p_paddr | `0x00100000` |
| PT_LOAD p_filesz | `0x001D84D4` (1934548) |
| PT_LOAD p_memsz | `0x001DF158` (1962328) |
| PT_LOAD p_align | `0x1000` |
| PT_LOAD flags | RWE |
| `vectors` | `0x00100040` |
| `_start` / `e_entry` | `0x00100060` |
| `rust_main` | `0x001002DC` |
| `.bss` | `0x002D9000` len `0x6158` (NOBITS, align 4096) |
| `__bss_start` | `0x002D9000` |
| `__bss_end` | `0x002DF158` |

`p_memsz - p_filesz` = `0x6C84`. That tail is alignment from `.data` end `0x002D84D4` to `.bss` plus `.bss`. Gold Bootgen P2 byte length equals **p_filesz** (1934548), not p_memsz.

## Working Stage 1 immediately before jump

Sources hashed on this tree:

| Path | SHA-256 | Bytes |
| --- | --- | ---: |
| `src/board/zynq7000/stage1/handoff.rs` | `896f5cb151ca041a33c9d7509e6bf1166c94461c96ed38026cb95c453bc92cd2` | 528 |
| `src/board/zynq7000/stage1/wdt.rs` | `0a2bf6f73b56098847cc6f6a3bbd9379ffcbcd3d4ca8e459f1b7cf5417d5d39d` | 876 |
| `src/arch/armv7/startup.S` | `efd0373ad72d963b158d345a7df8e395bc9aaf758f1d5f3b4838f8276a5b39c4` | 3644 |
| `zynq_stage1/owned/src/start.S` | `6a2d04f216384007c180dd9bfdbf3b4d40cb588609d85ab4acfd07b4d73d37a5` | 608 |

Owned System 3–9 device entry is `owned/src/start.S` (not vendor FSBL). That is the CPU state System 9 inherits after Systems 3–8.

| Topic | Proven state at `handoff::jump` | Do not invent |
| --- | --- | --- |
| MMU | SCTLR.M **off** (owned `_start` BIC `#0x0005`) | Do not enable MMU. Utility `kernel_main` / `cpu1::enable_mmu_scu_cpu0` turns it on after jump. |
| D-cache | SCTLR.C **off** | Do not enable C. |
| I-cache | SCTLR.I **off** (BIC `#0x1000`). Jump still executes ICIALLU. | Do not enable I. |
| BTB | Jump executes BPIALL (`c7,c5,6`) then DSB, ISB | Same three ops, no extra. |
| Interrupt mask | Stage 1 does not write CPSR I/F. Utility `_start` returns to SVC with I masked (`orr #0x80`) until GIC init. | Do not unmask IRQs in Stage 1. |
| VBAR | Stage 1 does not write VBAR. Utility `_start` sets VBAR = `vectors` (`0x00100040`). | Do not write VBAR in Stage 1. |
| CPU1 | Stage 1 does not write `0xFFFFFFF0` / CPU1 start. Utility may `wake_cpu1_and_report` after splash. | Do not release CPU1 in Stage 1. |
| Watchdog | `wdt::stop` at platform ready; stays stopped (working comment: no 100 s FSBL expire). | Do not start SWDT. |
| DevCfg | System 8 `post_config`: `LVL_SHFTR_EN` post mask, `FPGA_RST_CTRL=0` | Do not re-init or lock DevCfg further. |
| SD | Host left after ADMA CMD18 | Do not reset SDHCI. |

Working `main.rs` copy: every `ps_ddr_partitions` payload is `copy_nonoverlapping` to `load_addr` for **file length**, then `STAGE1: JUMP` + 8 hex digits of `exec_addr`, then `handoff::jump`. It does not zero `p_memsz - p_filesz`. `.bss` is cleared by Utility `startup.S` after jump.

Owned loader copies that same file-length image to `0x00100000`, then zeros the ELF PT_LOAD tail (`p_filesz .. p_memsz`) so the in-DDR image matches the ELF load contract. Utility `_start` still zeros `__bss_start .. __bss_end`. That tail zero is the only addition versus working Stage 1 copy; it is the ELF `p_memsz` gap, not a CPU-mode change.

Working jump body (`handoff.rs`):

1. `MCR p15,0,z,c7,c5,0` ICIALLU (ARM Cortex-A9 TRM I-cache invalidate all)
2. `MCR p15,0,z,c7,c5,6` BPIALL (BTB invalidate all)
3. `DSB`
4. `ISB`
5. Branch to `entry` as `extern "C" fn() -> !`

## Known-working JUMP / `gpos>` UART

Gold FSBL path (not owned Stage 1). File `fpga/zynq/evidence/kairos_zynq_boot/com12_path1_g64_combined_20260814_174430.txt` SHA-256 `bf3ca7940c61e22455276b26d218bb7bd44203859e2362a26e263a152e102800` (7475 bytes). Vendor FSBL is silent. First Utility text is `MMU: CPU0 identity...` then splash `type help at gpos>` then prompt `gpos>`.

Working owned shortcut prints `STAGE1: JUMP ` + `exec_addr` hex before `handoff::jump`. It does not print `HANDOFF PASS`.

System 9 device UART (after System 8 PCAP lines):

```
PCAP BEGIN
PL PASS
HANDOFF PASS
JUMP 00100060
```

Then Utility continues (MMU / splash / `gpos>`). No `SYS08 HOLD`. No `SYS09 HOLD`. Failure prints `HANDOFF FAIL` and does not jump.

## Host model vs locked working handoff

`handoff::locked_working_state()` is the compare target. New handoff must match:

| Op | Value |
| --- | --- |
| Copy | dest `0x00100000` len `0x001D84D4` |
| Zero | dest `0x002D84D4` len `0x6C84` (PT_LOAD tail through `.bss`) |
| I-cache | ICIALLU `c7,c5,0` |
| BTB | BPIALL `c7,c5,6` |
| Barrier | DSB then ISB |
| Jump | `0x00100060` |
| CPU1 / SWDT / VBAR | not written |

`cargo test --lib --features std`: host compare against that state, gold `BOOT.BIN` P2 plan, locked ELF PT_LOAD.

Silicon PASS image: `zynq_stage1/owned/build/BOOT_SYS09_HANDOFF.BIN` SHA-256 `eb2f34c3052f9c1a17b0ab34d31385decb0a3cc1c9c75fabacce97e9e63d7b27` (6005972 bytes), on-card match, source `40eaad4`. P0 unsigned `sys09_plat` PT_LOAD 19288 bytes, `e_entry` `0x00000000`. P1 PATH1_G64 `system.bit`. P2 Utility filesz `0x001D84D4` loaded at `0x00100000` and jumped at `0x00100060`. Capture: `fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS09_20260820/com12_sys09.uart`.

## Non-claims

- Not a new MMU/cache/CPU1 policy
- Not eFuse, not BBRAM, not BootROM RSA
- Not System 10
