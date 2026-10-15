<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# System 6B — DevCfg secure-transfer reference

Status: **SYS06B_HOST_SEQUENCE** (register sequence under a host MMIO model).

This commit does **not** close System 6. System 6 closes only after the owned packer, integration, and silicon PASS.

No software AES or HMAC. No owned packer. No BBRAM. No card write. No COM12. 6A stays `SYS06A_FRAMING_UNRESOLVED`. FabricInit (PROG_B / PL power / level shifters) is **System 8**, not this driver.

Inspected as behavioral reference only. No vendor C was copied into `zynq_stage1/owned`.

**Working Stage 1** (`zynq_stage1/src/main.rs`, hash `6637678d0e01b007caf3b5e150ed37e48ed5af6f55cc009a3b16e0d5c4ebd67d`, 9580 bytes) is not edited and is not called.

## What was opened (Vitis 2026.1)

| Artifact | Path | SHA-256 | Bytes |
| --- | --- | --- | ---: |
| FSBL image mover | `D:\Xilinx\2026.1\Vitis\data\embeddedsw\lib\sw_apps\zynq_fsbl\src\image_mover.c` | `41a592b194c4ee6d6d8f37e1d5c2f0238c73938eb5956a2ac230c703dde2713c` | 36530 |
| FSBL PCAP | `...\zynq_fsbl\src\pcap.c` | `26fd6a726aef9fc13e4ea5c70d08165e1a2a6e9ff5ae8b6ce1d7d25fb9915c35` | 21508 |
| FSBL PCAP header | `...\zynq_fsbl\src\pcap.h` | `b2756f173f842e62ceadbbc8eb7300bfb4d9f9553f2e1577a382180f7673d119` | 2733 |
| FSBL header | `...\zynq_fsbl\src\fsbl.h` | `6f34da46af1f5c7087c2c17119a10926f80e2859bb49ea3c9ea859a260b11d14` | 22118 |
| DevCfg driver | `D:\Xilinx\2026.1\Vitis\data\embeddedsw\XilinxProcessorIPLib\drivers\devcfg_v3_9\src\xdevcfg.c` | `026e265e74d5330f608278cdd3029acc9b7ed12ccbc2a483466a95ba04e28bf7` | 28526 |
| DevCfg API | `...\devcfg_v3_9\src\xdevcfg.h` | `85584fe80f6b38613351255fb42c3c91053324bbe1616b29792fbaec983192d2` | 13792 |
| DevCfg HW | `...\devcfg_v3_9\src\xdevcfg_hw.h` | `97681edd9a13c172cbf2d5a17a3e080bc7bac9a2f7ec586e58c8f79206be71cc` | 12762 |

Owned Stage 1 (precondition, not vendor):

| Artifact | Path | SHA-256 | Bytes |
| --- | --- | --- | ---: |
| SCTLR caches off | `zynq_stage1/owned/src/start.S` | `6a2d04f216384007c180dd9bfdbf3b4d40cb588609d85ab4acfd07b4d73d37a5` | 608 |
| Poll budget | `zynq_stage1/owned/src/platform/mmio.rs` | `ae425ad22701b4b68bc2a6a380924f60c035e832f5a2503a46903b91bfa6c42b` | 3275 |

UG585 (no local PDF hash): [PCAP bridge](https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/PCAP-Bridge-to-PL), [secure bitstreams and software images](https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/Secure-Bitstreams-and-Software-Boot-Images), [boot-image decrypt/auth](https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/Boot-Image-and-Bitstream-Decryption-and-Authentication), DevCfg register map (base `0xF8007000`).

Inspected functions (names recorded for provenance; owned code does not use them as identifiers): `DecryptPartition`, encrypted flag (`ImageWordLen != DataWordLen`), `PcapDataTransfer`, `PcapLoadPartition`, `ClearPcapStatus`, `InitPcap`, `XDcfgPollDone`, `FabricInit`, `XDcfg_CfgInitialize`, `XDcfg_Transfer`, `XDcfg_InitiateDma`.

## Registers (UG585 DevCfg)

Base `0xF8007000`. Width 32-bit.

| Offset | Name | Role |
| --- | ---: | --- |
| `0x00` | CTRL | `PCAP_RATE_EN` bit 25 (`0x02000000`) = quarter-rate. AES enable bits 11:9 (`0x00000E00`) are **not** owned. `PCFG_PROG_B` bit 30 is FabricInit (System 8). |
| `0x0C` | INT_STS | W1C. DMA done bit 13 (`0x00002000`). `PCFG_DONE` bit 2 (`0x00000004`). HMAC error bit 6 (`0x00000040`). |
| `0x14` | STATUS | `PCFG_INIT` bit 4 (`0x00000010`). DMA queue full bit 31 (`0x80000000`). Queue empty bit 30 (`0x40000000`). `DMA_DONE_CNT` bits 29:28 (`0x30000000`). |
| `0x18` | DMA_SRC_ADDR | Physical source. Two LSBs `01` = last command. |
| `0x1C` | DMA_DST_ADDR | Physical dest, or `0xFFFFFFFF` for PL config. Last-command LSBs same rule. |
| `0x20` | DMA_SRC_LEN | Source length in **words**. Field mask `0x07FFFFFF`. |
| `0x24` | DMA_DST_LEN | Dest length in **words**. Writing this register starts DMA (UG585). |
| `0x34` | UNLOCK | First APB access after reset: `0x757BDF0D`. |
| `0x80` | MCTRL | PCAP loopback bit 4 (`0x00000010`). Secure path **clears** it. |

### INT_STS error bits (FSBL poll/clear mask)

FSBL `pcap.h` fatal mask (AXI write-timeout bit 23 is in the DevCfg driver mask, not this FSBL mask):

| Bit | Mask | Meaning |
| ---: | --- | --- |
| 22 | `0x00400000` | AXI write response error |
| 21 | `0x00200000` | AXI read timeout |
| 20 | `0x00100000` | AXI read response error |
| 18 | `0x00040000` | Rx FIFO overflow |
| 15 | `0x00008000` | Illegal DMA command |
| 14 | `0x00004000` | DMA command queue overflow |
| 11 | `0x00000800` | PCAP-to-DMA length error |
| 6 | `0x00000040` | HMAC error (`PCFG_HMAC_ERR`) |

Owned driver also treats AXI write-timeout `0x00800000` as fatal (DevCfg driver error mask). HMAC is fatal and classified first.

## Encrypted flag and DMA lengths

Encrypted iff `ImageWordLen != DataWordLen` (FSBL image mover). Owned secure transfer requires `ImageWordLen > DataWordLen`.

| Path | Source words | Dest words | Dest address |
| --- | --- | --- | --- |
| PS decrypt (`PcapDataTransfer`, concurrent secure) | `ImageWordLen` | `DataWordLen` | physical RAM (in-place uses the same address) |
| PL config (`PcapLoadPartition`, secure PCAP write) | `ImageWordLen` | `DataWordLen` | **`0xFFFFFFFF`** (FSBL overwrites the caller dest with this sentinel) |

Last-command: OR `1` into source and dest addresses before the DMA register writes (`pcap.h` `PCAP_LAST_TRANSFER = 1`; DevCfg note: LSBs `2'b01` = last command). `0xFFFFFFFF | 1` stays `0xFFFFFFFF`.

## Ordered MMIO — secure PS

After caller preconditions (physical addrs, caches off, `ImageWordLen > DataWordLen`):

1. Write UNLOCK `0x757BDF0D`.
2. Write INT_STS `0xFFFFFFFF` (W1C all).
3. Read INT_STS. Refuse if any fatal error bit remains.
4. Read STATUS. Refuse if DMA queue full (`0x80000000`). If queue not empty (`0x40000000` clear): require INT_STS DMA_DONE, then W1C DMA_DONE.
5. If `STATUS & 0x30000000 != 0`, write STATUS as `(current \| 0x30000000)` to clear `DMA_DONE_CNT`. **INT_STS DMA_DONE W1C is not this cleanup.**
6. Read STATUS. Refuse if `PCFG_INIT` (`0x00000010`) is clear. Refuse if queue full.
7. Read MCTRL; write MCTRL with loopback bit 4 cleared.
8. Read CTRL; write CTRL with bit 25 set (`0x02000000`). Do not change AES bits 11:9.
9. Write DMA_SRC_ADDR = `src_phys \| 1`.
10. Write DMA_DST_ADDR = `dest_phys \| 1`.
11. Write DMA_SRC_LEN = `ImageWordLen`.
12. Write DMA_DST_LEN = `DataWordLen` (starts DMA).
13. Poll INT_STS for DMA_DONE, bound `POLL_SPINS` (`100_000_000`). Each iteration: classify fatal bits; if SWDT mode bit 0 is set, write restart `0x1999` to `0xF8005008`; else no-op. FSBL `MAX_COUNT` is `1_000_000_000`; owned Stage 1 uses `POLL_SPINS`.
14. W1C INT_STS DMA_DONE only (not `0xFFFFFFFF`).
15. If `DMA_DONE_CNT` nonzero, clear through STATUS as in step 5.
16. Read INT_STS. Refuse on fatal bits.

FSBL restarts WDT once before DMA. Sys03 **stops** SWDT (`wdt::stop`). Owned kick during poll is therefore a no-op on the current Stage 1 path and is required if SWDT is later left enabled.

## Ordered MMIO — secure PL (fabric prepared)

Same as PS through step 12, except:

- Caller must pass `fabric_prepared = true`. Otherwise refuse **before** MMIO. This is not FabricInit.
- Dest address written is `0xFFFFFFFF`, not the partition load address.
- Transfer type is secure PCAP write (same loopback clear + quarter-rate as concurrent secure).
- After DMA_DONE W1C, poll INT_STS `PCFG_DONE` (`0x00000004`) with the same bound and WDT kick, then W1C `PCFG_DONE` only.
- Then STATUS `DMA_DONE_CNT` cleanup and final error read, as PS.

Without `fabric_prepared`, this is not PL configuration. FabricInit (level shifters `0xF8000900`, `PCFG_PROG_B` high/low, wait `PCFG_INIT` clear then set, optional 5 ms when AES fuse key) is System 8.

## Cache and physical addresses

DevCfg DMA uses **physical** addresses. If D-cache is on, the source range must be cleaned and the dest range invalidated around the DMA (`xdevcfg.c` transfer note). System 3 `start.S` clears SCTLR M/C/I. The owned driver requires `caches_off` and refuses otherwise. It does not implement cache maintenance.

PS dest/src must be 32-bit aligned physical RAM: `0x00000000..=0x3FFFFFFC` or OCM high `0xFFFC0000..=0xFFFFFFFC`. MMIO windows (including DevCfg `0xF8007000` and SLCR `0xF8000000`) are refused as PS dest/src.

## AES / HMAC (hardware only)

BootROM enables the PL AES-256-CBC + HMAC-SHA256 unit for encrypted boot. `InitPcap` only unlocks DevCfg. The owned driver must not set or clear CTRL AES enable bits. HMAC failure is INT_STS bit 6, not a software tag compare. Keys are not read, stored, or printed.

## WDT policy

| Path | Behavior |
| --- | --- |
| Owned sys03 | Stop SWDT (`0xF8005000` enable clear) after a restart-key write if it was enabled |
| FSBL PCAP | Restart WDT before DMA |
| Owned 6B poll | If mode enable bit 0 is set, write `0x1999` to `0xF8005008`; else no write |

## Non-claims

- Not silicon AES PASS, not BBRAM decrypt, not packed encrypted `BOOT.BIN`
- Not complete PL configuration / PATH1_G64 load
- Not System 6 closed
- Not System 7 RSA
- Host claim only: DevCfg AES/HMAC **register sequence** matches UG585 and the inspected Zynq-7000 FSBL/DevCfg behavior under a host MMIO model
