<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# LightSpeed Studio LLC — Stage 1 boot performance

No invented ranges. Capture timeout (180 s) is **not** boot duration.

## System 12 host wall-clock (only owned `gpos>` run with meta)

Source: `fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS12_20260820/com12_sys12.meta.txt` (host script, not UART bytes). UART SHA-256 `6236336b06b9b4cb70fc098f8616bb91b7b790864dd53e0bc5d73b90bb5cf9d5` (7010). BOOT SHA-256 `738c6a0155d9558c37d51fffe11bc99236c4c29242caa07fa860756f47d24352` (6016212).

Clock origin: `SerialPort.Open` on COM12, then operator RESET.

| Interval | ms | Notes |
| --- | ---: | --- |
| Open → first Stage 1 byte | 17852 | Operator RESET delay. **Not** firmware. |
| First byte → `JUMP 00100060` | 2901 | PS7+SD+parse+PCAP+handoff **combined**. |
| `JUMP 00100060` → `gpos>` | 567 | Includes Utility splash UART. |
| First byte → `gpos>` | 3468 | Observed owned recovery boot after first UART byte. |
| Open → close | 21530 | Includes RESET wait. |

| Phase asked | Result |
| --- | --- |
| RESET/capture start to first Stage 1 byte | 17852 ms (RESET wait) |
| Stage 1 start to SD initialization | **NOT_MEASURED** (no per-line host timestamps) |
| SD load duration | **NOT_MEASURED** |
| Parser/security duration | **NOT_MEASURED** (plaintext path has no RSA/AES UART timebase) |
| PCAP duration | **NOT_MEASURED** |
| Handoff duration | **NOT_MEASURED** |
| `JUMP 00100060` to `gpos>` | 567 ms host |
| Total observed boot (first byte → `gpos>`) | 3468 ms |

UART line order is not a timer. Do not infer PCAP ms from `PCAP BEGIN` sitting next to `PL PASS`.

## Utility printed times (comparable across gold / SYS09 / SYS12)

From raw UART, same strings as `SYS01_WORKING_REFERENCE_LOCK.md` and SYS12 `COMPARE.md`. Gold file `bf3ca7940c61e22455276b26d218bb7bd44203859e2362a26e263a152e102800`. SYS09 `391218176cef09b7d642b71f5cca9990dec2d91b95cc73c9879185d9dfa0219d`. SYS12 as above. Each splash timestamp **matched exactly** (delta 0.000s): 0.004, 0.011, 0.018, 0.026, 0.033, 0.041, 0.049, 0.056, 0.068, 0.095, 0.103, 0.118, 0.127 seconds. These are Utility-internal prints after jump, not Stage 1.

## Timed COM12 vs gold vendor FSBL (same script)

Evidence: `fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS12_VS_GOLD_20260820/`. Same COM12 115200 8N1 DTR off RTS off. Host Stopwatch from port open. Gold FSBL prints nothing; first gold UART is Utility.

Owned recapture UART SHA-256 `6236336b06b9b4cb70fc098f8616bb91b7b790864dd53e0bc5d73b90bb5cf9d5` (7010). Gold UART SHA-256 `89bc10a99160353904bc3eb5a087193fb4ef939e126768a5a92a691797d70ade` (6720).

| Event (ms from COM12 open) | Owned SYS12 | Gold FSBL |
| --- | ---: | ---: |
| first UART byte | 3235 | 1280 |
| `MMU: CPU0 identity` | 6152 | 1284 |
| `JUMP 00100060` | 6152 | none |
| `gpos>` | 6668 | 1805 |

| Interval | Owned | Gold | Meaning |
| --- | ---: | ---: | --- |
| port open → Utility | 6152 (`JUMP`) | 1280 (first byte) | Includes operator RESET wait. Not FSBL-only. |
| first byte → `JUMP` | 2917 | NOT_MEASURED | Owned Stage 1 visible duration. Gold FSBL is silent. |
| Utility → `gpos>` | 516 (`JUMP`→gpos) | 521 (`MMU`→gpos) | Utility after handoff. |

Do not treat gold 1280 ms as FSBL duration. Do not treat the first SYS12 meta 17852 ms as firmware.

## Comparison methodology

| Other run | Host first-byte / JUMP / gpos meta | Stage 1 wall-clock vs SYS12 |
| --- | --- | --- |
| Gold FSBL (this timed pair) | first_byte 1280 / no JUMP / gpos 1805 | FSBL duration **NOT_MEASURED** (silent). Utility after `MMU` is 521 ms. |
| SYS09 `gpos>` | none | **NOT_MEASURED** |
| SYS10A product `gpos>` | none | **NOT_MEASURED** |
| SYS03–SYS08 HOLD | none | not a `gpos>` boot |

Supported **qualitative** reasons owned Stage 1 is visible on UART (code + UART, not timed gold-FSBL splits):

- ADMA2 + CMD18 full-file load vs shortcut PIO CMD17 (SYS04 vs SYS01 shortcut);
- single ADMA of the FAT `BOOT.BIN` (SYS12 `bytes=6016212`) then in-place parse (SYS10A hang proved 16 KiB stack copy is fatal);
- hardware PCAP of frozen PATH1_G64, not a second software bit-walk;
- no repeated software hash/memory pass on the plaintext recovery path (`DISPATCH PLAIN`, no RSA UART);
- direct Utility handoff (`HANDOFF PASS` / `JUMP 00100060`) after `wait_tx_empty`.

Do not treat 17852 ms as slow firmware. Do not treat 180000 ms timeout as boot time.
