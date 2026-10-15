<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# System 3 — PS7 / DDR / UART reference

Extracted **before** the owned platform entry was written. Numeric values are cited. Nothing here is a guessed delay, PLL, DDR parameter, or timeout.

**Working Stage 1 source:** `zynq_stage1/src/main.rs` `rust_main` → `wdt::stop` then `stage1::ps7::bringup` (`mio` → `pll` → `clocks` → `ddr` → `peripherals`) then `ddr_smoke`. SHA-256 `6637678d0e01b007caf3b5e150ed37e48ed5af6f55cc009a3b16e0d5c4ebd67d`. File is not edited.

**Sealed XSA stream (board-proven FSBL PS7):** `src/board/zynq7000/ps7_init_data.rs` silicon-3.0 arrays. Header: `.xsa` SHA-256 `cf4a3316…`, `ps7_init.c` SHA-256 `61c47f3c…`, encoded-stream SHA-256 `64eb150f…`. Interpreter: `ps7_init.rs` (`MASKWRITE` RMW, `MASKPOLL`, `MASKDELAY`). Shortcut `stage1/ps7.rs` uses the **same address/mask/value/order** as those arrays (verified by decoding both).

Locked host trace of that sequence: `docs/boot/SYS03_WORKING_PS7_TRACE.jsonl` (209 ops, EXIT stripped).

## Board / documents

| Item | Value | Source |
| --- | --- | --- |
| Board | PZ7020-SL-C | System 1 lock |
| PS_CLK | 33.333333 MHz on `PS_REF_CLK` | Schematic `PS_CLK_33d33M`; user manual p.9; `ps7_config_dump.txt` `PCW_CRYSTAL_PERIPHERAL_FREQMHZ=33.333333` |
| DDR part (schematic) | MT41K256M16TW-107IT:P | `Puzhi_PZ-StarLite_Schematic.pdf.full.txt` |
| DDR part (PS7 preset used) | MT41K256M16 RE-125, DDR3LV, 16-bit, 533.333333 MHz, highaddr `0x1FFFFFFF` | `ps7_config_dump.txt` / `FSBL_PS7_COMPARISON.txt` |
| UART0 | MIO 10..11, 115200 8N1 COM12 | System 1 lock; `PCW_UART0_UART0_IO` |
| SD0 | MIO 40..45 | same |
| AMD PS7 | UG585 *Zynq-7000 SoC Technical Reference Manual* Ch. 2 (MIO/SLCR), Ch. 25 clocks/PLL, Ch. 10 DDR, Ch. 19 UART, SWDT, `BOOT_MODE_USER` | Working firmware comments + register map |
| AMD DC | DS187 *Zynq-7000 DC and AC Switching* — PLL FBDIV and clock rates below are from the **working SLCR values**, not a guessed DS187 table row (DS187 PDF is not in this tree) | — |
| DDR datasheet | Micron MT41K256M16 (kit TW-107; Vivado RE-125 @ 533.33 MHz). Packed `DRAM_param_*` words are taken from the sealed XSA / working `ddr()` list, not re-derived here | `MT41K256M16TW-107_IT.pdf.eth_hits.txt`; `ps7_config_dump.txt` |

## PLL divisors (working `pll()` / `PS7_PLL_INIT`)

`APU_FREQ_HZ = 766666687` (`ps7_init_data.rs`, `stage1/mmio.rs`). Timer = APU/2 (UG585 SCU global timer).

| PLL | CTRL FBDIV field (mask `0x0007F000`) | FBDIV | VCO | Then |
| --- | --- | --- | --- | --- |
| ARM | `0x0002E000` | 46 | 33.333333 MHz × 46 = 1533.333 MHz | `ARM_CLK_CTRL` `0x1F000200`: divisor 2 → **766.667 MHz** APU |
| DDR | `0x00020000` | 32 | 33.333333 MHz × 32 = 1066.667 MHz | `DDR_CLK_CTRL` `0x0C200003`: 2× enable, divisor 2 → **533.333 MHz** DDR |
| IO | `0x0001E000` | 30 | 33.333333 MHz × 30 = 1000.000 MHz | UART/SDIO source |

CFG words (working): ARM `0x000FA3C0`, DDR `0x0012C220`, IO `0x001452C0`. Sequence: bypass, FBDIV, reset, **poll `PLL_STATUS` `0xF800010C`**, un-bypass (UG585 PLL). Poll masks: ARM `0x1`, DDR `0x2`, IO `0x4`.

## UART divisor (working `peripherals()` / `PS7_PERIPHERALS_INIT`)

| Reg | Addr | Mask | Value | Source |
| --- | --- | --- | --- | --- |
| UART_CLK_CTRL | `0xF8000168` | `0x00003F31` | `0x00000501` | clocks(); UG585 clock control. DIVISOR0=5, UART0 CLKACT |
| Baud_RATE_DIV | `0xE0000034` | `0x000000FF` | `0x00000006` | peripherals |
| Baud_RATE_GEN | `0xE0000018` | `0x0000FFFF` | `0x0000007C` (124) | peripherals |
| UART CR | `0xE0000000` | `0x000001FF` | `0x00000017` | peripherals |
| UART MR | `0xE0000004` | `0x000003FF` | `0x00000020` | peripherals |

UG585 UART: `baud = uart_ref / (CD × (BDIV+1))` = `uart_ref / (124 × 7)`. IO PLL 1000 MHz / DIVISOR0 5 = 200 MHz ref → computed 230414. Gold FSBL uses these same bytes and COM12 is operated at **115200**. System 3 **does not change** CD/BDIV. COM12 remains 115200 8N1 as in System 1.

## DDR clock, timing, DDRIOB, calibration

DDRC/PHY MASKWRITE list is `PS7_DDR_INIT` / `stage1::ps7::ddr` (identical). First DDRC write: `0xF8006000` mask `0x0001FFFF` value `0x00000084`. Enable: value `0x00000085`. Named DRAM words from working comments: `DRAM_param_reg0` `0x0004285B` through `reg4` `0x00000000`, `DRAM_EMR_reg` `0x00000008`, `DRAM_EMR_MR_reg` `0x00040B30`, etc. (full list in the JSONL / `ops_data.rs`).

**Order (board-proven, do not invert):**

1. DDRIOB analog `0xF8000B40`–`0xF8000B70` in **MIO** phase (seq 1–15 of working trace).
2. Full DDRC/PHY programming in **DDR** phase starting `0xF8006000`.
3. **Poll** `0xF8000B74` mask `0x00002000` (DCI/status, UG585 DDRIOB).
4. DDRC enable `0x00000085`.
5. **Poll** `0xF8006054` mask `0x00000007` (DDRC `mode_sts`, UG585).

Prerequisite: PLL + clocks complete. Working `ddr_smoke` at `0x00100000` and `0x00500000`, pattern `0xA5A55A5A`.

## Polls and timeouts

| Poll | Addr | Mask | Working timeout | Citation |
| --- | --- | --- | --- | --- |
| ARM/DDR/IO PLL lock | `0xF800010C` | `1` / `2` / `4` | `100_000_000` MMIO iterations | `stage1/mmio.rs` `POLL_SPINS`; `ps7_init.rs` `MASK_POLL_TIME`; AMD `ps7_init.c` `PS7_MASK_POLL_TIME` |
| DDRIOB DCI | `0xF8000B74` | `0x00002000` | same | same |
| DDRC ready | `0xF8006054` | `0x00000007` | same | same |
| GPIO settle | MASKDELAY 1 ms on SCU timer `0xF8F00200` | — | `cycles = APU_FREQ_HZ × ms / 2000` | working `delay_ms` / `ps7_init.rs` `cycles_for_ms`; UG585 CPU_3x2x = APU/2 |

**Correction (timeout implementation, not register values):** after APU is at 766.667 MHz, non-PLL polls wait on the SCU global timer with `timeout_ticks = 50_000_000` = `POLL_SPINS / 2` (timer Hz = APU/2, treating `POLL_SPINS` as CPU-cycle budget). PLL lock polls keep `100_000_000` MMIO iterations because they run while ARM PLL may still be bypassed (PS_CLK 33.333333 MHz). 1 ms MASKDELAY unchanged.

## COM12 elapsed times

System 1 lock: gold COM12 `…174430.txt` is FSBL-silent then Utility splash (kernel `0.004s`…`0.127s`). **No** `GP STAGE1` / PS7 elapsed timestamps exist in this repository. System 3 does **not** invent Stage 1 PS7 wall times. After owner authorization, measure on COM12 against this exact artifact.

## Reviewed trace corrections vs locked working JSONL

| ID | Old | New | Reason | Document |
| --- | --- | --- | --- | --- |
| UART-early | UART MASKWRITEs after DDR, inside PERIPHERALS | Same four UART MASKWRITEs after CLOCK, before DDR | Required banners `UART EARLY PASS` / `DDR BEGIN` need a live UART clock+regs | UG585 UART; working UART register values unchanged |
| SCTLR | Committed `zynq_stage1` has **no** `start.S` | `_start` clears SCTLR M, C, I | MMU/caches off before DDR (A9 TRM SCTLR; BootROM OCM) | ARM Cortex-A9 TRM SCTLR |
| Poll timer | spin `100_000_000` on every poll | PLL: same spins; DDR polls: SCU timer `50_000_000` ticks | Clock-derived budget from `POLL_SPINS` and `APU_FREQ_HZ` | working `mmio.rs` + UG585 SCU timer |
| WDT | stop before bringup, no UART line | stop first; print `WDT STOP PASS` after UART | Required UART list | working `wdt::stop`; UG585 SWDT `0xF8005000` |
| Hold | continue into SD/PCAP | print `SYS03 HOLD` and WFI | System 3 stop gate | this document |

All other 209 working MMIO ops keep address, mask, value, and relative order (DDRC/DDRIOB as above).

## Required UART (owned entry)

```
GP-S1 BEGIN
UART EARLY PASS
PS7 CLOCK PASS
DDR BEGIN
DDR PASS
WDT STOP PASS
BOOTMODE SD PASS
SYS03 HOLD
```

Boot mode: `SLCR BOOT_MODE_USER` `0xF800025C` bits `[2:0] == 0b101` (working `boot_mode.rs`; UG585; PZ7020 SD jumper).

## Artifact hashes

Filled at commit time in the commit message and `docs/boot/SYS03_ARTIFACT_HASHES.txt`.
