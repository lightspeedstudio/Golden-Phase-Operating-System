<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# System 6 packer — owned encrypted `BOOT.BIN` (host)

Status: **SYS06_PACKER_ORACLE_IMAGE**

Host-std encrypt + pack only. This commit does **not** close System 6. System 6 closes only after integration and silicon PASS.

6A stays `SYS06A_FRAMING_UNRESOLVED` (NIST CBC host recover of the locked oracle is still open). 6B is unchanged. Working Stage 1 (`zynq_stage1/src/main.rs`, hash `6637678d0e01b007caf3b5e150ed37e48ed5af6f55cc009a3b16e0d5c4ebd67d`, 9580 bytes) is not edited.

No BBRAM program. No card write. No COM12. No RSA. Keys stay gitignored and are never printed.

## What was opened (behavioral reference only)

Public Bootgen (Apache-2.0, `https://github.com/Xilinx/bootgen`) was cloned read-only to a temp tree. **No vendor file was copied into `zynq_stage1/owned`.** Vitis 2026.1 still ships `bootgen.bat` as a binary; the stream writer is not in that install.

| Artifact | Path in clone | SHA-256 | Bytes |
| --- | --- | --- | ---: |
| Zynq stream encrypt | `zynq/src/encryption-zynq.cpp` | `2ea6f77cd771712bd2e80b64ad9f6c9f34ec1a8425c9136bbb753474420b1942` | 62543 |
| Zynq encrypt header | `zynq/include/encryption-zynq.h` | `9279feaca89a59d4605e68035802a5fb04f5aac05a248220df0be19c19f77e97` | 5472 |
| Stream sizes / Type1 table | `utils/include/encryptutils.h` | `6a8b98d34fffe79040c6986bf7c07732c1e96b2bdf4e145b2fd8ebcc91658964` | 5613 |
| LE/BE data mover | `utils/include/systemutils.h` | `54438c202e6b1154c54f38b760713551e96cb92e1da31c7bede5fd551107f4af` | 9974 |
| Enable / CBC offsets | `common/include/encryption.h` | `28faa16b6b2c18a75db88839feb411c03b881d7a8a0b0fc21b6c5f117507e464` | 7926 |
| Bitstream strip | `common/src/bitutils.cpp` | `8981387feab9309014326cb76a6abaa38ce69cf6aceb2e105a08399fdee888ec` | 12554 |
| Bitstream types | `common/include/bitutils.h` | `8694ba786e0fec89dd9d5e967f3aa2300359cd1a69957d2e69038dfece4bb125` | 16028 |
| Zynq image header / BIT strip call | `zynq/src/imageheadertable-zynq.cpp` | `112026d2959f1a244d5489959611a7083f71824c58c18a88710d232747091d78` | 25468 |

UG585 / UG470 (no local PDF hash): [boot-image decrypt/auth](https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/Boot-Image-and-Bitstream-Decryption-and-Authentication), [PCAP bridge](https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/PCAP-Bridge-to-PL), [configuration packets](https://docs.amd.com/r/en-US/ug470-7series-config).

## Independent rewrite (not a source copy)

Zynq-7000 Bootgen does **not** emit NIST AES-256-CBC. That is why 6A OpenSSL/Cryptography KATs failed. The owned host engine is:

1. **AES-256** (FIPS-197) on 16-byte blocks, key loaded as big-endian 32-bit words from `.nky` `Key 0`.
2. **Zynq little-endian data mover** for plaintext and ciphertext bytes.
3. **First AES block:** `LE_plain XOR BE_IV`, then reverse all 32 bits of each word, then AES.
4. **Later blocks:** CBC in the bit-reversed domain (`state ^= reverse_bits(LE_plain)`), AES, then `reverse_bits` on each output word before LE store.
5. **Authentication stream** is assembled in software (ipad, user, SHA pad, 256-byte zero gap, opad, digest gap, pad2, tag) and the **entire** stream is AES'd. Hardware HMAC during PCAP recomputes and compares. The digest-gap 32 bytes in the file are zeros.

Zynq `Process()` always uses the little-endian mover (7-series FPGA bitstream encrypt is big-endian; that path is not this packer).

### Plaintext stream header is 140 bytes

UG470 Type1 words, same offsets 6A already checks:

| Off | Word | Role |
| --- | --- | --- |
| 0x000–0x014 | dummy / bus-width `0xBB` / `0x11220044` | preamble |
| 0x018 | `0xAA995566` | sync |
| 0x020 | `0x3000C001` | MASK |
| 0x024 | `0x00000040` | encryption enable |
| 0x028 | `0x3000A001` | CTL |
| 0x02C | `0x00000040` | encryption enable (BBRAM; bit 31 clear) |
| 0x070 | `0x30016004` | write 4 IV words |
| 0x074–0x083 | IV | `.nky` `StartCBC` with each 32-bit word byte-reversed |
| 0x084 | `0x30034001` | write 1 length word |
| 0x088 | u32le | AES-stream byte length / 4 |

### 6A “204-byte header” = 140 + encrypted ipad

6A first-diff of two same-length KATs is at file offset **204**. The GitHub plaintext header is **140**. Encrypted ipad is 64 bytes and is payload-independent. `140 + 64 = 204`.

`frame.rs` `STREAM_HEADER_LEN = 204` stays the 6A split (prefix before payload-dependent cipher). Length formula unchanged:

```
user = is_pl ? align64(unenc) : align64(unenc + 8)
enc  = 204 + user + 480 + 64
```

480 after the user stream is: pad1 64 + zero gap 256 + opad 64 + digest gap 32 + pad2 32 + tag 32.

PS user stream: 8-byte PCAP prefix (`0x3001E000`, `0x50000000 | payload_words`) ‖ payload ‖ `0x20000000` pad to 64. PL: no prefix; payload pad to 64.

### SHA pads and word SHA-256

Pad1 is one 64-byte SHA block (`0x80000000` then zeros, 64-bit **bit** length of `ipad+user` at the end, LE word stores). Pad2 is 32 bytes for `opad+first_digest`. Digest words are stored little-endian. That matches the Zynq LE mover, not FIPS byte-SHA over the same pad bytes.

### Trailer

16 words `0x20000000`. Public GitHub also adds extra NOOPs to 32-byte-align the **blob length**. Locked 6A sizes are `enc % 32 == 12` (example: 256-byte KAT → 1068). Extra GitHub pad would break those sizes. This packer follows **6A measured 16 NOOPs only**. Next partition start is still `align32(previous_end)` (`frame::PART_ALIGN`).

### Boot header / partition header

- `0x028` = `0x3A5C3C5A` (BBRAM)
- `0x034` = unencrypted Stage 1 bytes (`DataWordLen * 4`)
- `0x040` = encrypted Stage 1 bytes (`ImageWordLen * 4`)
- Encrypted iff `ImageWordLen != DataWordLen`
- Gold `parse()` still rejects that as `ParseError::Encrypted`. Encrypted parse is `parse_allow_encrypted`.

### PL bitstream

Encrypted Zynq PL: parse the x4k `.bit` `e` payload, skip dummy/bus-width words through `0xAA995566`, copy UG470 Type1/Type2 packets (BE words stored LE) until CMD DESYNCH/REBOOT, drop those two words, pad to 32 bytes with `0x20000000`. `presBitHdrSize = 0` (140-byte stream header, same 204-byte 6A prefix as PS). Raw `system.bit` is not the user stream.

## Proof this commit makes

Host tests (no secrets in logs):

- FIPS-197 AES-256 block KAT
- `predict_enc_bytes` / `split_encrypted_blob` / 16×NOOP trailer
- two same-length payloads share `[0,204)` then differ
- decrypt of the user stream recovers the payload
- `pack_secure` image passes `check_encrypted_image`; gold `parse()` stays `Encrypted`

Locked oracle (`BOOT_SYS06_ORACLE.BIN` + gitignored `.nky`), when present:

| Partition | Result |
| --- | --- |
| P1 KAT 256 bytes | byte-identical encrypt (1068) |
| P0 Stage 1 PT_LOAD 13380 bytes (`sys05_plat.elf`) | byte-identical encrypt (14188); `0x034`/`0x040`/`0x1700`/`0x4E80`; independent decrypt recovers the PT_LOAD; modified cipher, tag, header, and length are refused |
| P2 PATH1_G64 | strip `system.bit` → 4043904; byte-identical encrypt (4044652) at `0x52C0`; next `0x3E0A40`; independent decrypt recovers the stripped bitstream; modified cipher, tag, header, and length are refused |
| Full `BOOT.BIN` | encrypted P0+P1+P2 + plaintext Utility PT_LOAD at load `0x00100000` / exec `0x00100060`; owned image is byte-identical to `BOOT_SYS06_ORACLE.BIN` (6,000,404 bytes); gold `parse()` stays `Encrypted`; `parse_allow_encrypted` recovers P0/P1/P2 and matches P3; PH checksum tamper and P0 auth tamper fail closed |

That is Bootgen 2026.1 identity for the complete four-partition image. It is **not** silicon AES PASS and **not** System 6 closed. Header geometry used by the owned assembler (measured on the locked oracle, not guessed): IHT count 4, image-header names from the BIF basenames, unused IH word 8 is `0` when the 16-byte name occupies word 3 else `0xFFFFFFFF`, null partition-header terminator, `0xFF` fill from last IH to PH and from terminator to `0x1700`, payload alignment 32 with `0xFF` fill in the align gap (not zeros).

## Non-claims

- Not System 6 closed
- Not BBRAM / eFUSE programming
- Not COM12 / SD write
- Not 6B API wiring from Stage 1
- Not System 7 RSA
- Not FabricInit (System 8)
- Not a wrapper around `bootgen.bat`
