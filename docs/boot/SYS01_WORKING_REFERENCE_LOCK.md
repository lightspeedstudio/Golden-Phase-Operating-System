<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# System 1 — working PZ7020 firmware reference lock

Locked at tree `5d1027bcef372ee2fa5f85392ecc90685061056d` (`feat(boot): land owned Zynq Stage 1 (BootROM runs our ELF)`), branch `feat/stage1-sys01-reference-v3`.

SHA-256 values below are lowercase hex of the bytes on disk in `C:\k-mmu-stage1` at lock time. Lengths are file size in bytes.

**Policy.** The owned shortcut firmware (`zynq_stage1/src/main.rs` plus the kairos `stage1` / SD / boot-image modules it calls) is the **source-level behavioral reference**. The quarantined vendor `zynq_fsbl.elf` may be used only as a **black-box** timing and UART-output comparison. Do not copy or decompile vendor FSBL source. BootROM and DevCfg AES/HMAC engines remain AMD silicon.

**Not claimed.** This lock is not a COM12 PASS of a replacement Stage 1 binary. No owned Stage 1 ELF in this tree is board-proven. The last board-proven boot image is the gold PATH1_G64 `BOOT.BIN` whose `[bootloader]` partition is the quarantined FSBL.

## Machine-readable table

Paths are relative to the repository root. Tests hash these files and compare.

```lock-v1
zynq_stage1/src/main.rs	6637678d0e01b007caf3b5e150ed37e48ed5af6f55cc009a3b16e0d5c4ebd67d	9580
zynq_stage1/Cargo.toml	fb387c026c9ee74ec5285eac46ea998584edfa77cb08cb435998cc947ac597d7	730
src/board/zynq7000/ps7_init_data.rs	37b5710184351c7aaa13c5fdf485702a83455ad30a07d4203292a323fee3492b	12294
src/board/zynq7000/sd.rs	b30aee86c2c3a35395f611324ff75b62027b00cb59b1141f811f5f40eb706792	13441
src/board/zynq_bootimg.rs	c96b4b13ac7b884b266c6f90b55bfd0facd2a28105446af2d1dd207e4e6ce999	11329
fpga/zynq/release/PZ7020_G1P_PATH1_G64/BOOT.BIN	7da748547d5016191643ddc4df555a8cf02c1d8adee3235f1dde766166ea0b85	6114836
fpga/zynq/release/PZ7020_G1P_BRINGUP_V2/zynq_fsbl.elf	8b71aa707fd9e200a89cd2b966cf3d3ec26ed0350495718a51a9f8ec259f7874	577272
fpga/zynq/evidence/kairos_zynq_boot/com12_path1_g64_combined_20260814_174430.txt	bf3ca7940c61e22455276b26d218bb7bd44203859e2362a26e263a152e102800	7475
fpga/zynq/release/PZ7020_G1P_PATH1_G64/system.bit	d19f08d20cac85ea0e33c41a8a552166f5a9176e16977f09aa64d17e68135829	4045769
fpga/zynq/release/PZ7020_G1P_PATH1_G64/kairos_zynq.elf	447f9fbdec6bbdb03227c8620baa854f3a141d52ec71452251e2f63e1852eaf8	2475164
```

The quarantined FSBL row is recorded so it can be **refused**. It is not an owned Stage 1 source.

## Reference catalog

### 1. Owned Stage 1 entry — `zynq_stage1/src/main.rs`

| Field | Value |
| --- | --- |
| Path | `zynq_stage1/src/main.rs` |
| SHA-256 | `6637678d0e01b007caf3b5e150ed37e48ed5af6f55cc009a3b16e0d5c4ebd67d` |
| Bytes | 9580 |
| Source commit | `5d1027bcef372ee2fa5f85392ecc90685061056d` (introduced `7b7d75490c06d38d9b45c1540f53aaed0db1e846`; current blob is Stage 1 land) |
| Role | Behavioral reference. Do not delete, truncate, rename, or edit this file in System 1. |

### 2. Owned Stage 1 manifest — `zynq_stage1/Cargo.toml`

| Field | Value |
| --- | --- |
| Path | `zynq_stage1/Cargo.toml` |
| SHA-256 | `fb387c026c9ee74ec5285eac46ea998584edfa77cb08cb435998cc947ac597d7` |
| Bytes | 730 |
| Source commit | `5d1027bcef372ee2fa5f85392ecc90685061056d` |
| Note | This binary crate still depends on `kairos`. That dependency is part of the **shortcut** reference, not the isolated provenance library. |

### 3. Generated PS7 streams — `src/board/zynq7000/ps7_init_data.rs`

| Field | Value |
| --- | --- |
| Path | `src/board/zynq7000/ps7_init_data.rs` |
| SHA-256 | `37b5710184351c7aaa13c5fdf485702a83455ad30a07d4203292a323fee3492b` |
| Bytes | 12294 |
| Source commit (current blob) | `fbd10a2321f2990624d8683a1ff942b0691e09b9` |
| Introduced | `7b7d75490c06d38d9b45c1540f53aaed0db1e846` |
| Generator facts (file header) | source `.xsa` SHA-256 `cf4a33161e9ced82be674aecd4f332e00a5b1f279dae8a83ab7193f3ad6996aa`; `ps7_init.c` SHA-256 `61c47f3c57a775af8be8556eed11887325f7f26eb0bde5a65ad9ca95296ed5ff`; encoded-stream SHA-256 `64eb150f0cc19f5fe5c74ac8578489f668583630dadbabbd8f97b4bb209da433` |
| APU_FREQ_HZ | 766666687 |

The shortcut `rust_main` does **not** call this interpreter. It calls `stage1::ps7::bringup`. The generated streams remain the sealed XSA-derived register reference.

### 4. SD host — `src/board/zynq7000/sd.rs`

| Field | Value |
| --- | --- |
| Path | `src/board/zynq7000/sd.rs` |
| SHA-256 | `b30aee86c2c3a35395f611324ff75b62027b00cb59b1141f811f5f40eb706792` |
| Bytes | 13441 |
| Source commit (current blob) | `6f9a2c006f4841a2dfc08cd119fb680ec99bc996` |
| Introduced | `7b7d75490c06d38d9b45c1540f53aaed0db1e846` |

### 5. Boot image parse — `src/board/zynq_bootimg.rs`

| Field | Value |
| --- | --- |
| Path | `src/board/zynq_bootimg.rs` |
| SHA-256 | `c96b4b13ac7b884b266c6f90b55bfd0facd2a28105446af2d1dd207e4e6ce999` |
| Bytes | 11329 |
| Source commit | `5d1027bcef372ee2fa5f85392ecc90685061056d` |

### 6. Last board-proven boot image — gold PATH1_G64 `BOOT.BIN`

| Field | Value |
| --- | --- |
| Path | `fpga/zynq/release/PZ7020_G1P_PATH1_G64/BOOT.BIN` |
| SHA-256 | `7da748547d5016191643ddc4df555a8cf02c1d8adee3235f1dde766166ea0b85` |
| Bytes | 6114836 |
| Source commit | `d55c5ae48e157fafef913d66021bd0acadcf2610` (`feat: gold Utility PL with Path-1 TNSR and G64S on one bitstream`) |
| Pack recipe | `fpga/zynq/release/PZ7020_G1P_PATH1_G64/boot.bif`: `[bootloader]` = quarantined `zynq_fsbl.elf`, then frozen `system.bit`, then Utility `kairos_zynq.elf` |
| Board status | COM12 MATRIX PASS 2026-08-14T17:44 (same boot: G1P + TNSR + G64S) |

### 7. Stage 1 ELF inside that `BOOT.BIN`

| Field | Value |
| --- | --- |
| Path | `fpga/zynq/release/PZ7020_G1P_BRINGUP_V2/zynq_fsbl.elf` |
| SHA-256 | `8b71aa707fd9e200a89cd2b966cf3d3ec26ed0350495718a51a9f8ec259f7874` |
| Bytes | 577272 |
| Source commit | `fbd10a2321f2990624d8683a1ff942b0691e09b9` |
| Status | **Quarantined vendor FSBL.** Black-box comparison only. Same bytes as item 11. |

There is no board-proven owned `zynq_stage1` ELF in this tree (`zynq_stage1/build/zynq_stage1.elf` is absent).

### 8. Raw COM12 UART (gold PATH1_G64 boot)

| Field | Value |
| --- | --- |
| Exact path | `fpga/zynq/evidence/kairos_zynq_boot/com12_path1_g64_combined_20260814_174430.txt` |
| SHA-256 | `bf3ca7940c61e22455276b26d218bb7bd44203859e2362a26e263a152e102800` |
| Bytes | 7475 |
| Source commit | `d55c5ae48e157fafef913d66021bd0acadcf2610` |
| Duplicate (identical SHA-256 and length) | `fpga/zynq/evidence/kairos_zynq_boot/BOOT_PATH1_G64_COMBINED_20260814/com12_path1_g64_combined_20260814_174430.txt` |
| Claim | `fpga/zynq/release/PZ7020_G1P_PATH1_G64/STATUS.md` and `SHA256SUMS.txt`: COM12 MATRIX PASS, same boot |

This capture is the FSBL-then-Utility gold path. It does not contain `GP STAGE1` lines. Vendor FSBL is silent on UART in this file; first application text is `MMU: CPU0 identity...` then splash then `gpos>`.

### 9. Frozen PATH1_G64 bitstream

| Field | Value |
| --- | --- |
| Path | `fpga/zynq/release/PZ7020_G1P_PATH1_G64/system.bit` |
| SHA-256 | `d19f08d20cac85ea0e33c41a8a552166f5a9176e16977f09aa64d17e68135829` |
| Bytes | 4045769 |
| Source commit | `d55c5ae48e157fafef913d66021bd0acadcf2610` |
| Frozen | Yes. Do not rebuild or substitute. G1P+TNSR at `0x40000000`, G64S at `0x40001000`. |

### 10. Utility ELF (`e_entry` 0x00100060)

| Field | Value |
| --- | --- |
| Path | `fpga/zynq/release/PZ7020_G1P_PATH1_G64/kairos_zynq.elf` |
| SHA-256 | `447f9fbdec6bbdb03227c8620baa854f3a141d52ec71452251e2f63e1852eaf8` |
| Bytes | 2475164 |
| Source commit | `d55c5ae48e157fafef913d66021bd0acadcf2610` |
| ELF class | 32-bit little-endian (`EI_CLASS=1`, `EI_DATA=1`) |
| `e_entry` | `0x00100060` (read from ELF header offset 0x18; gold JUMP target, not `0x00100020`) |

### 11. Quarantined vendor FSBL

| Field | Value |
| --- | --- |
| Path | `fpga/zynq/release/PZ7020_G1P_BRINGUP_V2/zynq_fsbl.elf` |
| SHA-256 | `8b71aa707fd9e200a89cd2b966cf3d3ec26ed0350495718a51a9f8ec259f7874` |
| Bytes | 577272 |
| Source commit | `fbd10a2321f2990624d8683a1ff942b0691e09b9` |
| Policy | Refuse by filename `zynq_fsbl.elf` and by this hash. Black-box timing/output only. |

## Source functions that produce PS7, DDR, SD, PCAP, and handoff

Owned shortcut path (`zynq_stage1/src/main.rs` `rust_main`, commit `5d1027bcef372ee2fa5f85392ecc90685061056d`):

| Behavior | Functions / types |
| --- | --- |
| Watchdog | `kairos::board::zynq7000::stage1::wdt::stop` (`src/board/zynq7000/stage1/wdt.rs`) |
| PS7 | `stage1::ps7::bringup` → private `mio`, `pll`, `clocks`, `ddr`, `peripherals` (`src/board/zynq7000/stage1/ps7.rs`, commit `5d1027bcef372ee2fa5f85392ecc90685061056d`). Named UG585 MMIO steps for this PCB. Not Xilinx FSBL `.c`. |
| DDR check | `stage1::ps7::ddr_smoke(0x0010_0000)` and `ddr_smoke(0x0050_0000)` |
| Alternate PS7 (not called by shortcut `main.rs`) | `ps7_init::ps7_init` / `ps7_config` / `ps7_post_config` (`src/board/zynq7000/ps7_init.rs`, commit `7b7d75490c06d38d9b45c1540f53aaed0db1e846`) consuming `PS7_MIO_INIT`, `PS7_PLL_INIT`, `PS7_CLOCK_INIT`, `PS7_DDR_INIT`, `PS7_PERIPHERALS_INIT`, `PS7_POST_CONFIG` |
| Boot mode | `stage1::boot_mode::is_sd` |
| SD | `SdCard::init`, `SdCard::read_block` (SD0 `0xE0100000`, polled PIO CMD17) in `src/board/zynq7000/sd.rs` |
| FAT / `BOOT.BIN` load | `Fat32::mount`, `Fat32::load_root_file` (`src/board/zynq7000/fat.rs`, blob commit `b8fb0dc5f480a9b57ccbf838818bc9d4504afe54`) |
| Image parse | `zynq_bootimg::loader_bitstream_app`, `ps_ddr_partitions`, `partition_bytes`, `strip_bit_header` |
| Auth (shortcut, not gold FSBL) | `zynq_boot_auth::split_signed`; `stage1::auth::{check_app,ddr_span,check_pcap_src,verify_measure,root_hex}`; `stage1::measure::{measure,store}` |
| PCAP | `stage1::pcap::{silicon_version,program,post_config,lock_reg}` (`src/board/zynq7000/stage1/pcap.rs`) |
| Handoff | copy PS DDR partitions to `load_addr`; UART `STAGE1: JUMP` + `app_hdr.exec_addr`; `stage1::handoff::jump` (`src/board/zynq7000/stage1/handoff.rs`) |

Gold board-proven path (black box): quarantined `zynq_fsbl.elf` performs PS7, DDR, SD/QSPI boot-image walk, PCAP of frozen `system.bit`, and jump to Utility `0x00100060`. Do not treat its instructions as source.

## Current measured UART timings

Source: exact COM12 file in item 8. Vendor FSBL emits no timestamps in that capture.

Kernel splash timestamps in that file (seconds after Utility start, as printed):

| Line | Time |
| --- | --- |
| stateclock: chronos + kairos clocks | 0.004s |
| gphase_core: compose / reflect algebra | 0.011s |
| scheduler: Phi0-Phi3 phase graph | 0.018s |
| memory: 4 tagged pages mapped | 0.026s |
| syscall: user/kernel boundary armed | 0.033s |
| phasefs: /tmp mounted (2 inodes) | 0.041s |
| vm: RISC-V ISA loader ready | 0.049s |
| trace: kernel event ring (4 records) | 0.056s |
| statechain: merkle ledger online (tick 3) | 0.068s |
| init: pid 0 scheduled (1 proc) | 0.095s |
| timer: preempt + chronos tick | 0.103s |
| phasefs.journal: phase records read-only | 0.118s |
| stateclock.atlas: 10-sector map (sec 5, k=4) | 0.127s |

Owned shortcut Stage 1 (`GP STAGE1` / `STAGE1: HASH` / `STAGE1: PCAP`) has **no raw COM12 file in this repository** at lock time. Those PIO/PhaseHash/PCAP splits are therefore **not re-measured here**.

## Exact board

| Item | Value |
| --- | --- |
| Board | PZ7020-SL-C (Zynq-7020, Cortex-A9) |
| Console | COM12 CH340, 115200 8N1, DTR off, RTS off |
| PS_CLK / crystal | 33.333333 MHz (`PCW_CRYSTAL_PERIPHERAL_FREQMHZ` in `fpga/zynq/release/PZ7020_G1P_PATH1_G64/ps7_config_dump.txt`) |
| APU | 767 MHz nominal (`PCW_APU_PERIPHERAL_FREQMHZ=767`); generated `APU_FREQ_HZ = 766666687` |
| DDR part | MT41K256M16 RE-125 (`PCW_UIPARAM_DDR_PARTNO`), DDR3 LV, 16-bit, 533.333333 MHz, `PCW_DDR_RAM_HIGHADDR=0x1FFFFFFF` |
| SDIO instance | SD0, controller `0xE0100000` (`src/board/zynq7000/sd.rs`); `PCW_SD0_PERIPHERAL_ENABLE=1` |
| SD MIO | MIO 40 .. 45 (`PCW_SD0_SD0_IO`); card detect not wired (`SD0_HAS_CARD_DETECT = false`) |
| UART0 | MIO 10 .. 11 |
| FCLK0 | 100 MHz |

## Frozen bitstream hash

`d19f08d20cac85ea0e33c41a8a552166f5a9176e16977f09aa64d17e68135829` — `fpga/zynq/release/PZ7020_G1P_PATH1_G64/system.bit`

## Utility entry

`0x00100060`

## Quarantined FSBL hash

`8b71aa707fd9e200a89cd2b966cf3d3ec26ed0350495718a51a9f8ec259f7874`
