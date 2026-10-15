<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# Stage 1 size, completeness, and provenance — System 10A

Status: **SYS10_OVERSIZE**

Unified product Stage 1: one `rust_main` dispatches plaintext recovery, encrypted P1 AES/HMAC (`p1_decrypt_kat` → `secure_transfer`), and later-partition software RSA, then PATH1_G64 PCAP and Utility handoff. Security was not stripped to hit 8192. System 11 is not started.

System 9 board-passed measurement remains **SYS10_INCOMPLETE_LINK_SET** (`docs/boot/sys10/`, `docs/boot/SYS10_*.txt`). That ELF's `--gc-sections` drop of AES/RSA is why 10A exists.

## Labels

| Label | Meaning |
| --- | --- |
| `SYS09_BOARD_PASSED` | Packed `BOOT_SYS09_HANDOFF.BIN` SHA-256 `eb2f34c3…e63d7b27` (6005972). PT_LOAD 19288. AES/RSA not linked. |
| `SYS10A_UNIFIED` | `cargo build --release --offline --target armv7a-none-eabi --bin sys10a_plat --features device,sys10a` then `sys10a_pack`. |

## 1. Reference lock

| Item | Value |
| --- | --- |
| Branch | `feat/stage1-sys10a-unified-v3` |
| System 9 source / evidence | `40eaad4` / `8b3acf0` |
| System 10 host measurement | `8017d8e` |
| `zynq_stage1/src/main.rs` | `6637678d0e01b007caf3b5e150ed37e48ed5af6f55cc009a3b16e0d5c4ebd67d` (9580) — not edited |
| Utility `e_entry` | `0x00100060` |
| PATH1_G64 `system.bit` | `d19f08d20cac85ea0e33c41a8a552166f5a9176e16977f09aa64d17e68135829` (4045769) |
| PATH1_G64 `kairos_zynq.elf` | `447f9fbdec6bbdb03227c8620baa854f3a141d52ec71452251e2f63e1852eaf8` |
| Gold `BOOT.BIN` (FSBL image, not SYS10A) | `7da748547d5016191643ddc4df555a8cf02c1d8adee3235f1dde766166ea0b85` |

`SYS10A_UNIFIED` ELF: `docs/boot/sys10a/sys10a_plat.elf` SHA-256 `cbc4e37ad3a8444541db886a040a42dc0c94c9a09102bffb3b51f5280ee26854` (50292). PT_LOAD payload SHA-256 `8e4254ead8617071ec8c768272b2ae347b2caaf629aeb67298dbd49f6097ef42` (29836).

Packed unified BOOT SHA-256 `8b396c6f519935cb8099747db466c2bc7647df0666ece36ccf6d0a261e577874` (6018260): unsigned P0, RSA-authenticated 256-byte KAT, plaintext PATH1_G64 bitstream, plaintext Utility. Recovery image `BOOT_SYS10A_RECOVERY.BIN` SHA-256 `738c6a0155d9558c37d51fffe11bc99236c4c29242caa07fa860756f47d24352` (6016212) is the three-partition plaintext path on the same ELF.

## 2. Toolchain

Same as System 9: rustc 1.89.0, `armv7a-none-eabi`, in-tree Arm GNU Toolchain 13.3.Rel1, `linker.ld`, `--gc-sections`, LTO, `opt-level = "s"`. Features: `device,sys10a`. Map filename remains `build/sys03_plat.map`.

## 3. PT_LOAD / size gate

8 KiB gate = **8192** bytes.

`SYS10A_UNIFIED` ELF program headers:

| Type | File off | vaddr | paddr | `p_filesz` | `p_memsz` | flags | align |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| PT_LOAD | `0x001000` | `0x00000000` | `0x00000000` | `0x748C` (29836) | `0x7490` (29840) | `RW E` | `0x1000` |
| GNU_STACK | 0 | 0 | 0 | 0 | 0 | RW | `0x10` |

GNU_STACK is not added into the 8 KiB sums.

| Metric | Bytes | vs 8192 |
| --- | ---: | --- |
| Sum of PT_LOAD `p_filesz` (loaded-file) | **29836** | over |
| Sum of PT_LOAD `p_memsz` | **29840** | over |
| Occupied OCM span `_start`..`_stack_top` (`0x0`..`0x9490`) | **38032** | over |
| Complete ELF file size | 50292 | **not OCM size** |

`p_filesz` is 29836; `p_memsz` is 29840 (4-byte `.bss`). They are not interchangeable. The 8 KiB loaded-file metric uses `p_filesz` only. Incomplete SYS09 loader was already 19288; this complete loader is 29836. Both are honest overs.

Device classify parses the 16 KiB header window in place at `BOOT16_PHYS`. A 16 KiB stack copy overflows the 8 KiB SVC stack and hung after `SD ADMA2 CMD18 PASS` on the first COM12 attempt.

Device `p1_rsa_kat` verifies the 1728-byte AC in the DDR image window (SYS07 traced path). Copying that AC onto the SVC stack smashed `.rodata` after the RSA UART line; COM12 then showed `0xFF` through JUMP remnant `00100060`. `uart::wait_tx_empty` waits `SR.TXEMPTY` and `SR.TACTIVE` clear so `HANDOFF PASS` and `JUMP 00100060` finish shifting before `handoff::jump`.

Boot header (unified image): Source Offset `0x1700`, Length of Image 29836, partition 0 `DataWordLen` = `ImageWordLen` = 7459.

## 4. Required-feature linkage

`_start` → `KEEP_RUST_MAIN` → `sys10a::rust_main`. Runtime `classify()` on the loaded BOOT.BIN selects the arm. `--gc-sections` therefore keeps every arm.

| # | Feature | Linked | Reachable from `_start` | Notes |
| --- | --- | --- | --- | --- |
| 1 | PS7, UART, DDR, WDT, boot-mode | yes | yes | inlined into `rust_main` |
| 2 | SD ADMA2 + FAT BOOT.BIN | yes | yes | `read_blocks_adma`, `FatVol` |
| 3 | Independent parser | yes | yes | `parse_all_product`, `parse_all_encrypted`, `parse_all_rsa_later` |
| 4 | DevCfg AES/HMAC | yes | yes | `p1_decrypt_kat` (700) calls `secure_transfer`; LTO inlined the latter (no unique nm name) |
| 5 | Software later-partition RSA-2048 | yes | yes | `p1_rsa_kat` (1268), `verify_pkcs1_sha256_digest`, `modexp` |
| 6 | PCAP PATH1_G64 | yes | yes | `pcap_pl` (2156) |
| 7 | Utility load + `handoff::jump` | yes | yes | `working_entry`; `jump_app` waits TX empty |
| 8 | Plaintext recovery | yes | yes | `Kind::Plaintext` → PCAP + handoff |
| 9 | Encrypted image dispatch | yes | yes | `Kind::Encrypted` → `p1_decrypt_kat` |
| 10 | RSA product dispatch | yes | yes | `Kind::RsaProduct` → RSA then PCAP then handoff |

Host tests execute the AES arm (`p1_decrypt_kat` + RATE_EN) and the RSA arm (`p1_rsa_kat` + KAT compare) plus classify of plaintext / encrypted / RSA-product / RSA-KAT images. Silicon AES of an encrypted bitstream still needs BootROM-decrypted Stage 1 and a live BBRAM key; this COM12 product uses software RSA on a later partition with plaintext Stage 1 / PL / Utility.

**SYS10_OVERSIZE**: complete linked set; loaded-file 29836 > 8192.

## 5. Provenance

Map archive members: 201 LightSpeed owned (`sys10a_plat` / `zynq_stage1_owned`), 217 `compiler_builtins`, 0 unknown.

Not present: `kairos` crate, Xilinx FSBL object, Bootgen object, Xilinx BSP, PEM or private-key blocks in tracked SYS10A reports, `zynq_fsbl.elf` as Stage 1. `[dependencies]` remains absent from `zynq_stage1/owned/Cargo.toml`.

## 6. COM12 close

UART `com12_sys10a.uart` SHA-256 `ceb02585476e6b316f94c01c720e4f385ba1f6d86aaf2b1c9a3a67dd3294c64d` (7107): `DISPATCH RSA` → `STAGE1 RSA2048 PARTITION PASS` → `PCAP BEGIN` → `PL PASS` → `HANDOFF PASS` → `JUMP 00100060` → `gpos>`.

System 10 is closed as **SYS10_OVERSIZE**. System 11 is not started.

## 7. Non-claims

- Not `SYS10_SIZE_PASS`
- Not a size-reduction change
- Not System 11
- Not BootROM RSA enforcement
- Not eFuse
- Encrypted PL on this COM12 card is not claimed; AES is linked and host-proven
