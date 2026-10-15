<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# LightSpeed Studio LLC — Stage 1 executive summary

> LightSpeed Studio LLC has independently implemented and proven an owned post-ROM boot stack for the AMD Zynq-7000 PZ7020 platform. The loader initializes the processing system, loads storage through DMA, validates the boot image, configures the FPGA, loads Golden Phase Utility OS and transfers execution to the kernel without using the vendor `zynq_fsbl.elf`.

This is not a retraction of Golden Phase Utility OS. It is the development-hardware close-out: Systems 1–12 are evidenced; Systems 13–15 are deferred to custom production hardware ([`DEFERRED_PRODUCTION_SECURITY_HARDWARE.md`](DEFERRED_PRODUCTION_SECURITY_HARDWARE.md)).

Legal name used in this set: **LightSpeed Studio LLC** (repository SPDX / copyright headers). No other company spelling is used here.

## What was built

An owned Zynq-7000 Stage 1 in `zynq_stage1/owned` with **zero package `[dependencies]`**. BootROM still runs first (AMD, immutable). After BootROM, the owned ELF — not quarantined `zynq_fsbl.elf` SHA-256 `8b71aa707fd9e200a89cd2b966cf3d3ec26ed0350495718a51a9f8ec259f7874` — performs PS7/UART/DDR, SD ADMA2+CMD18, image parse, optional software RSA or DevCfg AES/HMAC KAT, PATH1_G64 PCAP, Utility copy/zero, and `JUMP 00100060`.

Current development-hardware freeze: plaintext recovery `BOOT.BIN` SHA-256 `738c6a0155d9558c37d51fffe11bc99236c4c29242caa07fa860756f47d24352` (6016212 bytes) at `fpga/zynq/release/PZ7020_STAGE1_PLAINTEXT/BOOT.BIN`. COM12 UART `6236336b06b9b4cb70fc098f8616bb91b7b790864dd53e0bc5d73b90bb5cf9d5` (7010 bytes) contains `PL PASS`, `HANDOFF PASS`, `JUMP 00100060`, and exact `gpos>`. Evidence commits `53100ef` and `33ef254`.

## Why replacing `zynq_fsbl.elf` matters

The quarantined vendor FSBL is a black-box binary. LightSpeed Studio LLC does not own it, does not copy its C, and refuses it by filename and hash. An owned loader is auditable, can print a fail-closed UART contract, and can take plaintext recovery, software-RSA product, or powered-session AES/HMAC KAT paths from one ELF (System 10A dispatcher). The gold FSBL image `7da748547d5016191643ddc4df555a8cf02c1d8adee3235f1dde766166ea0b85` remains in-tree as comparison, not as the owned product boot.

## What is owned and controlled

Owned: Stage 1 Rust/ASM, packer/parser, SD ADMA driver, DevCfg driver, software RSA-2048, SHA-256, PCAP/handoff, UART classifier, evidence scripts. FPGA bitstream `system.bit` SHA-256 `d19f08d20cac85ea0e33c41a8a552166f5a9176e16977f09aa64d17e68135829` and Utility ELF `447f9fbdec6bbdb03227c8620baa854f3a141d52ec71452251e2f63e1852eaf8` are the locked PATH1_G64 pair packed into the owned `BOOT.BIN`. See [`LIGHTSPEED_STAGE1_OWNERSHIP.md`](LIGHTSPEED_STAGE1_OWNERSHIP.md).

Not owned: AMD BootROM and silicon AES/HMAC engines, ARM architecture, Vivado/Vitis, Rust `core` / `compiler_builtins`.

## Physical PZ7020 proof

Board PZ7020-SL-C, UART **COM12** 115200 8N1 DTR off RTS off. Raw UART is board proof. Host banners (`SAW_GPOS`, `=== COM12`) are not UART. Final owned recovery boot (System 12) and unified product boot (System 10A) both print `PL PASS` → `HANDOFF PASS` → `JUMP 00100060` → `gpos>`. Index: [`LIGHTSPEED_STAGE1_EVIDENCE_INDEX.md`](LIGHTSPEED_STAGE1_EVIDENCE_INDEX.md).

## Security capabilities (honest)

Implemented and, where noted, board- or session-proven: software RSA later-partition verify (SYS10A UART `implementation=SOFTWARE`; BootROM line `NOT_ENFORCED`); powered-session BBRAM AES/HMAC P1 KAT (`SYS06_P1_KAT_COM12_PASS`); owned encrypt packer (host); fail-closed UART classifier (SYS11, file-only).

Not claimed: BootROM RSA eFuse enforcement; production PPK hash; permanent AES key; VCCBATT retention; production provisioning. Systems 13–15: `DEFERRED_CUSTOM_HARDWARE`.

## Measured boot performance

The only host wall-clock on an owned `gpos>` boot is System 12 `com12_sys12.meta.txt`: first UART byte 17852 ms after port open (operator RESET delay), `JUMP 00100060` at 20753 ms, `gpos>` at 21320 ms. Firmware-relevant interval first-byte → `gpos>` = **3468 ms**. Per-phase SD/parser/PCAP/handoff splits are **NOT_MEASURED**. Gold FSBL UART has no host timestamps; Stage 1 duration vs gold is **NOT_MEASURED**. Utility splash printed times match gold/SYS09 exactly (0.004s … 0.127s). Full method: [`LIGHTSPEED_STAGE1_BOOT_PERFORMANCE.md`](LIGHTSPEED_STAGE1_BOOT_PERFORMANCE.md).

## Current product status

> Golden Phase Utility OS and its owned PZ7020 post-ROM boot path are complete for the current development-hardware scope. Final secure-production key provisioning is intentionally deferred until custom hardware supplies the selected retention, secure-element and recovery infrastructure.

Stage 1 loaded-file size is **29836** bytes (`SYS10_OVERSIZE` vs 8192). Security features were not deleted to hit 8 KiB. [`LIGHTSPEED_STAGE1_RELEASE_STATUS.md`](LIGHTSPEED_STAGE1_RELEASE_STATUS.md).

## Honest limitations

- 8 KiB OCM estimate was not met.
- PZ-StarLite has no proven battery-backed BBRAM retention.
- Research BBRAM keys are not production keys.
- No universal x86/ARM/RISC-V Stage 1 port is claimed here.
- Shortcut `zynq_stage1/src/main.rs` remains a behavioral reference and is not the board-proven owned ELF.

## Custom-hardware roadmap

Systems 13–15 reopen only when the schematic, key-custody procedure, sacrificial production-revision board, and signed recovery exist. See [`DEFERRED_PRODUCTION_SECURITY_HARDWARE.md`](DEFERRED_PRODUCTION_SECURITY_HARDWARE.md).
