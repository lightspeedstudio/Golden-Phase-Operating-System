<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# LightSpeed Studio LLC — Stage 1 evidence index

Board: PZ7020-SL-C. UART: COM12 115200 8N1 DTR off RTS off (USB-SERIAL CH340). Card serial `121220160204` where noted. Raw `.uart` / gold `.txt` are board proof. `SAW_GPOS` and `=== COM12` are host banners, not UART.

## Final development-hardware proof (find this first)

| Item | Path | SHA-256 | Bytes |
| --- | --- | --- | ---: |
| Evidence dir | `fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS12_20260820/` | — | — |
| Raw UART | `…/com12_sys12.uart` | `6236336b06b9b4cb70fc098f8616bb91b7b790864dd53e0bc5d73b90bb5cf9d5` | 7010 |
| Host meta (not UART) | `…/com12_sys12.meta.txt` | — | — |
| BOOT (frozen) | `fpga/zynq/release/PZ7020_STAGE1_PLAINTEXT/BOOT.BIN` | `738c6a0155d9558c37d51fffe11bc99236c4c29242caa07fa860756f47d24352` | 6016212 |
| BOOT backup | `…/BOOT_SYS12_PLAINTEXT.BIN` | `738c6a0155d9558c37d51fffe11bc99236c4c29242caa07fa860756f47d24352` | 6016212 |
| ELF | `docs/boot/sys10a/sys10a_plat.elf` | `cbc4e37ad3a8444541db886a040a42dc0c94c9a09102bffb3b51f5280ee26854` | 50292 |
| Source / evidence | HASHES `a39ac18`; UART commit `53100ef`; freeze `33ef254` | — | — |
| Classifier | Recovery (`CLASSIFIER_PASS.txt`) | — | — |
| Class | live hardware, recovery evidence | — | — |

Required UART lines: `PL PASS`, `HANDOFF PASS`, `JUMP 00100060`, `gpos>`.

## Gold FSBL comparison (not owned Stage 1)

| Item | Value |
| --- | --- |
| Dir / file | `fpga/zynq/evidence/kairos_zynq_boot/com12_path1_g64_combined_20260814_174430.txt` |
| UART SHA-256 | `bf3ca7940c61e22455276b26d218bb7bd44203859e2362a26e263a152e102800` (7475) |
| BOOT | `fpga/zynq/release/PZ7020_G1P_PATH1_G64/BOOT.BIN` `7da748547d5016191643ddc4df555a8cf02c1d8adee3235f1dde766166ea0b85` (6114836) |
| ELF | Utility `447f9fbdec6bbdb03227c8620baa854f3a141d52ec71452251e2f63e1852eaf8`; FSBL `8b71aa707fd9e200a89cd2b966cf3d3ec26ed0350495718a51a9f8ec259f7874` |
| Class | live hardware, vendor FSBL gold |

## System directories (owned Stage 1)

| Dir | UART file | UART SHA-256 | BOOT SHA-256 | ELF SHA-256 | Commit | Classifier / status | Class |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `BOOT_STAGE1_SYS03_20260819/` | `com12_sys03.uart` | `8fcdc3257f3449a9114594474d0e34d8e71540348081da82336f819f132f0dee` | `46bb693c123df147609cc26f765fa68f7bc935edbea88d5a01f5976c05c713fb` | `82c9b1e7…` cited HASHES | `8ea707a` / `659a774` | SYS03 HOLD PASS | live hardware |
| `BOOT_STAGE1_SYS04_20260819/` | `com12_sys04.uart` | `d298dc4bf6cc00031372577df32f9c6069a2b17f73c0f06b4e2a3fcef9f9e78b` | `e966318fb687464c6a8257d110edd71b970ff4bf6c8ed212fbfa0676543fe279` | `1683a389…` cited HASHES | `c2caeef` | SYS04 HOLD PASS | live hardware |
| `BOOT_STAGE1_SYS05_20260819/` | `com12_sys05.uart` | `b8f7145da7d3cf24c572a06b3e7a61f1b7b6a9a8bc281e366a1502da0665d3d8` | `ccfbbbc5a9c7ab573ccad9251dc933ab44d294ee26b7c7eccfaded0cf0955c5f` | `f40f7e13…` cited HASHES | `93bb491` | SYS05 HOLD PASS | live hardware |
| `BOOT_STAGE1_SYS06_20260820/` | `com12_sys06.uart` | `ad6b0106577972effe75e0b6597a980b55242cedbf2f650c8b4cd1b63814c95c` | `363a44aa6a0ab3b091f2f5b43dd14242cc8643afc05d237f4a4c28d0a10a5bd8` | `c346065b…` cited preflight | `de71e05` | SYS06_P1_KAT_COM12_PASS | powered-session security |
| `BOOT_STAGE1_SYS07_20260820/` | `com12_sys07_instr.uart` | `4377e3018a48789f9caf6ac359db87e932314a6c9e6a8471bf3df75669f5063f` | `0192b1007f0efde6f337992490a20b486a46e91542cd26d5a96db14256cb22d1` | NOT_VERIFIED | `cc88e73` | SYS07_COM12_PASS HOLD | live hardware |
| same | `com12_sys07.uart` | `6fd8675e4d477f4e5507256633be8e315c7eeb5fad1f6c925754df370dfe4bdc` | same BOOT | NOT_VERIFIED | `37d9094` | incomplete | live hardware |
| `BOOT_STAGE1_SYS08_20260820/` | `com12_sys08_crcfix.uart` | `75ee7277e6d554c47ff2f559d6af0752f0e8b816339752e137b5504053d768a7` | `8d0aefa6f5aef206a1eec6617f988a14aa3ea07733f167c3c51620159f201ef7` | NOT_VERIFIED | `b9e34b4` | SYS08_COM12_PASS HOLD | live hardware |
| same | `com12_sys08.uart` | `97462f04ce6096ed17c95619c171225dba7d4b3654f011a477ee7c83d388dbcf` | earlier PCAP image | NOT_VERIFIED | `f29b4c4` | PL FAIL | live hardware |
| same | `com12_sys08_diag.uart` | `75c20baa18a495fac3e1280b0c30faa51f8f667e41dc351c78d84b6f6060017d` | diag | NOT_VERIFIED | `f76d981` | PL FAIL class=CRC | live hardware |
| `BOOT_STAGE1_SYS09_20260820/` | `com12_sys09.uart` | `391218176cef09b7d642b71f5cca9990dec2d91b95cc73c9879185d9dfa0219d` | `eb2f34c3052f9c1a17b0ab34d31385decb0a3cc1c9c75fabacce97e9e63d7b27` | `08502fd5fd9f9a5c16f333571a1e9d6f9970bb1d4618ae87a3683e0d4f77c082` | `40eaad4` | SYS09_COM12_PASS Recovery | live hardware |
| `BOOT_STAGE1_SYS10A_20260820/` | `com12_sys10a.uart` | `ceb02585476e6b316f94c01c720e4f385ba1f6d86aaf2b1c9a3a67dd3294c64d` | `8b396c6f519935cb8099747db466c2bc7647df0666ece36ccf6d0a261e577874` | `cbc4e37ad3a8444541db886a040a42dc0c94c9a09102bffb3b51f5280ee26854` | `3a8049b` | SYS10A_COM12_PASS Product; SYS10_OVERSIZE | live hardware |
| same | `com12_sys10a_stack_hang.uart` | `01956933f2715d2e65a608421ce29229c7a021b482800811309d2d8cfc8efe5e` | unified (oversize parse hang) | same ELF | — | fail / hang | live hardware |
| same | `com12_sys10a_binary_burst.uart` | `30b77ee0528cd6dfb6ca329733bd91fd8942eb56347b7608fd9db18f36fa4867` | unified (AC smash) | same ELF | — | fail / stale | live hardware |
| SYS11 | n/a new UART | uses rows above | n/a | n/a | `a39ac18` | host-only classifier | host-only |
| `BOOT_STAGE1_SYS12_20260820/` | see top table | | | | | Recovery | live hardware, recovery |

SYS06 `BOOT_SYS06_P1.BIN` and SYS07 `BOOT_SYS07_P1.BIN` are gitignored; hashes are from committed HASHES files. On-disk re-hash of those BOOTs: **NOT_VERIFIED** if the oracle files are absent.

PATH1_G64 `system.bit` SHA-256 `d19f08d20cac85ea0e33c41a8a552166f5a9176e16977f09aa64d17e68135829` (4045769) is packed into SYS08–SYS12 images.
