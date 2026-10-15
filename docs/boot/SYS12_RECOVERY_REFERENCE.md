<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# System 12 — plaintext recovery vs gold

Status: **SYS12_COM12_PASS**

Plaintext recovery image `BOOT_SYS10A_RECOVERY.BIN` SHA-256 `738c6a0155d9558c37d51fffe11bc99236c4c29242caa07fa860756f47d24352` (6016212) on the SYS04 card `121220160204` (H:, FAT32). Source `a39ac187aca0c0988131dc03dd36b352deb48d68`. COM12 UART `com12_sys12.uart` SHA-256 `6236336b06b9b4cb70fc098f8616bb91b7b790864dd53e0bc5d73b90bb5cf9d5` (7010): `PL PASS` → `HANDOFF PASS` → `JUMP 00100060` → `gpos>`. sys11 classifies **Recovery**, not Product, not Aes.

Gold Utility splash times and PL identity lines matched the locked gold/SYS09 UART exactly. No BBRAM. No eFuse. No AES-image flash. Journal partition not written.

Frozen plaintext boot (identical bytes): `fpga/zynq/release/PZ7020_STAGE1_PLAINTEXT/BOOT.BIN`. Evidence backup: `fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS12_20260820/BOOT_SYS12_PLAINTEXT.BIN`. SYS03 `BOOT_RECOVERY_PLAINTEXT.BIN` and gold FSBL `PZ7020_G1P_PATH1_G64/BOOT.BIN` were not overwritten.

Evidence: `fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS12_20260820/`. Timed COM12 vs gold FSBL: `fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS12_VS_GOLD_20260820/`.

Systems 13–15 are not started. Production key provisioning is deferred: [`DEFERRED_PRODUCTION_SECURITY_HARDWARE.md`](DEFERRED_PRODUCTION_SECURITY_HARDWARE.md). Summaries: [`README.md`](README.md).
