<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# System 11 UART reference

File-only classifier. No COM port. No card write. Fixtures are existing COM12 captures; none were synthesized.

Branch from System 10 PASS commit `3a8049b` (`SYS10_OVERSIZE` product UART).

## UART bytes vs host-script bytes

Capture scripts (`capture_sys10a_com12.ps1` and the SYS03–SYS09 siblings) print host banners with `Write-Host`:

- `=== COM12 open 115200 8N1 DTR=off RTS=off. PRESS PZ7020 RESET ONCE.`
- `SAW_GPOS` / `SAW_FAIL` / `SAW: <marker> @<offset>`
- `wrote <path> (<n> bytes)`

Those strings are **not** in the `.uart` files. SYS10A stores only bytes copied from the serial Read buffer (`WriteAllBytes` of that buffer). SYS03 stores `ReadExisting` into the uart file only. A uart that contains `SAW_GPOS`, `=== COM12`, or `PRESS PZ7020 RESET` is host-injected and must fail closed.

## Known-good gold / product UARTs

| Role | Path | SHA-256 | Bytes |
| --- | --- | --- | ---: |
| Product PASS (SYS10A) | `fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS10A_20260820/com12_sys10a.uart` | `ceb02585476e6b316f94c01c720e4f385ba1f6d86aaf2b1c9a3a67dd3294c64d` | 7107 |
| Recovery PASS (SYS09 gold handoff) | `fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS09_20260820/com12_sys09.uart` | `391218176cef09b7d642b71f5cca9990dec2d91b95cc73c9879185d9dfa0219d` | 6994 |
| AES KAT HOLD (SYS06) | `fpga/zynq/evidence/kairos_zynq_boot/BOOT_STAGE1_SYS06_20260820/com12_sys06.uart` | `ad6b0106577972effe75e0b6597a980b55242cedbf2f650c8b4cd1b63814c95c` | 284 |

Product order (exact trimmed lines): `GP-S1 BEGIN` → `DDR PASS` → `IMAGE HEADER PASS` → `PARTITIONS count=4` → `DISPATCH RSA` → `STAGE1 RSA2048 PARTITION PASS implementation=SOFTWARE` → `PCAP BEGIN` → `PL PASS` → `HANDOFF PASS` → `JUMP 00100060` → `gpos>`.

Recovery is classified separately: `PARTITIONS count=3`, no `DISPATCH RSA`, no `ENCRYPTED`, same `JUMP 00100060`. AES is classified separately: `ENCRYPTED` → `P1 KAT PASS` → `SYS06 HOLD`, no JUMP.

## Real failure / incomplete UARTs

| Asked class | Actual UART | SHA-256 | Bytes | Notes |
| --- | --- | --- | ---: | --- |
| DDR failure | **none** | — | — | no COM12 UART exists for DDR FAIL. SYS03 `com12_sys03.uart` is DDR PASS then HOLD (`8fcdc3257f3449a9114594474d0e34d8e71540348081da82336f819f132f0dee`, 112). |
| ADMA wait failure | **none** | — | — | no COM12 UART exists for ADMA wait FAIL. SYS04 `com12_sys04.uart` is ADMA PASS then HOLD (`d298dc4bf6cc00031372577df32f9c6069a2b17f73c0f06b4e2a3fcef9f9e78b`, 195). |
| Parser failure | `BOOT_STAGE1_SYS10A_20260820/com12_sys10a_stack_hang.uart` | `01956933f2715d2e65a608421ce29229c7a021b482800811309d2d8cfc8efe5e` | 185 | ADMA completed; `IMAGE HEADER PASS` never printed. |
| Bit-load / PCAP failure | `BOOT_STAGE1_SYS08_20260820/com12_sys08.uart` | `97462f04ce6096ed17c95619c171225dba7d4b3654f011a477ee7c83d388dbcf` | 245 | `PCAP BEGIN` then `PL FAIL`. |
| PCAP diagnostic CRC | `BOOT_STAGE1_SYS08_20260820/com12_sys08_diag.uart` | `75c20baa18a495fac3e1280b0c30faa51f8f667e41dc351c78d84b6f6060017d` | 557 | `PL FAIL class=CRC`. |
| Incomplete product | `BOOT_STAGE1_SYS10A_20260820/com12_sys10a_binary_burst.uart` | `30b77ee0528cd6dfb6ca329733bd91fd8942eb56347b7608fd9db18f36fa4867` | 6988 | RSA line then `0xFF`; no `JUMP 00100060`. |
| PCAP HOLD (not product) | `BOOT_STAGE1_SYS08_20260820/com12_sys08_crcfix.uart` | `75ee7277e6d554c47ff2f559d6af0752f0e8b816339752e137b5504053d768a7` | 257 | `PL PASS` → `SYS08 HOLD`, no JUMP. |
| Parser HOLD | `BOOT_STAGE1_SYS05_20260819/com12_sys05.uart` | `b8f7145da7d3cf24c572a06b3e7a61f1b7b6a9a8bc281e366a1502da0665d3d8` | 258 | `IMAGE HEADER PASS` then `SYS05 HOLD`; no JUMP. |
| RSA HOLD | `BOOT_STAGE1_SYS07_20260820/com12_sys07.uart` | `6fd8675e4d477f4e5507256633be8e315c7eeb5fad1f6c925754df370dfe4bdc` | 329 | BootROM line then binary; no JUMP. |
| RSA HOLD (instr) | `BOOT_STAGE1_SYS07_20260820/com12_sys07_instr.uart` | `4377e3018a48789f9caf6ac359db87e932314a6c9e6a8471bf3df75669f5063f` | 528 | Software partition PASS then `SYS07 HOLD`; no JUMP. |

No uart in this tree contains `JUMP 00100020`. The classifier still rejects that string on a mutated copy of the product PASS file. The binary-burst capture has a leftover exact `gpos>` line and no `JUMP 00100060`; that is stale, not product.

## Bound BOOT hashes (not uart)

| Image | SHA-256 | Bytes |
| --- | --- | ---: |
| SYS10A unified | `8b396c6f519935cb8099747db466c2bc7647df0666ece36ccf6d0a261e577874` | 6018260 |
| SYS09 handoff | `eb2f34c3052f9c1a17b0ab34d31385decb0a3cc1c9c75fabacce97e9e63d7b27` | 6005972 |

System 12 is not started.
