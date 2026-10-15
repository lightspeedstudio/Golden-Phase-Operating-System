<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# LightSpeed Studio LLC — Stage 1 ownership and licensing boundary

SPDX on owned Stage 1 sources and these docs: `AGPL-3.0-only OR LicenseRef-GPOS-Commercial`. Copyright line: **LightSpeed Studio LLC**. A standalone `LicenseRef-GPOS-Commercial` text file is **NOT_VERIFIED** in this repository (identifier appears in headers only).

Purchasing an AMD Zynq-7000 device permits use of that silicon subject to AMD’s applicable terms. It does **not** transfer ownership of AMD silicon IP, BootROM, or hardened crypto to LightSpeed Studio LLC.

## What LightSpeed Studio LLC owns

Independently authored in `zynq_stage1/owned` (zero `[dependencies]`):

- Stage 1 entry (`start.S`, platform UART/PS7/WDT/boot-mode);
- boot-image packer and parser;
- AES/HMAC **host** framing / encrypt packer (device does not run software AES);
- DevCfg secure-transfer driver (MMIO to AMD engines);
- software RSA-2048 later-partition verify and SHA-256;
- SD / ADMA2 / FAT locator;
- plaintext PCAP and Utility handoff;
- FPGA integration of the locked PATH1_G64 `system.bit` into owned `BOOT.BIN`;
- kernel/Utility integration (jump to locked `kairos_zynq.elf` `0x00100060`);
- host tests, SYS11 classifier, capture scripts, evidence docs.

Shortcut behavioral reference `zynq_stage1/src/main.rs` is also in-tree and must not be treated as vendor FSBL.

## What LightSpeed Studio LLC does not own

- AMD Zynq-7000 silicon, including BootROM and hardened AES/HMAC;
- ARM architecture and Cortex-A9;
- Xilinx/AMD Vivado and Vitis;
- Rust `core` and `compiler_builtins` as linked into the ELF (SYS10 provenance: allowed link members);
- third-party components under their original licenses (e.g. `third_party/sunset`);
- quarantined `zynq_fsbl.elf` (refused by name and hash `8b71aa707fd9e200a89cd2b966cf3d3ec26ed0350495718a51a9f8ec259f7874`).

Public Bootgen was used as an **oracle** for format comparison. Vendor FSBL C was not copied into `zynq_stage1/owned`.
