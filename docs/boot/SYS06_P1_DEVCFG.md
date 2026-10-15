<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# System 6 — P1 DevCfg KAT (host-wired)

Status: **SYS06_P1_KAT_COM12_PASS**

Owned Stage 1 ADMA-loads the complete encrypted `BOOT.BIN` into reserved DDR, re-parses with `parse_all_encrypted`, and sends partition 1 through the 6B DevCfg secure-transfer driver. Destination is `0x03000000`. After DMA success with no HMAC/fatal bits, the decrypted KAT bytes are compared in DDR.

Silicon: COM12 `P1 KAT PASS` then `SYS06 HOLD` on `de71e05` packed `BOOT_SYS06_P1.BIN`. Evidence: `fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS06_20260820/`. This does **not** close System 6.

Gold `parse()` / `parse_all()` still reject the image as `Encrypted`. Device code does not run software AES; DevCfg hardware does. Host tests use the already-proven stream open as the MMIO DMA stand-in.

Working Stage 1 (`zynq_stage1/src/main.rs`, hash `6637678d0e01b007caf3b5e150ed37e48ed5af6f55cc009a3b16e0d5c4ebd67d`, 9580 bytes) is not edited. System 5 `parse_all` / plaintext path is unchanged.

## Reserved DDR

| Region | Address | Role |
| --- | --- | --- |
| BOOT.BIN | `0x02000000` .. `0x03000000` (16 MiB cap) | Full encrypted image |
| P1 dest | `0x03000000` | Decrypted DDR KAT (256 bytes `00..FF`) |

P1 source is `0x02000000 +` partition file offset (`0x4E80` on the locked oracle). Encrypted length 1068, unencrypted 256. Those lengths must match `predict_enc_bytes(256, false)` and the partition header. Source and dest ranges must not overlap.

ADMA descriptors cannot exceed 65535 bytes. Each CMD18 is at most **127×512 = 65024** bytes. Contiguous FAT runs are split to that size. PIO is refused.

## Fail closed

| Fault | Error |
| --- | --- |
| Size / overlap / overflow | `Bounds` |
| Encrypted parse / missing P1 KAT partition | `Parse` |
| ADMA / FAT | `Adma` |
| PIO | `Pio` |
| DevCfg HMAC | `Hmac` |
| DevCfg poll budget | `Timeout` |
| Other DevCfg | `Dma` |
| Dest bytes ≠ `00..FF` | `Kat` |

## Device UART (HOLD)

```
SD INIT PASS
SD FAT BOOT.BIN PASS bytes=<DIR_FileSize>
SD ADMA2 CMD18 PASS bytes=<DIR_FileSize>
IMAGE HEADER PASS
PARTITIONS count=4
ENCRYPTED
UTILITY ENTRY 00100060
P1 KAT PASS
SYS06 HOLD
```

On P1 failure: `P1 FAIL` and hold is not printed.

## Non-claims

- Not System 6 closed
- Not eFuse (`eFuse: NOT_ENFORCED`)
- Not cold-power VCCBATT / BBRAM retention
- Not encrypted P2 / PCAP / FabricInit (System 8)
- Not P3 Utility jump
- Not System 7 RSA-2048
