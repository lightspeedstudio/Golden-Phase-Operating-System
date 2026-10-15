<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# System 6A — Bootgen encrypted-partition framing

Status: **SYS06A_FRAMING_UNRESOLVED**

For System 6A, use an external host-only crypto verifier such as OpenSSL or Python Cryptography to decrypt and verify the fixtures. Do not add owned AES/HMAC implementation code yet.

Do not copy AMD/Xilinx licensed source into this crate. Inspect it as a behavioral reference and rewrite independently.

Read-only host parser. No owned packer, no DevCfg, no BBRAM, no COM12. Keys stay gitignored. `H:\BOOT.BIN` is not modified.

Locked oracle: `zynq_stage1/owned/build/oracle/BOOT_SYS06_ORACLE.BIN`  
SHA-256 `684d97f4a07826f6ec9a94b2d0fc2dfd710514a1ad96a0a1dac4fcce855e21de`, 6,000,404 bytes.  
Bootgen v2026.1 (`D:\Xilinx\2026.1\Vitis\bin\bootgen.bat`), `-arch zynq -encrypt bbram -p xc7z020clg400`, same gitignored `.nky` as System 6.

## Manufacturer reference audit (required before more ciphertext diffs)

Installed Vitis/Vivado 2026.1 was read as a behavioral reference. No vendor file was copied into `zynq_stage1/owned`. Ciphertext-difference guessing is **paused** until this audit is complete and any remaining gap is named.

### What was opened

| Artifact | Path / URL | Applies to Zynq-7000 AES-CBC/HMAC? |
| --- | --- | --- |
| zynq_fsbl | `D:\Xilinx\2026.1\Vitis\data\embeddedsw\lib\sw_apps\zynq_fsbl\src\` (`image_mover.c`, `pcap.c`, `pcap.h`, `fsbl.h`) | yes — this is the post-BootROM consumer |
| DevCfg driver | `...\XilinxProcessorIPLib\drivers\devcfg_v3_9\src\` (`xdevcfg.c`, `xdevcfg.h`, `xdevcfg_hw.h`) | yes — PCAP DMA + AES rate bit + HMAC ISR bit |
| DevCfg examples | `...\devcfg_v3_9\examples\` | no encryption; bitstream download / register dump only |
| xilsecure 5.7 | `...\lib\sw_services\xilsecure_v5_7\` | **no** — Versal/ZynqMP AES-GCM and HMAC-SHA3, not Zynq-7000 CBC |
| Bootgen install | `D:\Xilinx\2026.1\Vitis\bin\bootgen.bat` launches binary `bootgen`; `Vitis\data\bootgen` is Versal CSV lists, not C++ source | binary only |
| UG1283 2026.1 | [Encryption and Authentication](https://docs.amd.com/r/en-US/ug1283-bootgen-user-guide/Encryption-and-Authentication), [Zynq-7000 boot](https://docs.amd.com/r/en-US/ug1283-bootgen-user-guide/Zynq-7000-SoC-Boot-and-Configuration) | AES-CBC+HMAC for 7-series; `.nky` shape; **no** stream-header byte map |
| UG585 | [Boot image decrypt/auth](https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/Boot-Image-and-Bitstream-Decryption-and-Authentication), [PCAP bridge](https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/PCAP-Bridge-to-PL), [Secure PS/PL images](https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/Secure-Bitstreams-and-Software-Boot-Images) | AES-256-CBC + HMAC-SHA256 in the PL; PCAP is the software path |

### Independent rewrite of manufacturer behavior (not a source copy)

1. **FSBL does not run software AES or HMAC.** Encrypted PS partitions are decrypted by feeding the blob through DevCfg PCAP into the PL AES/HMAC unit. HMAC failure is a DevCfg ISR bit (`PCFG_HMAC_ERR`), not a software tag compare.

2. **Encrypted flag** is `ImageWordLen != DataWordLen` (owned names: `enc_words != data_words`). Same test this parser already uses.

3. **PCAP DMA lengths** for secure PS decrypt: source length = `ImageWordLen` (on-flash encrypted words), destination length = `DataWordLen` (post-decrypt payload words), same buffer. The engine consumes the extra encrypted words (header, HMAC material, trailer) and writes only the unencrypted payload. Secure PL download uses PCAP write with destination `0xFFFFFFFF` (UG585).

4. **Quarter-rate:** UG585 and DevCfg set `QUARTER_PCAP_RATE_EN` for encrypted transfers because the AES unit accepts 8 bits per PCAP clock on a 32-bit interconnect. Not a framing byte.

5. **Loopback:** non-secure concurrent DMA sets PCAP loopback; **secure** concurrent DMA clears loopback so the stream goes through AES/HMAC. Software AES libraries are not on this path.

6. **Authenticated range (manufacturer):** HMAC-SHA256 authenticates the decrypted PS image / PL bitstream inside the PL while PCAP moves `ImageWordLen` words. Software never names HMAC key/tag file offsets. After a successful transfer, the visible payload length is `DataWordLen` words. Inner HMAC key/tag placement is **not** documented in the installed FSBL, DevCfg, xilsecure, UG1283 encryption chapter, or DevCfg examples.

7. **Bootgen stream writer** (the code that emits the 204-byte header and HMAC stream) is **not** in the 2026.1 install — only the `bootgen` binary. That source is the remaining manufacturer artifact for KAT recover on the host. It must be audited next, still without copying it into the owned crate.

Host Cryptography/OpenSSL remains the 6A KAT/HMAC *host* verifier. It is not what FSBL does. Do not start a packer, DevCfg driver, System 6B, BBRAM, card, COM12, or System 7 from this audit.

## Measured (independent of Bootgen `-read`)

Controlled mini-oracles: encrypted `sys05_plat.elf` + encrypted KAT only. Sizes 15, 16, 17, 31, 32, 33, 255, 256, 257 bytes, plus a second 256-byte KAT filled `0xA5`. Commands and hashes: `docs/boot/SYS06A_FIXTURE_MANIFEST.txt`. Fixtures are gitignored.

### Stream layout of an encrypted partition blob

| Range | Role | How obtained |
| --- | --- | --- |
| `[0, 204)` | plaintext stream header | 256-byte KAT vs `0xA5` KAT: first difference at byte 204 |
| `[204, n−64)` | 16-byte-aligned cipher region | that range changes with payload; length `% 16 == 0` |
| `[n−64, n)` | 16 words `0x20000000` | counted on every encrypted partition including locked P0/P1/P2 |

`n % 16 == 12` because `204 % 16 == 12` and cipher + trailer are 16-aligned.

### Stream header fields (non-secret)

UG470 Type1 config packets, measured offsets:

| Off | Word | Meaning |
| --- | --- | --- |
| 0x018 | `0xAA995566` | width / dummy (same constant as boot header `0x020`) |
| 0x020 | `0x3000C001` | Type1 write MASK |
| 0x024 | `0x00000040` | encryption-enable mask bit (UG585 `ENCRYPTION_ENABLE`) |
| 0x028 | `0x3000A001` | Type1 write CTL |
| 0x02C | `0x00000040` | encryption enable |
| 0x070 | `0x30016004` | Type1 write 4 words (IV) |
| 0x074–0x083 | 16 bytes | CBC IV. Equals `.nky` `Key StartCBC` with each 32-bit word byte-reversed. Values not recorded. |
| 0x084 | `0x30034001` | Type1 write 1 word (length) |
| 0x088 | u32le | `(cipher_len + 64) / 4` |

Bytes `0x08C..0xCB` (offset 140–203) are payload-independent and are not interpreted. They are not printed here.

### Length formula (fits every measured PS and PL partition)

`unenc = align4(payload_len)` (Bootgen stores word-padded plaintext length).

```
user = is_pl ? align64(unenc) : align64(unenc + 8)
enc  = 204 + user + 480 + 64
```

The `+ 8` is the measured PS prefix **inside the AES plaintext**, before the KAT. Same-`unenc` fixtures first differ at cipher byte

`204 + 16 * ((8 + payload_first_diff) / 16)`

| Pair | payload_first_diff | first cipher diff |
| --- | ---: | ---: |
| 15 vs 16 | 15 | 220 |
| 31 vs 32 | 31 | 236 |
| 255 vs 256 | 255 | 460 |
| 256 vs `0xA5` | 0 | 204 |

Different `unenc` (16 vs 17, 32 vs 33, 256 vs 257) first differ at **204**, so the 8-byte prefix carries a length. Prefix bytes were not decrypted.

`480` is the measured remainder after header, 64-byte-padded user stream, and trailer. Inner split of that remainder (HMAC key vs tag vs SHA pads) is **not** known. Two same-length KATs (256 vs `0xA5`) differ at byte 204, so the first AES block is payload-dependent: HMAC ipad is **not** the first 64 bytes of the cipher region.

Checks:

| Payload | unenc | enc predicted | enc measured |
| ---: | ---: | ---: | ---: |
| 15–16 | 16 | 812 | 812 |
| 17 | 20 | 812 | 812 |
| 31–32 | 32 | 812 | 812 |
| 33 | 36 | 812 | 812 |
| 255–256 | 256 | 1068 | 1068 |
| 257 | 260 | 1068 | 1068 |
| Stage 1 13380 | 13380 | 14188 | 14188 (`0x040`) |
| PL 4043904 | 4043904 | 4044652 | 4044652 |

UG585: `0x034` is post-decrypt Stage 1 (= P0 `unenc`). `0x040` is on-flash Stage 1 (= P0 `enc`).

### Source offsets

P0 is `0x1700` (gold / UG585 Source Offset). Each following partition starts at `align32(previous_end)`. Locked: `0x1700`, `0x4E80`, `0x52C0`, `0x3E0A40`. Mini KAT images: P1 at `0x4E80`.

### Key / IV / word transformations (host Cryptography)

Measured, no secret values:

| Item | Rule |
| --- | --- |
| Stream-header IV at +116 | `.nky` `Key StartCBC` with each 32-bit word **byte-reversed** |
| IV reuse | P0, P1, and P2 of the locked oracle share that 16-byte IV field |
| Header bytes 140–203 | Identical on every encrypted partition of this `.nky`. Not printed (may be key-derived). |
| Header diffs P0 vs P1 | Only the length word at +136 |
| AES key in the file | None. `.nky` `Key 0` is 32 bytes for BBRAM; it is not in the partition blob |

Tried with Python Cryptography / OpenSSL AES-256 (not owned AES), still **no KAT**:

- NIST AES-256-CBC using `.nky` `Key 0` and header/`StartCBC` IVs, with and without 32-bit byte-reverse on the key
- NIST AES-256-ECB with 32-bit **bit-reverse** wrapping (FPGA-shaped CBC)
- 7-series 32-bit **byte-swap plus per-byte bit-reverse** on key, IV, and ciphertext (standard-library mapping described for 7-series bitstreams), including ECB-wrap CBC in both chaining domains

The mapping from `.nky` key bytes to the NIST AES-256 key schedule that Bootgen 2026.1 actually used is **unresolved**.

### Authenticated byte range (measured bounds)

HMAC-SHA256 is not evaluated until KAT recover succeeds. Bounds from differentials and UG585 (AES and HMAC run in tandem on decrypted words):

| Range in partition blob | In AES? | Payload-dependent? | HMAC? |
| --- | --- | --- | --- |
| `[0, 204)` stream header | no | only the length word at +136 | no (plaintext header) |
| `[204, 204+user)` | yes | yes (PS prefix + KAT; first-diff rule above) | inferred HMAC **message** |
| `[204+user, n−64)` = 480 bytes | yes | yes (CBC continues) | HMAC **key/tag/SHA pads**; inner split unknown |
| `[n−64, n)` NOOP trailer | no | no | no |

`user = align64(unenc + 8)` (PS) or `align64(unenc)` (PL). Parser helpers: `user_stream_bytes`, `hmac_remainder_off`.

Inferred HMAC message is the 64-byte-padded user stream inside the AES plaintext (8-byte PS prefix + payload + `0x20000000` pad). That is **not** independently verified: the 480-byte remainder’s tag offset is unknown, and ipad is not at the start of the cipher (256 vs `0xA5` first-diff 204).

Do not start a packer or System 6B from these bounds.

## Host verifier (no owned AES/HMAC)

For System 6A, use an external host-only crypto verifier such as OpenSSL or Python Cryptography to decrypt and verify the fixtures. Do not add owned AES/HMAC implementation code yet.

KAT recover and HMAC check run **outside** the owned crate:

`zynq_stage1/owned/host_verify/sys06a_cryptography.py`

AES-256 is Python Cryptography (OpenSSL backend). HMAC-SHA256 is hashlib. The script never prints key, IV, HMAC, or payload bytes. Rust tests only assert the script’s PASS/FAIL lines.

Tried on the locked oracle and the mini KAT fixtures, using the private test `.nky`:

- NIST AES-256-CBC (`Cipher(AES, CBC)`; same algorithm as `openssl enc -aes-256-cbc -nopad`)
- NIST AES-256-ECB with 32-bit bit-reverse wrapping around that engine
- 7-series 32-bit byte-swap plus per-byte bit-reverse on key / IV / ciphertext
- `.nky` `Key 0` / `StartCBC` and the stream-header IV (word-swapped and raw)

None of those mappings recover the KAT. HMAC-SHA256 is therefore not evaluated against a decrypted stream. Ciphertext-byte mutation still **parses** in `frame.rs`; the host verifier reports `KAT_MUTATION_STILL_ABSENT`.

Do not add AES or HMAC source to `zynq_stage1/owned`. Do not start a packer.

## Unresolved (stop)

| Item | Unresolved range |
| --- | --- |
| AES-CBC plaintext mapping | `[file_off+204, file_off+n−64)` of each encrypted partition |
| `.nky` key → NIST AES-256 schedule | unknown 32-bit word / bit order |
| HMAC key placement | unknown offset inside `[204+user, n−64)` (480 bytes) |
| HMAC tag placement | unknown offset inside `[204+user, n−64)` (480 bytes) |
| Authenticated message bytes | UG585: HMAC-SHA256 over the decrypted PS/PL image inside the PL during PCAP of `ImageWordLen`; software-visible payload is `DataWordLen`. Host tag offset still unknown |
| Bootgen stream-writer source | **not installed** (binary only); required before more ciphertext guessing |
| KAT byte-for-byte recover | not achieved (host Cryptography/OpenSSL; FSBL uses PL AES) |
| One-byte ciphertext/tag auth fail | framing still parses; host verifier has no tag |

Parser: `zynq_stage1/owned/src/bootimg/frame.rs`. Constant `SYS06A_STATUS`.
