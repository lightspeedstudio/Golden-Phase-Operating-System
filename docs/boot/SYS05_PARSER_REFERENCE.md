<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# System 5 — parser reference (real BOOT artifacts)

Recorded **before** device integration of `parse_all`. Values are from the files, UG585 boot header, System 2 independent parser, and `src/board/zynq_bootimg.rs`. This is not a generic Bootgen tutorial.

**System 4 gate:** evidence commit `d9bf3047a5c53f921617e812d0849ba2b4877fef` (`SYS04_COM12_PASS`). Device parse uses the 16 KiB window ADMA already placed at `0x02000000` plus FAT `DIR_FileSize`. No payload copy. No crypto. No PCAP.

**Working Stage 1** (`zynq_stage1/src/main.rs`, hash `6637678d…`) calls `zynq_bootimg::loader_bitstream_app` after loading the whole `BOOT.BIN`. System 5 does not edit that file.

## Fixtures

| Role | Path | SHA-256 | Bytes |
| --- | --- | --- | ---: |
| Known-working gold | `fpga/zynq/release/PZ7020_G1P_PATH1_G64/BOOT.BIN` | `7da748547d5016191643ddc4df555a8cf02c1d8adee3235f1dde766166ea0b85` | 6114836 |
| System 4 COM12 image | `zynq_stage1/owned/build/BOOT_SYS04_ADMA.BIN` | `e966318fb687464c6a8257d110edd71b970ff4bf6c8ed212fbfa0676543fe279` | 5997268 |

Gold is the System 1 lock / System 2 parse fixture (Bootgen product). System 4 is the owned packer image that passed COM12. Do not treat gold P1/P2 **file offsets** as a requirement for owned images (512-byte correction, SYS02).

Header tables for both files sit inside the first 16 KiB: IHT `0x8C0`, PH `0xC80`…`0xD40`.

## Shared boot header (both fixtures)

| Offset | Field | Both files |
| --- | --- | --- |
| 0x020 | Width detection | `0xAA995566` |
| 0x024 | Ident | `0x584C4E58` (`XLNX`) |
| 0x030 | Source Offset | `0x00001700` (P0 file offset, not length) |
| 0x098 | IHT pointer (bytes) | `0x000008C0` |
| 0x09C | PH pointer (bytes) | `0x00000C80` |
| 0x8C0 | IHT version | `0x01020000` |
| 0x8C4 | Partition count | **3** |
| 0x8C8 | PH offset (words) | `0x320` → `0xC80` |
| 0x8CC | Image header offset (words) | `0x240` → `0x900` |

`0x034` Length of Image is Stage 1 **byte** length (differs per fixture).

## Gold working `BOOT.BIN` partition table

`0x034` = `0x1F714` (128788). P0 end `0x20E14`. File end = P2 end `0x5D4E14` = 6114836.

| | P0 Stage 1 | P1 PL | P2 Utility |
| --- | --- | --- | --- |
| Header | `0xC80` | `0xCC0` | `0xD00` |
| enc = data = total words | `0x7DC5` | `0xF6EC0` | `0x76135` |
| Byte length | 128788 | 4045568 | 1934548 (`0x1D84D4`) |
| Load | `0x00000000` | `0x00000000` | `0x00100000` |
| Exec | `0x00000000` | `0x00000000` | **`0x00100060`** |
| Data offset | **`0x1700`** | **`0x20E40`** | **`0x3FC940`** |
| Attributes | `0x10` PS | `0x20` PL | `0x10` PS |
| Align 512 remainder | 256 | 64 | 320 |

This is the table `zynq_bootimg::loader_bitstream_app` returns on the full gold file: loader = P0, bitstream = P1, app = P2. Owned `parse_all` on a 16 KiB window with `full_boot_bin_len = 6114836` must produce the same offsets, lengths, load, and exec.

## System 4 `BOOT_SYS04_ADMA.BIN` partition table

`0x034` = `0x2AF0` (10992). P0 end `0x41F0`. File length 5997268 = `0x5B82D4` = P2 end.

| | P0 Stage 1 | P1 PL | P2 Utility |
| --- | --- | --- | --- |
| Header | `0xC80` | `0xCC0` | `0xD00` |
| enc = data = total words | 2748 | 1011443 | 483637 (`0x76135`) |
| Byte length | 10992 | 4045772 | 1934548 (`0x1D84D4`) |
| Load | `0x00000000` | `0x00000000` | `0x00100000` |
| Exec | `0x00000000` | `0x00000000` | **`0x00100060`** |
| Data offset | **`0x1700`** | **`0x4200`** | **`0x3DFE00`** |
| Attributes | `0x10` PS | `0x20` PL | `0x10` PS |
| Align 512 remainder | 256 | **0** | **0** |

P1 length is `system.bit` (4045769) rounded up to 32-bit words (4045772). Gold P1 is the Bootgen bitstream partition (4045568). P2 length and Utility addresses match gold.

## kairos `zynq_bootimg.rs` vs System 2 owned parser

On **both fixtures** the live partition fields agree: count 3, P0 PS `load=0` at `0x1700`, P1 PL, P2 PS `load=0x00100000` `exec=0x00100060`, not encrypted, no vendor RSA.

Disagreements (resolved from the working image + UG585; owned parser keeps the stricter rule):

| Topic | `src/board/zynq_bootimg.rs` | Owned System 2/5 | Resolution |
| --- | --- | --- | --- |
| Width detect `0x20` | not checked | must be `0xAA995566` | UG585 boot header; both fixtures have it |
| Ident `0x24` | `XLNX` | `XLNX` | agree |
| IHT/PH pointers `0x98`/`0x9C` | not checked | must equal `0x8C0`/`0xC80` | UG585; both fixtures match |
| Source Offset vs P0 | not checked | `0x030 == P0.data_off` and `0x034 == P0.byte_len` | UG585 Source Offset; both fixtures match |
| Overlap | not checked | reject overlapping payload ranges | both fixtures do not overlap |
| Payload bound | `end > boot.len()` on the **slice passed in** | `end > full_boot_bin_len` | 16 KiB window cannot hold payloads; FAT length is the bound |
| `total_words` | omitted from struct | stored | both fixtures `enc=data=total` |
| First three vs IHT count | `parse_partitions` requires `count>=3` and reads P0–P2 | uses IHT count (max 14) | both fixtures count **3** |
| Utility exec | `exec >= 0x00100000` to classify app | **must be `0x00100060`** | gold P2 and Utility ELF `e_entry`; SYS01 lock |
| 64-byte Ed25519 trailer | working `main.rs` `split_signed` peels 64 bytes **before** parse | **not used** (no crypto this system) | gold file end equals P2 end — peeling would truncate P2. `parse_all` uses FAT file length |

`zynq_bootimg.rs` hash remains `c96b4b13ac7b884b266c6f90b55bfd0facd2a28105446af2d1dd207e4e6ce999` (not edited).

## `parse_all(header_window, header_window_len, full_boot_bin_len)`

- Header field reads use `header_window[..header_window_len]` only. `header_window_len ≤ 16384` and ≤ slice length. IHT and every partition header must end at or before `header_window_len`.
- Payload `data_off + byte_len` uses **checked** add and must be `≤ full_boot_bin_len` (FAT `BOOT.BIN` size). Payloads are not loaded.
- Gold layout is accepted (P1/P2 not 512-aligned). System 4 layout is accepted (P1/P2 512-aligned).
- Mutations of ident, checksum, source offset, overlap, encryption, truncated file length, bad Utility exec are rejected.
- After a successful parse, UART:

```
IMAGE HEADER PASS
PARTITIONS count=3
UTILITY ENTRY 00100060
SYS05 HOLD
```

`count` is the IHT partition count from the image on the card (3 for both fixtures). After copying `BOOT_SYS05_PARSER.BIN`, FAT still prints live `DIR_FileSize` (this pack: 5999828). COM12 **SYS05_COM12_PASS** on UART `com12_sys05.uart` SHA-256 `b8f7145da7d3cf24c572a06b3e7a61f1b7b6a9a8bc281e366a1502da0665d3d8`. Do not start System 6.
