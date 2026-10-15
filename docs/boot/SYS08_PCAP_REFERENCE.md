<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# System 8 — plaintext PCAP / FabricInit

Status: **SYS08_COM12_PASS**

COM12 PASS on CRC-fix artifact `8d0aefa6f5aef206a1eec6617f988a14aa3ea07733f167c3c51620159f201ef7` (6004948 bytes), source `b9e34b4`. UART `com12_sys08_crcfix.uart` SHA-256 `75ee7277e6d554c47ff2f559d6af0752f0e8b816339752e137b5504053d768a7` (257 bytes): `PCAP BEGIN` → `PL PASS` → `SYS08 HOLD`. No `PL FAIL`.

Starts from System 7 COM12 PASS evidence `e0614878d1064a06042899f499f88637d0c2c7e2`. Diagnostic COM12 (`f76d981` artifact) named `PL FAIL class=CRC`: DMA finished, `PCFG_DONE` stayed 0, first DDR bytes `aa995566`, LE word `0x665599AA`. UART `com12_sys08_diag.uart` SHA-256 `75c20baa18a495fac3e1280b0c30faa51f8f667e41dc351c78d84b6f6060017d` (557 bytes). Working Stage 1 (`zynq_stage1/src/main.rs`, hash `6637678d0e01b007caf3b5e150ed37e48ed5af6f55cc009a3b16e0d5c4ebd67d`, 9580 bytes) is not edited.

This revision converts each 32-bit configuration word in the aligned `BIT_SCRATCH` buffer into PCAP byte order. No Utility jump. No encrypted P2. No eFuse. No BBRAM. No System 9. 6B `secure_transfer` stays the encrypted path (`ImageWordLen > DataWordLen`). MMIO sequence is unchanged.

Inspected as behavioral reference only. No vendor C was copied into `zynq_stage1/owned`.

## What was opened

| Artifact | Path | SHA-256 | Bytes |
| --- | --- | --- | ---: |
| Working PCAP | `src/board/zynq7000/stage1/pcap.rs` | `1cd443ccabe32c731e53f77d2e9cd9ed921eafa6dae60b3e41506cd1fe5c255c` | 5995 |
| Working MMIO / SLCR | `src/board/zynq7000/stage1/mmio.rs` | `1f9c04dda9e28ba9b1cf53c6afbf342937159993b6eb213e9993be2e3483b17b` | 2042 |
| Working `.bit` strip | `src/board/zynq_bootimg.rs` `strip_bit_header` | `c96b4b13ac7b884b266c6f90b55bfd0facd2a28105446af2d1dd207e4e6ce999` | 11329 |
| Working Stage 1 | `zynq_stage1/src/main.rs` | `6637678d0e01b007caf3b5e150ed37e48ed5af6f55cc009a3b16e0d5c4ebd67d` | 9580 |
| PATH1_G64 `system.bit` | `fpga/zynq/release/PZ7020_G1P_PATH1_G64/system.bit` | `d19f08d20cac85ea0e33c41a8a552166f5a9176e16977f09aa64d17e68135829` | 4045769 |
| FSBL PCAP | `D:\Xilinx\2026.1\Vitis\data\embeddedsw\lib\sw_apps\zynq_fsbl\src\pcap.c` | `26fd6a726aef9fc13e4ea5c70d08165e1a2a6e9ff5ae8b6ce1d7d25fb9915c35` | 21508 |
| FSBL PCAP header | `...\pcap.h` | `b2756f173f842e62ceadbbc8eb7300bfb4d9f9553f2e1577a382180f7673d119` | 2733 |
| DevCfg driver | `...\devcfg_v3_9\src\xdevcfg.c` | `026e265e74d5330f608278cdd3029acc9b7ed12ccbc2a483466a95ba04e28bf7` | 28526 |

UG585 (no local PDF hash): [PCAP bridge](https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/PCAP-Bridge-to-PL), DevCfg register map (base `0xF8007000`), SLCR `FPGA_RST_CTRL` `0xF8000240`, `LVL_SHFTR_EN` `0xF8000900`.

Inspected functions (names recorded for provenance): `FabricInit`, `PcapLoadPartition`, `XDcfg_Transfer` (`XDCFG_NON_SECURE_PCAP_WRITE`), `program`, `post_config`, `strip_bit_header`.

## Board-proven FabricInit (owned working `pcap.rs`)

SLCR unlock key `0xDF0D` at `0xF8000008`, lock key `0x767B` at `0xF8000004` (same as System 3 `ops_data`).

1. DevCfg UNLOCK `0x757BDF0D`. Prove CTRL is writable by toggling `PCAP_RATE_EN` (bit 25) and restoring.
2. MCTRL `PCFG_POR_B` bit 8 (`0x00000100`) must be set (FSBL `pcap.h`; working `pcap.rs` omits this check; owned System 8 refuses if clear).
3. CTRL: set `PCAP_PR` (bit 27) and `PCAP_MODE` (bit 26); **clear** `PCAP_RATE_EN`.
4. SLCR unlock. `FPGA_RST_CTRL = 0xF`. `LVL_SHFTR_EN = 0`. SLCR lock.
5. Pulse `PCFG_PROG_B` high then low (FSBL `FabricInit`; working `pcap.rs` only drives low and relies on BootROM having left PROG_B high). Delay 5 ms. If `STATUS.PCFG_INIT` still set, delay 5 ms more. Still set → fail.
6. Set `PCFG_PROG_B`. Delay 5 ms. Poll `PCFG_INIT` set.

FSBL `FabricInit` also writes `LVL_SHFTR_EN = 0xA` (`LVL_PS_PL`) first and uses a 30 ms INIT-clear timeout with a full PROG_B retry. It does not assert `FPGA_RST_CTRL` (comment in `pcap.c`: define removed). Owned SLCR holds (`FPGA_RST_CTRL = 0xF`, shifters 0) come from working Stage 1. Owned PROG_B pulse is high→low→high so INIT actually falls when PROG_B was already low.

AES-eFuse 5 ms waits in FSBL are not taken: eFuse is not programmed.

## Non-secure PCAP write (plaintext bitstream)

`XDcfg_Transfer` `NON_SECURE_PCAP_WRITE`: clear MCTRL loopback; **clear** CTRL quarter-rate; DMA dest `0xFFFFFFFF`; source and dest word counts **equal**. Last-command: OR `1` into source; dest `0xFFFFFFFF | 1` stays `0xFFFFFFFF`.

Then poll INT_STS `DMA_DONE` then `PCFG_DONE`. INT_STS is W1C. If `STATUS.DMA_DONE_CNT` is nonzero, clear it through STATUS (System 6B rule). Caches off. Physical RAM source. DMA source 4-byte aligned.

## `.bit` header

Working `strip_bit_header` finds the first `AA 99 55 66` (UG470 sync) and keeps that suffix. PATH1_G64 `system.bit` sync offset is **253** (not 32-bit aligned), remaining **4045516** bytes. Device copies the suffix to `0x01000000` (`BIT_SCRATCH` in working `main.rs`) before DMA. `BIT_MAX = 0x00800000`.

## Source-word representation (CRC fix)

UG470 stores the sync as file bytes `AA 99 55 66` (big-endian word `0xAA995566`). Diagnostic COM12 proved the aligned DMA buffer held those file bytes, so the ARM LE word was `0x665599AA`. Gold Bootgen P1 (`fpga/zynq/release/PZ7020_G1P_PATH1_G64/BOOT.BIN` offset `0x20E40+48`) stores the same sync as `66 55 99 AA` (LE word `0xAA995566`). Working `pcap.rs` DMA's a Bootgen partition (already in that order) or a suffix that `strip_bit_header` did not rewrite. Owned raw `.bit` partitions are converted in `BIT_SCRATCH` by `pcap_word_order` (each 4-byte group reversed) before DMA.

## MMIO sequence vs FSBL / UG585 / working Stage 1

UG585 DevCfg: UNLOCK `0x757BDF0D`, DMA dest `0xFFFFFFFF` for PCAP, INT_STS W1C, `STATUS.PCFG_INIT` bit 4, `INT_STS.DMA_DONE` bit 13, `INT_STS.PCFG_DONE` bit 2. SLCR `FPGA_RST_CTRL` `0xF8000240`, `LVL_SHFTR_EN` `0xF8000900`.

| Step | Owned System 8 | FSBL `NON_SECURE_PCAP_WRITE` / `FabricInit` | Working `pcap.rs` |
| --- | --- | --- | --- |
| UNLOCK + CTRL writable | Yes (toggle `PCAP_RATE_EN`) | Yes | Yes |
| `MCTRL.PCFG_POR_B` | Refuse `FABRIC_POWER` if clear | Refuse if clear | Not checked |
| `PCAP_PR` + `PCAP_MODE`, rate off | Yes | Yes | Yes |
| `FPGA_RST_CTRL=0xF`, shifters 0 during init | Yes | Shifters `0xA` first; no FPGA_RST write | Same SLCR holds as owned |
| PROG_B | High → low → high | High → low → high | Drive low only |
| Transfer loopback | Clear | Clear | Not written |
| Transfer `PCAP_RATE_EN` | Clear | Clear | Already clear from init |
| DMA dest | `0xFFFFFFFF` | `0xFFFFFFFF` | `0xFFFFFFFF` |
| Word counts | Equal | Equal | Equal |
| Source last-command | `src \| 1` | `src \| 1` | `src \| 1` |
| Poll | `DMA_DONE` then `PCFG_DONE` | `DMA_DONE` then `PCFG_DONE` | Same |

Owned extra vs working: POR_B, loopback clear, PROG_B high-before-low. Those are diagnostic observations, not this-commit fixes.

## Device UART (contiguous after the System 3/4/5 SD lines)

PASS (unchanged):

```
IMAGE HEADER PASS
PARTITIONS count=3
PCAP BEGIN
PL PASS
SYS08 HOLD
```

On FabricInit or DMA/`PCFG_DONE` failure, extra diagnostic lines then typed class. Hex is 8 lowercase digits with no `0x`. Word counts are decimal. No `SYS08 HOLD`. No `STAGE1: JUMP`.

```
PCAP SRC addr=01000000 words=<dec>
PCAP SRC bytes=665599aa
PCAP SRC word=aa995566
PCAP INIT=<0|1>
PCAP CTRL=<8hex> STATUS=<8hex> MCTRL=<8hex>
PCAP RST=<8hex> LVL=<8hex>
PCAP INT=<8hex> STATUS=<8hex>
PCAP DMA src=<8hex> dst=<8hex> slen=<8hex> dlen=<8hex>
PCAP INIT=<0|1> DONE=<0|1>
PL FAIL class=<TOKEN>
```

`SRC bytes` is the first four DDR bytes after `pcap_word_order`. `SRC word` is the little-endian 32-bit load of those bytes (PCAP `0xAA995566` when conversion ran). Pre-DMA lines are CTRL/STATUS/MCTRL, `PCFG_INIT`, `FPGA_RST_CTRL`, `LVL_SHFTR_EN`. Post-DMA lines are INT_STS, STATUS, DMA address/length registers, `PCFG_INIT` and `PCFG_DONE`.

Typed tokens: `UNALIGNED`, `ZERO_LENGTH`, `LENGTH`, `NOT_ENCRYPTED`, `DEST`, `SRC`, `CACHES`, `FABRIC_PREP`, `PCFG_INIT`, `DMA_QUEUE`, `DMA_STUCK`, `TIMEOUT`, `HMAC`, `AXI_WTO`, `AXI_WERR`, `AXI_RTO`, `AXI_RERR`, `RX_OV`, `DMA_CMD`, `DMA_Q_OV`, `P2D`, `UNLOCK`, `FABRIC_POWER`, `INITB`, `CRC`, plus parse-path `BOUNDS` / `PARSE` / `ADMA` / `PIO` / `BITSTREAM`. `SEU` ISR is reported as `CRC`. DMA_DONE without `PCFG_DONE` is `CRC`, not `TIMEOUT`.

Partition 2 (Utility) is parsed and not executed.

Silicon PASS image: `zynq_stage1/owned/build/BOOT_SYS08_PCAP.BIN` SHA-256 `8d0aefa6f5aef206a1eec6617f988a14aa3ea07733f167c3c51620159f201ef7` (6004948 bytes), on-card match, source `b9e34b4`. P0 unsigned `sys08_plat` PT_LOAD 18388 bytes. Diagnostic FAIL image was `e54fa9f5647cad3562772e6fb409811b7dda5e5023f21a16244d049482c23f55` from `f76d981`. P1 PATH1_G64 `system.bit`. P2 Utility parsed, not jumped.

## Non-claims

- Not Utility handoff / `0x00100060`
- Not encrypted bitstream / 6B secure PL
- Not eFuse, not BBRAM, not BootROM RSA
- Not System 9
