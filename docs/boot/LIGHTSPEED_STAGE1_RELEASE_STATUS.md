<!-- Copyright LightSpeed Studio LLC -->
<!-- SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-GPOS-Commercial -->

# LightSpeed Studio LLC — Stage 1 release status

> Golden Phase Utility OS and its owned PZ7020 post-ROM boot path are complete for the current development-hardware scope. Final secure-production key provisioning is intentionally deferred until custom hardware supplies the selected retention, secure-element and recovery infrastructure.

## Development-hardware close

- System 12 is the current **final development-hardware proof** (plaintext recovery `gpos>` vs gold UART contract). Frozen `BOOT.BIN` `738c6a0155d9558c37d51fffe11bc99236c4c29242caa07fa860756f47d24352`.
- System 10A remains the unified **product-path** `gpos>` proof (`8b396c6f…`, software RSA, **SYS10_OVERSIZE**).
- Systems 13–15 are not started ([`DEFERRED_PRODUCTION_SECURITY_HARDWARE.md`](DEFERRED_PRODUCTION_SECURITY_HARDWARE.md)).

## Local main image (host pack, 2026-08-20)

`fpga/zynq/release/PZ7020_MAIN_OWNED_STAGE1/BOOT.BIN` SHA-256 `e64909431273537d7a390d800ed3dad0cc84ab8e0a5ac2a25321fbbc47355d55` (6321504). Newly built `zynq-a9,utility` ELF `e_entry` `0x00100060` packed with owned Stage 1 and frozen PATH1_G64 `system.bit`. **Not COM12-proven.** Board-proven recovery remains `738c6a01…`.

## Size honesty

Stage 1 is oversized relative to the original **8192**-byte goal. SYS10A PT_LOAD `p_filesz` **29836**, `p_memsz` **29840**, OCM span **38032** (`docs/boot/STAGE1_SIZE_REPORT.md`, `docs/boot/SYS10A_ARTIFACT_HASHES.txt`). Full linked functionality was preserved. No security feature was deleted to meet the estimate. This is **SYS10_OVERSIZE**, not an 8 KiB PASS.

## Future (not this release)

Production key provisioning, BootROM RSA eFuse, permanent AES policy, and universal x86/ARM/RISC-V Stage 1 ports are future projects. They are not implied by System 12.
