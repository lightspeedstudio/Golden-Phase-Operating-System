<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# System 7 — RSA-2048 later-partition reference

Status: **SYS07_RSA_REFERENCE_LOCKED**

Starts from System 6 P1 KAT COM12 PASS (`88c049a1e672e61fb58330cf34b87f41c9db944d`). Working Stage 1 (`zynq_stage1/src/main.rs`, hash `6637678d0e01b007caf3b5e150ed37e48ed5af6f55cc009a3b16e0d5c4ebd67d`, 9580 bytes) is not edited. No eFuse program. No hardware RSA claim. No PCAP.

BootROM RSA stays **not enforced**. Stage 1 verifies **later partitions only** in software.

## Sources (read before code)

| Doc | What was used |
| --- | --- |
| [UG585 RSA Authentication](https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/RSA-Authentication) | BootROM RSA is enabled only by blowing PS eFuse RSA Authentication Enable. That fuse is not blown. |
| [UG585 RSA Authentication Certificate Format](https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/RSA-Authentication-Certificate-Format) | Header `0x00000101`. Each public key: 2048-bit modulus, 2048-bit modulus extension, 32-bit exponent padded to 512 bits. SHA-256 ⇒ partitions and ACs padded to a 64-byte boundary. |
| [UG1283 Zynq-7000 AC](https://docs.amd.com/r/en-US/ug1283-bootgen-user-guide/Zynq-7000-SoC-Authentication-Certificate) | Field offsets. Other-partition hash input. Header-table AC if any partition is authenticated (Bootgen oracle only). |
| [UG1283 AC header bits](https://docs.amd.com/r/en-US/ug1283-bootgen-user-guide/Zynq-7000-SoC-Authentication-Certificate-Header) | PKCS#1 v1.5, SHA-256, RSA-2048, SPK enable. Packed LE word is `0x00000101` (UG585). UG1283 table text `0x0101000` and UG821 `0x01010000` are documentation typos; the oracle word is `0x00000101`. |

Public Bootgen (Apache-2.0) was used as an **oracle only**. No vendor file was copied into `zynq_stage1/owned`.

## Bootgen oracle (layout comparison only)

Bootgen v2026.1 (`Jun 5 2026-13:21:08`) refused a later authenticated partition unless the bootloader was also authenticated. The oracle therefore authenticates P0. **Device software still does not verify P0.** Owned images keep P0 unsigned so BootROM RSA remains `NOT_ENFORCED`.

| Artifact | Path | SHA-256 | Bytes |
| --- | --- | ---: | ---: |
| Oracle `BOOT.BIN` (gitignored) | `zynq_stage1/owned/build/oracle/sys07/BOOT_SYS07_ORACLE.BIN` | `76b26ab48451519b7c2852452341843c72aed4b28f73a8bc2122bbea6220779b` | 25024 |
| PPK PEM (gitignored, public `n`/`e`) | `…/sys07/ppk.pem` | `3f2dfdfed43e833e04f85f68598d602cad4c36721e20808f4dff220134a8c661` | 451 |
| SPK PEM (gitignored, public `n`/`e`) | `…/sys07/spk.pem` | `5f729b2837973ea070d2ef48dc0ae4a71a9e0db7d70c60f6ae87a75ad6a99426` | 451 |
| PSK PEM (gitignored private; hash only) | `…/sys07/psk.pem` | `00fa58153d21267cbe2fd3ba22d4b788e6f4f48a07188ae50152bb7a3bbb8d10` | 1679 |
| SSK PEM (gitignored private; hash only) | `…/sys07/ssk.pem` | `83c0d4e0379c2793f3beeb770cf2283eabad2726c3841e70eb6dd029300a6f64` | 1679 |
| BIF (gitignored) | `…/sys07/oracle.bif` | `57fba5100b838d5d176361fa44f3adf92bc3945992dedbadc6fe7dd7e0ba5a8a` | 355 |
| Committed P1 payload | `docs/boot/SYS07_ORACLE_P1.bin` | `40aff2e9d2d8922e47afd4648e6967497158785fbd1da870e7110266bf944880` | 256 |
| Committed P1 AC | `docs/boot/SYS07_ORACLE_P1_AC.bin` | `b46acd5bb98be90fbee1b625eff3e1c42d0cc12898164542187abbe82ced85b6` | 1728 |
| Committed first 16 KiB | `docs/boot/SYS07_ORACLE_FIRST_16K.bin` | `470c0a2a8b55577d64f901cd49e7547d22c52b3c7f1f3b7da28e5fdfbce5c46d` | 16384 |
| Owned COM12 `BOOT.BIN` (gitignored) | `zynq_stage1/owned/build/oracle/BOOT_SYS07_P1.BIN` | `fe606b883eaede4d8f8b7197be78e55c64b59fe76d4176a257740cc235f93388` | 28608 |

Oracle dump (after align): header AC `0x1040` length `0x6C0`; P0 `0x1700` length `0x3C40`; P0 AC `0x5340`; P1 `0x5A00` length `0x100`; P1 AC `0x5B00`.

P1 partition header at `0xCC0`: `enc=data=64` words (256 bytes), `total=496` words (`256+1728`), `load=0x03000000`, `attr=0x00008010` (`ATTR_PS\|ATTR_RSA`).

## Independently measured AC layout

Proven from the oracle image plus PEM moduli (`int`/`hashlib` measurement only; not the owned verifier). All three ACs (header, P0, P1) match.

| Off | Field | Proven value |
| --- | --- | --- |
| `0x00` | Authentication header | `0x00000101` little-endian |
| `0x04` | Certificate size | `0x6C0` = 1728 |
| `0x08` | UDF | 56 bytes (zeros in this oracle) |
| `0x40` | PPK modulus | **256 bytes little-endian**. Matches PEM `n`. |
| `0x140` | PPK modulus extension | **256 bytes little-endian** `R² mod n` with `R = 2^{bitlen(n)}` |
| `0x240` | PPK exponent | 4 bytes little-endian `0x00010001`, then 60 zero bytes |
| `0x280` | SPK modulus / ext / exp | Same layout as PPK. PEM `n`/`e` match. |
| `0x4C0` | SPK signature | 256-byte **little-endian** integer |
| `0x5C0` | Partition signature | 256-byte **little-endian** integer |

RSA public operation: `m = s^e mod n` with `s` and `n` interpreted little-endian. `I2OSP(m, 256)` is standard **big-endian** PKCS#1 v1.5.

PKCS#1 v1.5 SHA-256 encoding (RFC 8017 / Bootgen `tPad`):

```
EM = 00 01 || FF×202 || 00 || DigestInfo || H
DigestInfo = 30 31 30 0D 06 09 60 86 48 01 65 03 04 02 01 05 00 04 20
```

SPK signature message is SHA-256 of the 576-byte SPK structure at `0x280..0x4C0` (`N ‖ N_ext ‖ E ‖ 60-byte pad`).

Later-partition signature message (P1, proven):

```
SHA-256( partition ‖ AC[0 .. 0x5C0] )
```

P1 payload is 256 bytes `00..FF`. Hash input length 1728. This matches UG1283 “other partition” (`Partition ‖ Padding ‖ Authentication Header ‖ PPK ‖ SPK ‖ SPK Signature`).

Header-table AC hash (oracle only, not used on device): SHA-256 of bytes `0x8C0 .. 0x1040` concatenated with that AC except the last 256 bytes. Proven. Includes the gap after the null partition header.

FSBL hash input (`Boot Header ‖ FSBL`) was **not** proven against this oracle’s P0 signature with the concatenations tried (`0x8A0` and `0x8C0`). System 7 does not verify P0. Stop condition is the certificate layout plus later-partition hash; those are proven.

## Published RSA-2048 KATs (source + hash)

Verify KAT (PKCS#1 v1.5 SHA-256, 2048-bit):

| Field | Value |
| --- | --- |
| Source | `https://raw.githubusercontent.com/C2SP/wycheproof/master/testvectors_v1/rsa_signature_2048_sha256_test.json` |
| Project | C2SP/wycheproof `testvectors_v1`, algorithm `RSASSA-PKCS1-v1_5`, `keySize` 2048, `sha` SHA-256 |
| File SHA-256 | `94a917b01ff50fb874cfc05bf29b4af44868d944a6558201cf18380da93fb393` |
| File bytes | 211075 |
| Vector | first test group, `tcId` 3, `result` `valid`, `msg` `54657374` (`Test`) |

Wycheproof stores `n` and `sig` as big-endian. Owned Zynq verify uses little-endian `n`/`sig` from the AC; tests reverse those fields.

Round-trip KAT key (published, not a device secret): first `privateKey` in C2SP/wycheproof `testvectors_v1/rsa_pkcs1_2048_test.json` (`keySize` 2048, `e=0x010001`). Used only to exercise owned sign→verify. Not programmed to eFuse.

## Device contract

COM12 PASS order (contiguous, nothing between these lines):

```
BOOTROM RSA2048 STAGE1 status=NOT_ENFORCED
STAGE1 RSA2048 PARTITION PASS implementation=SOFTWARE
SYS07 HOLD
```

Signature-corrupted comparison image: Stage 1 verifies the later-partition AC **before** copying P1 to `0x03000000`. UART must not contain `STAGE1 RSA2048 PARTITION PASS`. `SYS07 HOLD` is not printed.

Private oracle/signing PEMs (`psk.pem`, `ssk.pem`) stay gitignored (`*.pem`, `zynq_stage1/owned/build/oracle/`). This document records public parameters and file hashes only. No PEM armor and no private key bytes.

Owned image: P0 unsigned Stage 1, P1 authenticated KAT at `0x03000000`. Gold `parse()` / `parse_all()` still return `VendorRsa`. No DevCfg. No AES. eFuse = no. No PCAP / System 8.
