<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# Golden Phase production-security deferral

Systems 13, 14 and 15 are **not started**. This file records a deferral to custom Golden Phase hardware. It does not generate keys, program BBRAM or eFuses, write an SD card, open COM12, or change boot code.

Baseline: committed System 12 PASS evidence `53100ef` (COM12 UART) and freeze `33ef254` (plaintext `BOOT.BIN`) on `feat/stage1-sys12-evidence-v3`.

## Current status

> Golden Phase Utility OS is complete for the current PZ7020 development scope. The owned post-ROM Stage 1, FPGA configuration, kernel, storage, security subsystems and Utility OS boot successfully to `gpos>` on physical silicon. Final secure-production provisioning is intentionally deferred until custom Golden Phase hardware provides the required key-retention, recovery and provisioning infrastructure.

This is not a retraction of the proven OS or bootloader. It separates completed software from irreversible production provisioning.

## What is already proven

Existing committed evidence only. No new silicon claims.

| Proven item | Existing record |
| --- | --- |
| Owned Stage 1 replaces `zynq_fsbl.elf` on the owned PASS images | SYS09/SYS10A/SYS12 COM12 PASS loaders are `sys09_plat` / `sys10a_plat`. `zynq_fsbl.elf` hash `8b71aa70…259f7874` remains quarantined (`docs/boot/SYS01_WORKING_REFERENCE_LOCK.md`). Gold FSBL `BOOT.BIN` `7da74854…` is comparison-only and was not overwritten. |
| PS7 / UART / DDR initialization | SYS03 `docs/boot/SYS03_PS7_REFERENCE.md`; SYS12 UART `GP-S1 BEGIN` … `DDR PASS` … `UART EARLY PASS`. |
| SD ADMA2 / CMD18 loading | SYS04 `docs/boot/SYS04_SD_REFERENCE.md`; SYS12 `SD ADMA2 CMD18 PASS bytes=6016212`. |
| Owned boot parser and dispatcher | SYS05 `docs/boot/SYS05_PARSER_REFERENCE.md`; SYS10A `parse_all_product` / `DISPATCH RSA` or `DISPATCH PLAIN`. |
| Owned AES/HMAC secure-image packer | Host packer `docs/boot/SYS06_PACKER_REFERENCE.md`. Keys stay gitignored. |
| Owned Rust DevCfg secure-transfer driver | `docs/boot/SYS06B_DEVCFG_REFERENCE.md`; device path `p1_decrypt_kat` → `secure_transfer`. |
| BBRAM AES/HMAC KAT PASS during the powered research session | SYS06 `SYS06_P1_KAT_COM12_PASS`, `docs/boot/SYS06_P1_DEVCFG.md`, evidence `BOOT_STAGE1_SYS06_20260820/`. Powered session only. |
| Owned RSA-2048 software verification | SYS07 `docs/boot/SYS07_RSA_REFERENCE.md` (later partitions, software). SYS10A unified UART: `STAGE1 RSA2048 PARTITION PASS implementation=SOFTWARE`. |
| PCAP PATH1_G64 configuration | SYS08 `docs/boot/SYS08_PCAP_REFERENCE.md`; SYS09/SYS12 `PCAP BEGIN` → `PL PASS`. Frozen bit `d19f08d2…68135829`. |
| Utility load and `JUMP 00100060` | SYS09 `docs/boot/SYS09_HANDOFF_REFERENCE.md`; SYS12 `JUMP 00100060`. Not `JUMP 00100020`. |
| Physical `gpos>` boot | SYS09 UART `39121817…dfa0219d`; SYS10A product UART `ceb02585…3294c64d`; SYS12 recovery UART `6236336b…bb5cf9d5`. |
| Unified product path | SYS10A `BOOT_SYS10A_UNIFIED.BIN` `8b396c6f…577874` (6018260), `docs/boot/STAGE1_SIZE_REPORT.md`, evidence `BOOT_STAGE1_SYS10A_20260820/`. |
| System 10 honest `SYS10_OVERSIZE` | `docs/boot/STAGE1_SIZE_REPORT.md` status **SYS10_OVERSIZE**; PT_LOAD filesz 29836 vs 8192. Commit `3a8049b`. |
| System 11 fail-closed UART classifier | `docs/boot/SYS11_UART_REFERENCE.md`; `zynq_stage1/owned/src/sys11.rs`; commit `a39ac18`. File-only; never opens a COM port. |
| System 12 recovery comparison | `docs/boot/SYS12_RECOVERY_REFERENCE.md`; UART/classifier/compare `BOOT_STAGE1_SYS12_20260820/`; commits `53100ef`, `33ef254`. sys11 **Kind::Recovery**. Frozen plaintext boot `fpga/zynq/release/PZ7020_STAGE1_PLAINTEXT/BOOT.BIN` `738c6a01…47d24352`. |

Do not claim BootROM RSA enforcement, permanent AES retention, or production key provisioning.

## System 13 — final secure-image construction

Status: **`DEFERRED_CUSTOM_HARDWARE`**

Not started. Future work, only after custom hardware and key custody exist:

- create permanent production RSA and AES keys through an offline key ceremony;
- build `BOOT_RECOVERY_SIGNED.BIN`;
- build `BOOT_PRODUCTION_SECURE.BIN`;
- independently verify every certificate, signature, encrypted partition and HMAC;
- keep all private keys outside Git;
- choose the final AES key source only after custom hardware is defined.

Development keys and research BBRAM keys must never become production keys. SYS06/SYS07/SYS10A research material is not a production ceremony.

## System 14 — hardware RSA root of trust

Status: **`DEFERRED_CUSTOM_HARDWARE`**

Not started. Future work:

- program the production PPK hash into the production device;
- verify the programmed hash;
- enable the Zynq RSA authentication eFuse last;
- test valid signed boot;
- test corrupted-signature refusal;
- preserve a compatible signed recovery image;
- perform the first irreversible test on a sacrificial production-revision board.

RSA eFuse programming is irreversible. It must not be performed on the current working research board (PZ7020-SL-C / PZ-StarLite used for Systems 1–12). BootROM RSA on that board remains `NOT_ENFORCED`.

## System 15 — permanent AES key policy

Status: **`DEFERRED_CUSTOM_HARDWARE`**

Not started. Two policies are recorded. **Neither is selected.**

### Battery-backed BBRAM

Requires:

- correctly routed Zynq `VCCBATT`;
- battery or qualified supercapacitor;
- voltage supervision;
- retention testing across power removal;
- battery replacement/service policy;
- secure BBRAM provisioning procedure.

Benefits:

- AES key can be changed;
- device need not permanently contain the AES key in eFuse.

Limit:

- loss of retention power removes the key.

### AES eFuse

Requires:

- final production AES key;
- irreversible provisioning procedure;
- sacrificial-board proof;
- stable programming power;
- encrypted recovery policy;
- cold-boot and tamper testing.

Benefits:

- survives complete power loss;
- no backup battery required.

Limit:

- cannot be changed after programming.

## Optional external security device

A secure element, TPM, or dedicated encryption/security chip may later provide:

- protected device identity;
- update authorization;
- monotonic rollback counters;
- remote attestation;
- sealed application or recovery keys;
- manufacturing provisioning records.

An external security chip does not automatically replace the Zynq BootROM’s BBRAM/eFuse AES key source. Its role must be explicitly designed into the custom board and boot protocol.

## Custom-board requirements

The future Golden Phase board must provide:

- battery-backed `VCCBATT` if BBRAM is selected;
- secure element/TPM footprint if selected;
- stable and supervised provisioning power;
- accessible but protectable JTAG/programming header;
- unique board/device identity;
- recovery button or protected recovery strap;
- A/B boot storage or separate authenticated recovery storage;
- write-protection support for recovery media;
- tamper/reset monitoring;
- production-key provisioning pads and documented fixture;
- ability to test RSA and AES failure without losing the only recovery path.

## Reopening conditions

Systems 13–15 may reopen only when:

1. the custom-board schematic defines the key-storage policy;
2. `VCCBATT` retention or AES eFuse policy is selected;
3. the secure element’s role is defined;
4. a sacrificial production-revision board exists;
5. a production key-custody procedure is approved;
6. signed recovery is designed before irreversible fuses are programmed.

## Current non-claims

These remain true on this tree and this research board:

- BootROM RSA is not eFuse-enforced;
- no production PPK hash is programmed;
- no production AES key is programmed;
- current BBRAM proof is powered-session research evidence (`SYS06_P1_KAT_COM12_PASS`);
- the current PZ-StarLite has no proven battery-backed retention;
- no external secure element is integrated;
- no production cold-boot security claim;
- no production provisioning claim.

System 13 is not started.
