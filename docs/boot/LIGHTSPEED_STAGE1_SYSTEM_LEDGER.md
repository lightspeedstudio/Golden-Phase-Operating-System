<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# LightSpeed Studio LLC — Stage 1 Systems 1–12 ledger

Engineering history. Hashes are lowercase SHA-256. `NOT_VERIFIED` means the file is not on this tree to re-hash (often gitignored oracle/BOOT). UART files listed here were re-hashed for the evidence index validator.

Working `main.rs` SHA-256 `6637678d0e01b007caf3b5e150ed37e48ed5af6f55cc009a3b16e0d5c4ebd67d` (9580) is unchanged through this ledger.

## System 1 — working firmware reference lock

| Field | Value |
| --- | --- |
| Source commit | `5d1027bcef372ee2fa5f85392ecc90685061056d` |
| What changed | Locked gold PATH1_G64 artifacts and shortcut Stage 1 sources. No owned replacement ELF board PASS. |
| Host-test | Hash lock table in `SYS01_WORKING_REFERENCE_LOCK.md` |
| Board-test | Gold FSBL boot UART only (not owned Stage 1) |
| BOOT | `7da748547d5016191643ddc4df555a8cf02c1d8adee3235f1dde766166ea0b85` (6114836) gold FSBL image |
| ELF | Utility `447f9fbdec6bbdb03227c8620baa854f3a141d52ec71452251e2f63e1852eaf8` (2475164); FSBL `8b71aa707fd9e200a89cd2b966cf3d3ec26ed0350495718a51a9f8ec259f7874` (577272) quarantined |
| UART | `bf3ca7940c61e22455276b26d218bb7bd44203859e2362a26e263a152e102800` (7475) `com12_path1_g64_combined_20260814_174430.txt` |
| Claim | Reference lock. Gold Utility `e_entry` `0x00100060`. |
| Non-claim | Owned Stage 1 COM12 PASS |

## System 2 — boot-image parse reference

| Field | Value |
| --- | --- |
| Source commit | `d5ffeb1ae9022d16d2a2edff504dcc93a12cb72c` (named in SYS02 doc as System 1 lock commit for the fixture) |
| What changed | Independent parse of gold `BOOT.BIN`. No packer. |
| Host-test | Parser accepts gold fixture |
| Board-test | none (host-only) |
| BOOT / UART | gold BOOT as SYS01; UART n/a |
| ELF | Utility as SYS01 |
| Claim | Owned parser matches Bootgen gold layout |
| Non-claim | Owned packer output |

## System 3 — PS7 / DDR / UART owned ELF

| Field | Value |
| --- | --- |
| Source commit | `8ea707a409b59fddcb2dda7b4ac2097c333baca2` (platform); pack `bc71ec3e744783443875335f476b95e51c29e623`; evidence `659a774` |
| What changed | Recoded PS7/DDR/UART; linked `sys03_plat`; packed `BOOT_RECOVERY_PLAINTEXT.BIN` |
| Host-test | PS7 trace vs working arrays |
| Board-test | **SYS03_COM12_PASS** HOLD after `BOOTMODE SD PASS` |
| BOOT | `46bb693c123df147609cc26f765fa68f7bc935edbea88d5a01f5976c05c713fb` (5992148) |
| ELF | `82c9b1e742291d4613fdf14db938b02edcffef62d4a23d8fc427218eed5277f6` (12496) cited in SYS03_ARTIFACT_HASHES; live `build/sys03_plat.elf` may differ if rebuilt |
| UART | `8fcdc3257f3449a9114594474d0e34d8e71540348081da82336f819f132f0dee` (112) |
| Claim | DDR/UART/WDT/SD-mode on silicon |
| Non-claim | SD load, PCAP, JUMP. Failure class: DDR **ordering** had to match working MIO→PLL→clocks→DDR (not a later DDR FAIL UART; none exists). |

## System 4 — ADMA2 CMD18

| Field | Value |
| --- | --- |
| Source commit | `c2caeef900b3d6d751f8cd89779d5e623bc419a9` (HASHES); feat `1a15fca` |
| What changed | ADMA2+CMD18 first 16 KiB. Wait Transfer Complete, never DMA-INT. |
| Host-test | SD unit tests |
| Board-test | **SYS04_COM12_PASS** `SYS04 HOLD` after ADMA 16384 |
| BOOT | `e966318fb687464c6a8257d110edd71b970ff4bf6c8ed212fbfa0676543fe279` (5997268) |
| ELF | `1683a3891258e8f2f8fe37cd3f94e9f4ad8a68dd3f4ba6239cc57f718ff08ba0` (20656) cited SYS04_ARTIFACT_HASHES |
| UART | `d298dc4bf6cc00031372577df32f9c6069a2b17f73c0f06b4e2a3fcef9f9e78b` (195) |
| Claim | DMA SD read of BOOT.BIN prefix |
| Non-claim | Full-file ADMA, parser. No ADMA-wait FAIL UART exists. |

## System 5 — parse_all 16 KiB window

| Field | Value |
| --- | --- |
| Source commit | `93bb491a6ea3ca764cb064ef2dcd7d0bf112e341` |
| What changed | `IMAGE HEADER PASS` / partition count on 16 KiB window |
| Host-test | parse tests vs gold |
| Board-test | **SYS05_COM12_PASS** `SYS05 HOLD`; `UTILITY ENTRY 00100060`; no JUMP |
| BOOT | `ccfbbbc5a9c7ab573ccad9251dc933ab44d294ee26b7c7eccfaded0cf0955c5f` (5999828) |
| ELF | `f40f7e13a8f141afd404c407803f3de394c4e1ae082ca0ca112a932a53b248a2` (24000) cited SYS05_ARTIFACT_HASHES |
| UART | `b8f7145da7d3cf24c572a06b3e7a61f1b7b6a9a8bc281e366a1502da0665d3d8` (258) |
| Claim | Header parse on card |
| Non-claim | Payload load / PCAP |

## System 6 — AES/HMAC KAT (powered session)

| Field | Value |
| --- | --- |
| Source commit | `de71e058017150caa81ef3828cc8294ce6e879c5`; evidence `88c049a` |
| What changed | Packer + DevCfg `secure_transfer` P1 KAT to DDR |
| Host-test | packer/parse/DevCfg host sequence |
| Board-test | **SYS06_P1_KAT_COM12_PASS** `P1 KAT PASS` `SYS06 HOLD` |
| BOOT | `363a44aa6a0ab3b091f2f5b43dd14242cc8643afc05d237f4a4c28d0a10a5bd8` (6002388) gitignored oracle path; hash from HASHES.txt |
| ELF | `c346065ba2516ce91d454c1180816dfc706a8fe3a80db101665ac76f57e50b01` (27648) cited SYS06_PREFLIGHT |
| UART | `ad6b0106577972effe75e0b6597a980b55242cedbf2f650c8b4cd1b63814c95c` (284) |
| Claim | Powered-session BBRAM AES/HMAC KAT |
| Non-claim | Cold-power retention; encrypted bitstream PCAP; production keys |

## System 7 — software RSA-2048

| Field | Value |
| --- | --- |
| Source commit | `cc88e73e3cc982b8e068b5991249a83df0076a29` (instrumented PASS) |
| What changed | Later-partition software RSA; AC kept off OCM stack |
| Host-test | RSA KATs |
| Board-test | **SYS07_COM12_PASS** `SYS07 HOLD`; incomplete prior UART `6fd8675e…` (329) |
| BOOT | `0192b1007f0efde6f337992490a20b486a46e91542cd26d5a96db14256cb22d1` (29120) gitignored oracle |
| ELF | NOT_VERIFIED (not in HASHES_PASS) |
| UART | `4377e3018a48789f9caf6ac359db87e932314a6c9e6a8471bf3df75669f5063f` (528) |
| Claim | Software PARTITION PASS; BootROM still NOT_ENFORCED |
| Non-claim | Hardware RSA / eFuse |

## System 8 — plaintext PCAP

| Field | Value |
| --- | --- |
| Source commit | PASS `b9e34b4159bfba993f3a3b60225058a8e7700de0`; CRC fail `f76d981`; first PL FAIL `f29b4c4` |
| What changed | FabricInit + PCAP. Fix: convert configuration words to PCAP byte order after `PL FAIL class=CRC`. |
| Host-test | PCAP host model |
| Board-test | **SYS08_COM12_PASS** `PL PASS` `SYS08 HOLD`. Fail UART `97462f04…` / diag `75c20baa…` |
| BOOT | `8d0aefa6f5aef206a1eec6617f988a14aa3ea07733f167c3c51620159f201ef7` (6004948) |
| ELF | NOT_VERIFIED in HASHES_PASS |
| UART PASS | `75ee7277e6d554c47ff2f559d6af0752f0e8b816339752e137b5504053d768a7` (257) |
| Claim | PATH1_G64 `PL PASS` |
| Non-claim | Utility JUMP |

## System 9 — Utility handoff

| Field | Value |
| --- | --- |
| Source commit | `40eaad40e22b49f441989cec5a4ce646e2323336`; UART docs `8b3acf0` |
| What changed | Load Utility, zero PT_LOAD tail, `HANDOFF PASS`, `JUMP 00100060` |
| Host-test | handoff locked_working_state |
| Board-test | **SYS09_COM12_PASS** full `gpos>` |
| BOOT | `eb2f34c3052f9c1a17b0ab34d31385decb0a3cc1c9c75fabacce97e9e63d7b27` (6005972) |
| ELF | `08502fd5fd9f9a5c16f333571a1e9d6f9970bb1d4618ae87a3683e0d4f77c082` (34072) board-passed copy |
| UART | `391218176cef09b7d642b71f5cca9990dec2d91b95cc73c9879185d9dfa0219d` (6994) |
| Claim | Owned Stage 1 → Utility `gpos>` |
| Non-claim | AES/RSA linked in that ELF (SYS10 found `--gc-sections` drop) |

## System 10 — size / provenance (incomplete link set)

| Field | Value |
| --- | --- |
| Source commit | `8017d8e` |
| What changed | Measured SYS09 ELF: AES/RSA not linked. Label **SYS10_INCOMPLETE_LINK_SET**. PT_LOAD 19288. |
| Host-test | size/provenance scripts |
| Board-test | none new (uses SYS09 UART) |
| BOOT / ELF / UART | SYS09 board-passed hashes above |
| Claim | Honest incomplete link set |
| Non-claim | 8 KiB PASS; complete security link |

## System 10A — unified dispatcher (SYS10_OVERSIZE)

| Field | Value |
| --- | --- |
| Evidence commit | `3a8049b` (HASHES_PASS has no `source_commit` field) |
| What changed | One ELF keeps plaintext, AES KAT, software RSA, PCAP, handoff. In-place parse. `wait_tx_empty` before JUMP. |
| Host-test | sys10 / pack / classify |
| Board-test | **SYS10A_COM12_PASS** `gpos>`; size **SYS10_OVERSIZE** (29836 vs 8192) |
| Failures | stack hang UART `01956933…` (185) — 16 KiB parser-bound copy; binary burst `30b77ee0…` (6988) — RSA AC on SVC stack + JUMP before TX empty |
| BOOT | `8b396c6f519935cb8099747db466c2bc7647df0666ece36ccf6d0a261e577874` (6018260) unified |
| ELF | `cbc4e37ad3a8444541db886a040a42dc0c94c9a09102bffb3b51f5280ee26854` (50292) `docs/boot/sys10a/sys10a_plat.elf` |
| UART | `ceb02585476e6b316f94c01c720e4f385ba1f6d86aaf2b1c9a3a67dd3294c64d` (7107) |
| Claim | Unified product `gpos>` with software RSA; BootROM RSA NOT_ENFORCED |
| Non-claim | 8 KiB; hardware RSA |

## System 11 — file-only UART classifier

| Field | Value |
| --- | --- |
| Source commit | `a39ac187aca0c0988131dc03dd36b352deb48d68` |
| What changed | Host `sys11::classify` from real UART fixtures. No COM port. |
| Host-test | `cargo test --features std --offline --lib sys11` (7 passed at that commit) |
| Board-test | none |
| BOOT / ELF | n/a (classifies UART bytes) |
| UART fixtures | SYS10A product, SYS09 recovery, SYS06 AES, real fails — see SYS11_UART_REFERENCE |
| Claim | Fail-closed Product / Recovery / Aes; reject `JUMP 00100020` |
| Non-claim | Live capture |

## System 12 — plaintext recovery vs gold

| Field | Value |
| --- | --- |
| Source commit (HASHES_PASS) | `a39ac187aca0c0988131dc03dd36b352deb48d68` |
| Evidence commits | `53100ef` UART; `33ef254` freeze BIN |
| What changed | Board comparison of SYS10A recovery image vs gold/SYS09 UART contract. No device-code change. |
| Host-test | sys11 Recovery on SYS12 UART |
| Board-test | **SYS12_COM12_PASS** |
| BOOT | `738c6a0155d9558c37d51fffe11bc99236c4c29242caa07fa860756f47d24352` (6016212) |
| ELF | same sys10a_plat as 10A `cbc4e37a…` |
| UART | `6236336b06b9b4cb70fc098f8616bb91b7b790864dd53e0bc5d73b90bb5cf9d5` (7010) |
| Claim | Recovery `PL PASS` `HANDOFF PASS` `JUMP 00100060` `gpos>`; gold splash times and PL IDs match |
| Non-claim | AES flash; production keys |

## Systems 13–15

**DEFERRED_CUSTOM_HARDWARE.** [`DEFERRED_PRODUCTION_SECURITY_HARDWARE.md`](DEFERRED_PRODUCTION_SECURITY_HARDWARE.md). Not in this ledger as started work.
