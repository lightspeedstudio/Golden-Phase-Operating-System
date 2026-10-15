<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# System 6 — encrypted header parse (16 KiB window)

Status: **SYS06_PARSE_ENCRYPTED_HEADER** (headers). P1 DevCfg wiring is `SYS06_P1_DEVCFG_KAT` (`docs/boot/SYS06_P1_DEVCFG.md`).

Owned Stage 1 path parses the encrypted four-partition image from the ADMA 16 KiB header window plus FAT `DIR_FileSize`. Payloads are not copied. DevCfg is not called. This commit does **not** close System 6.

Gold `parse()` / `parse_all()` still reject the same bytes as `ParseError::Encrypted`. Encrypted headers use `parse_all_encrypted`.

Working Stage 1 (`zynq_stage1/src/main.rs`, hash `6637678d0e01b007caf3b5e150ed37e48ed5af6f55cc009a3b16e0d5c4ebd67d`, 9580 bytes) is not edited.

## Fixture

Locked oracle `zynq_stage1/owned/build/oracle/BOOT_SYS06_ORACLE.BIN` (gitignored). Byte-identical to the owned packer (`SYS06_PACKER_ORACLE_IMAGE`). 6,000,404 bytes.

| | P0 Stage 1 | P1 DDR KAT | P2 PL | P3 Utility |
| --- | --- | --- | --- | --- |
| Encrypted | yes | yes | yes | no |
| File offset | `0x1700` | `0x4E80` | `0x52C0` | `0x3E0A40` |
| Load | `0` | `0x03000000` | `0` | `0x00100000` |
| Exec | `0` | `0` | `0` | **`0x00100060`** |

IHT count **4**. Boot header `0x028` = `0x3A5C3C5A` (BBRAM). `0x034` = 13380. `0x040` = 14188.

On-flash partition span is `total_words × 4` (`ImageWordLen`), not the decrypted `DataWordLen`.

## Device UART (HOLD)

Same SD init / FAT / ADMA lines as System 5, then headers. Full-file ADMA and P1 DevCfg are the P1 gate, not this parse-only UART.

```
IMAGE HEADER PASS
PARTITIONS count=4
ENCRYPTED
UTILITY ENTRY 00100060
SYS06 HOLD
```

## Non-claims

- Not System 6 closed
- Not BBRAM / eFUSE programming
- Not COM12 / SD write
- Not P2 / FabricInit (System 8)
- Not System 7 RSA
