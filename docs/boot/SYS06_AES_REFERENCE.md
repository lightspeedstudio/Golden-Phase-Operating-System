<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# System 6 — AES/HMAC reference (Bootgen oracle)

Status: **ORACLE_HOST_PARSED**  
BBRAM: **BLOCKED_PENDING_OWNER_BBRAM_DECISION**

Disposable host-only oracle exists. Independent header parse matches Bootgen `-read`. Owned AES packing, DevCfg AES/HMAC driver, board flash, and BBRAM provisioning are **not** started. `H:\BOOT.BIN` was left at System 5 PASS (not overwritten).

**Start commit (System 5 COM12 PASS):** `feba27bc9d92f708be700324d12dc219ae68a247`

**Working Stage 1** (`zynq_stage1/src/main.rs`, hash `6637678d…`) is not edited.

## Bootgen oracle (not the product packer)

| Item | Value |
| --- | --- |
| Bootgen | `D:\Xilinx\2026.1\Vitis\bin\bootgen.bat` |
| Version | v2026.1, build date Jun 5 2026-13:21:08 |
| Arch | `-arch zynq` |
| Key source CLI | `-encrypt bbram` (Zynq-7000). BIF `[keysrc_encryption]` is **not** supported on this arch; Bootgen warned and ignored it. |
| Part | `-p xc7z020clg400` (copied into `.nky` `Device`) |
| BIF | `docs/boot/sys06_oracle.bif` (no secrets) |
| Command | `bootgen -arch zynq -image docs/boot/sys06_oracle.bif -encrypt bbram -p xc7z020clg400 -w on -o zynq_stage1/owned/build/oracle/BOOT_SYS06_ORACLE.BIN` |
| Oracle file | `zynq_stage1/owned/build/oracle/BOOT_SYS06_ORACLE.BIN` (**gitignored**, host-only) |
| Oracle SHA-256 | `684d97f4a07826f6ec9a94b2d0fc2dfd710514a1ad96a0a1dac4fcce855e21de` |
| Oracle bytes | 6000404 |
| Keys | `zynq_stage1/owned/build/oracle/sys06_dev.nky` (**gitignored**, never commit) |

`.nky` fields present (values not recorded): `Device`, `Key 0` (32 bytes), `Key StartCBC` (16 bytes), `Key HMAC` (32 bytes). Bootgen 2026.1 generated a 256-bit HMAC key; UG1283’s 128-bit sample is not this file.

## Non-secret inputs

| File | SHA-256 | Bytes | Role |
| --- | --- | ---: | --- |
| `zynq_stage1/owned/build/sys05_plat.elf` | `f40f7e13a8f141afd404c407803f3de394c4e1ae082ca0ca112a932a53b248a2` | 24000 | encrypted Stage 1 |
| `ddr_kat.bin` (gitignored; bytes `00..FF`) | `40aff2e9d2d8922e47afd4648e6967497158785fbd1da870e7110266bf944880` | 256 | encrypted DDR KAT |
| `fpga/zynq/release/PZ7020_G1P_PATH1_G64/system.bit` | `d19f08d20cac85ea0e33c41a8a552166f5a9176e16977f09aa64d17e68135829` | 4045769 | frozen PATH1_G64; encrypt a copy, do not rebuild |
| `fpga/zynq/release/PZ7020_G1P_PATH1_G64/kairos_zynq.elf` | `447f9fbdec6bbdb03227c8620baa854f3a141d52ec71452251e2f63e1852eaf8` | 2475164 | plaintext Utility |

KAT contents: 256 incrementing bytes `0x00` through `0xFF`. Load address in BIF: `0x03000000`.

## Independent header parse (not Bootgen `-read`)

Little-endian u32. Boot header checksum `~sum(0x020..0x044)` matches. All four partition-header checksums match.

| Offset | Field | Oracle |
| --- | --- | --- |
| 0x020 | Width | `0xAA995566` |
| 0x024 | Ident | `0x584C4E58` (`XLNX`) |
| **0x028** | Encryption status | **`0x3A5C3C5A` (BBRAM)** |
| 0x030 | Source Offset | `0x00001700` |
| **0x034** | Length of Image (post-decrypt Stage 1) | **13380** (`0x3444`) |
| **0x040** | Total Image Length (on-flash Stage 1) | **14188** (`0x376C`) |
| 0x098 / 0x09C | IHT / PH | `0x8C0` / `0xC80` |
| IHT count | partitions | **4** |

UG585: secure `0x034` is decrypted Stage 1 (≤ 192 KiB). Secure `0x040` is larger and includes HMAC header, encryption overhead, and alignment. Measured delta `0x040 − 0x034` = **808 bytes**.

`0x034` equals `sys05_plat.elf` PT_LOAD image size (13380). `0x040` equals partition 0 encrypted byte length.

## Partition table

`enc` / `unenc` / `total` below are **bytes** (`words × 4`). Flash span uses `enc` (= `total` on this image).

| | P0 Stage 1 | P1 DDR KAT | P2 PL | P3 Utility |
| --- | --- | --- | --- | --- |
| Header | `0xC80` | `0xCC0` | `0xD00` | `0xD40` |
| File offset | `0x1700` | `0x4E80` | `0x52C0` | `0x3E0A40` |
| Encrypted | 14188 | 1068 | 4044652 | 1934548 |
| Unencrypted | 13380 | 256 | 4043904 | 1934548 |
| Total | 14188 | 1068 | 4044652 | 1934548 |
| Overhead `enc−unenc` | **808** | **812** | **748** | **0** |
| `enc % 16` | 12 | 12 | 12 | 4 |
| Load | `0` | `0x03000000` | `0` | `0x00100000` |
| Exec | `0` | `0` | `0` | **`0x00100060`** |
| Attributes | `0x10` PS | `0x10` PS | `0x20` PL | `0x10` PS |

P3 payload is **byte-identical** to Utility ELF PT_LOAD (1934548 bytes). Encryption is indicated by `enc ≠ unenc` and by header `0x028`, not by Bootgen `-read` “encryption [no]” on Zynq-7000 attribute bits.

P1 unencrypted length equals the KAT file (256). P0 unencrypted length equals Stage 1 PT_LOAD.

P2 unencrypted length **4043904** is not the frozen `.bit` file length (4045769) and is not the `.bit` `e` payload length (4045564). Bootgen transformed the bitstream before encryption. Owned packing must not assume the encrypted PL plaintext is the raw `system.bit` file. The frozen `.bit` hash above is still the source; the oracle’s P2 plaintext is Bootgen’s derived bitstream.

## HMAC placement (header-measured + Bootgen stream rules)

Whole-partition AES-CBC of P1 with `.nky` `Key 0` / `StartCBC` does **not** yield the KAT bytes. Ciphertext length `≡ 12 (mod 16)`, so the stored partition is not a single AES-CBC blob.

AMD Bootgen `zynq/src/encryption-zynq.cpp` (`AESDataStreamEncrypt`) places:

1. **Plaintext stream header** (IV at header offset 116 / `CBC_WORD_OFFSET`; encrypted word-length at 136). Header is never AES.
2. **AES-CBC** of the HMAC authentication stream: ipad (512 bits, HMAC key XOR `0x36`) ‖ (PS: 8-byte PCAP loopback header + user data + `0x20000000` NOOP pad to 64-byte SHA block) ‖ SHA pad1 ‖ 2048-bit gap ‖ opad (HMAC key XOR `0x5C`) ‖ first-digest gap ‖ SHA pad2 ‖ HMAC-SHA256 digest.
3. **Plaintext NOOP trailer** (`0x20000000`), at least 16 words, then pad the partition to 32 bytes (PCAP DMA).

HMAC key and digest live **inside that AES stream**, matching UG585 (HMAC material is not in BBRAM). Exact header byte length and FPGA vs PS header choice for P2 were not host-decrypted in this commit.

Do not implement an owned secure packer until host decrypt of P1 recovers `ddr_kat.bin` and host decrypt of P0 recovers the Stage 1 PT_LOAD, using the stream layout above rather than a guessed blob format.

## Card (not flashed)

| Item | Value |
| --- | --- |
| `H:\BOOT.BIN` SHA-256 | `ccfbbbc5a9c7ab573ccad9251dc933ab44d294ee26b7c7eccfaded0cf0955c5f` |
| Bytes | 5999828 |
| Action | none (System 5 PASS left in place) |

## What this commit does not contain

- No owned AES-256 / AES-CBC / HMAC-SHA256 engines
- No owned secure-image packer
- No DevCfg AES/HMAC driver
- No encrypted image on the SD card
- No BBRAM key write, no COM12, no SYS06 board PASS
- No `.nky`, no oracle `BOOT.BIN` in git
