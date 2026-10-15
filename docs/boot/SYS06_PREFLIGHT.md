<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# System 6 preflight — before BBRAM / SD / COM12

Source commit: `de71e058017150caa81ef3828cc8294ce6e879c5`

Powered-session silicon later PASSed (`SYS06_P1_KAT_COM12_PASS`, evidence `BOOT_STAGE1_SYS06_20260820/`). This preflight file keeps the measurements taken **before** that run. It does **not** claim eFuse, cold-power BBRAM retention, or encrypted P2/PCAP.

Keys were not printed. `.nky` stays gitignored.

## 1. Encrypted `BOOT.BIN` built from `de71e05`

P0 is `sys06_plat` PT_LOAD (15384 bytes), packed by owned `pack_secure_image` with `sys06_dev.nky`. This is the silicon P1 image. It is **not** the Bootgen oracle (oracle P0 is `sys05_plat`).

| Field | Value |
| --- | --- |
| Path | `zynq_stage1/owned/build/oracle/BOOT_SYS06_P1.BIN` (gitignored) |
| SHA-256 | `363a44aa6a0ab3b091f2f5b43dd14242cc8643afc05d237f4a4c28d0a10a5bd8` |
| Bytes | 6002388 |
| Header `0x028` | `0x3A5C3C5A` (BBRAM) |
| Header `0x02C` | `0x01010000` |
| P1 file offset | `0x5640` |
| Stage 1 ELF | `zynq_stage1/owned/build/sys06_plat.elf` SHA-256 `c346065ba2516ce91d454c1180816dfc706a8fe3a80db101665ac76f57e50b01` 27648 bytes |
| Stage 1 PT_LOAD | `zynq_stage1/owned/build/sys06_plat.bin` SHA-256 `8d79e68a6a9f8dbba041bcb2564f4048364fc172ded9a4ada1f99d36c6624046` 15384 bytes |
| `e_entry` / `p_paddr` | `0x00000000` / `0x00000000` |
| Linker | `fpga/zynq/tools/arm-gnu-toolchain-13.3.rel1-mingw-w64-i686-arm-none-eabi/bin/arm-none-eabi-gcc.exe` |

Oracle (packer identity KAT, **not** this COM12 image): `zynq_stage1/owned/build/oracle/BOOT_SYS06_ORACLE.BIN` SHA-256 `684d97f4a07826f6ec9a94b2d0fc2dfd710514a1ad96a0a1dac4fcce855e21de` 6000404 bytes. P0 is `sys05_plat`. Writing it cannot print `P1 KAT PASS`.

## 2. Packer and BBRAM programmer key file

**Not confirmed.** Packer key file is measured. No BBRAM programmer exists in this tree, and `D:\Xilinx\2026.1\Vitis` is absent on this machine.

| Field | Value |
| --- | --- |
| Packer key path | `zynq_stage1/owned/build/oracle/sys06_dev.nky` |
| Key-file SHA-256 | `dfb4c86e00780e96fc0460b992a2db340cefb8ede143afd798e9f928f4264fe4` |
| Key-file bytes | 237 |
| Structure (no values) | `Key 0` 64 hex chars; `StartCBC` 32 hex chars; `HMAC` 64 hex chars |
| BIF | `docs/boot/sys06_oracle.bif` `[aeskeyfile]` same path |
| BBRAM programmer | none in-tree; Vitis `bootgen.bat` / `program_flash.bat` / `zynq_bbram.tcl` not present |

A later programmer may proceed only if it reads this same path and the key-file SHA-256 above, without printing Key 0 / StartCBC / HMAC.

## 3. BBRAM only, no eFuse

**Confirmed for the image and packer.** Not confirmed as a completed program step (none was run).

- Packer writes `ENC_STATUS_BBRAM` (`0x3A5C3C5A`) at boot header `0x028`.
- Packed P1 image measures `0x028 = 0x3A5C3C5A`.
- Oracle Bootgen command was `-encrypt bbram` (see `docs/boot/SYS06_AES_REFERENCE.md`).
- UG585 eFUSE status is `0xA5C3C5A3`. That word is not in this image.
- No eFUSE programmer was invoked.

## 4. This board’s VCCBATT / BBRAM retention path

**Not confirmed. Available schematic does not show a retention path.**

PZ-StarLite schematic extract `fpga/zynq/evidence/kairos_zynq_boot/manufacturer_pack_read_20260801/Puzhi_PZ-StarLite_Schematic.pdf.full.txt`:

- XC7Z020 pin **F11** is named `VCCBATT_0`.
- Nearby decoupling: `C69` `0.1uF/10V/X5R/0402`.
- `VCCBATT` appears once. No coin cell, `BT*`, or battery net tied to that pin.
- `VBAT` on the same extract is USB3320 pin 21, not PS BBRAM.

Hardware user manual extract has no BBRAM / VCCBATT retention section. `docs/PZ7020_KNOWN_LIMITATIONS.md` does not claim a PZ7020 BBRAM keystore.

BBRAM must not be programmed until a battery or always-on VCCBATT net is traced on this PCB.

## 5. Preserved System 5 recovery

Live card was not written. Hashes measured 2026-08-20:

| Item | SHA-256 | Bytes |
| --- | --- | ---: |
| Live `H:\BOOT.BIN` | `ccfbbbc5a9c7ab573ccad9251dc933ab44d294ee26b7c7eccfaded0cf0955c5f` | 5999828 |
| `zynq_stage1/owned/build/BOOT_SYS05_PARSER.BIN` | `ccfbbbc5a9c7ab573ccad9251dc933ab44d294ee26b7c7eccfaded0cf0955c5f` | 5999828 |
| `fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS06_20260819/SD_BOOT_BEFORE.BIN.preserved` | `ccfbbbc5a9c7ab573ccad9251dc933ab44d294ee26b7c7eccfaded0cf0955c5f` | 5999828 |
| System 5 UART `.../BOOT_STAGE1_SYS05_20260819/com12_sys05.uart` | `b8f7145da7d3cf24c572a06b3e7a61f1b7b6a9a8bc281e366a1502da0665d3d8` | 258 |

`fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS05_20260819/SD_BOOT_BEFORE.BIN.preserved` is **System 4** (`e966318fb687464c6a8257d110edd71b970ff4bf6c8ed212fbfa0676543fe279`, 5997268). Do not restore that file as System 5.

## 6. Expected COM12 markers (not captured)

COM12 was not opened. Ordered strings from `sys03_platform_ready` then `sys06_device`, for FAT size **6002388**:

1. `GP-S1 BEGIN`
2. `UART EARLY PASS`
3. `PS7 CLOCK PASS`
4. `DDR BEGIN`
5. `DDR PASS`
6. `WDT STOP PASS`
7. `BOOTMODE SD PASS`
8. `SD INIT PASS`
9. `SD FAT BOOT.BIN PASS bytes=6002388`
10. `SD ADMA2 CMD18 PASS bytes=6002388`
11. `IMAGE HEADER PASS`
12. `PARTITIONS count=4`
13. `ENCRYPTED`
14. `UTILITY ENTRY 00100060`
15. `P1 KAT PASS`
16. `SYS06 HOLD`

On P1 failure: `P1 FAIL` and no `SYS06 HOLD`. Not P2 / FabricInit. Not Utility jump.

Port: COM12, 115200 8N1, DTR off, RTS off, one RESET after port open (same as System 5).
