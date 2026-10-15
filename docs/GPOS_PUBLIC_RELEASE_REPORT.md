# GPOS public release report

**Release status:** AUDIT FAIL - do not publish
**Repo:** 1 of 3 - Golden Phase Operating System (this Git repository is GPOS; crate name `kairos`).
**Certified SHA:** `97ed1e190cf573e62a0fbc9713e0604ae48a24e9`
**Branch:** `producing`
**GPOS_PUBLIC_READY:** unset
**When:** 2026-10-15T11:16:12.2890399-06:00
**Push:** not performed by audit-only; publish mutates git only after READY=1 and zero failures.

## License checks

| Artifact | Status |
|----------|--------|
| `LICENSE.md` | dual AGPLv3-only Path A + commercial Path B |
| `LICENSE-AGPL-3.0.txt` | GNU AGPLv3 text |
| `COMMERCIAL_LICENSE.md` | Path B instrument pointer |
| `CONTRIBUTING.md` | CLA |
| `COPYRIGHT_NOTICE` | ownership / docket; public distribution is LICENSE.md |
| `src/core/gphase_core.rs` | not modified by this release script |
| Fail-closed SSH | `AUTHORIZED_PUBKEY = [0u8; 32]` required |

## Feature inventory

Required tokens remain in `Cargo.toml`: utility, ssh-bm, usb, sched2, tensor, cpu1, craft, workspace member products/craft-mainframe. Audit must not drop them to pass.

## HEAD / history / secrets

- AUTHORIZED_PUBKEY is [0u8; 32]
- HEAD tree has no deny-listed FSBL/bit/BOOT/ELF/scratchpad/private-key names
- scratchpad never committed
- history name: fpga/zynq/board/pz7020-sl-c/seller/BOOT.BIN
- history name: fpga/zynq/board/pz7020-sl-c/seller/course/2_SDK/2_SDK/5_hello_world_fsbl/PZ_Uart.sdk/ts_uart/bootimage/BOOT.bin
- history name: fpga/zynq/build_phaseguard/BOOT.BIN
- history name: fpga/zynq/build_phaseguard/kairos_zynq.elf
- history name: fpga/zynq/build_phaseguard/system.bit
- history name: fpga/zynq/build_phaseguard/zynq_fsbl.elf
- history name: fpga/zynq/build_phaseguard_full/BOOT.BIN
- history name: fpga/zynq/build_phaseguard_full/kairos_zynq.elf
- history name: fpga/zynq/build_phaseguard_full/system.bit
- history name: fpga/zynq/build_phaseguard_full/zynq_fsbl.elf
- history name: fpga/zynq/evidence/kairos_zynq_boot/BOOT_A05_REPLACE_20260816/BOOT.BIN
- history name: fpga/zynq/evidence/kairos_zynq_boot/BOOT_A05_REPLACE_20260816/kairos_zynq.elf
- history name: fpga/zynq/evidence/kairos_zynq_boot/BOOT_A11_GPA_WAL_20260816/BOOT.BIN
- history name: fpga/zynq/evidence/kairos_zynq_boot/BOOT_A11_GPA_WAL_20260816/kairos_zynq.elf
- history name: fpga/zynq/evidence/kairos_zynq_boot/BOOT_A11_GPA_WAL_20260816/system.bit
- object deny: fpga/zynq/board/pz7020-sl-c/seller/BOOT.BIN
- object deny: fpga/zynq/board/pz7020-sl-c/seller/course/2_SDK/2_SDK/5_hello_world_fsbl/PZ_Uart.sdk/ts_uart/bootimage/BOOT.bin
- object deny: fpga/zynq/evidence/kairos_zynq_boot/BOOT_A05_REPLACE_20260816/BOOT.BIN
- object deny: fpga/zynq/evidence/kairos_zynq_boot/BOOT_A05_REPLACE_20260816/kairos_zynq.elf
- object deny: fpga/zynq/evidence/kairos_zynq_boot/BOOT_A11_GPA_WAL_20260816/BOOT.BIN
- object deny: fpga/zynq/evidence/kairos_zynq_boot/BOOT_A11_GPA_WAL_20260816/kairos_zynq.elf
- object deny: fpga/zynq/evidence/kairos_zynq_boot/BOOT_A11_GPA_WAL_20260816/system.bit
- object deny: fpga/zynq/evidence/kairos_zynq_boot/BOOT_A12_EXTMAP_RECOVER_20260816/BOOT.BIN
- object deny: fpga/zynq/evidence/kairos_zynq_boot/BOOT_A12_EXTMAP_RECOVER_20260816/kairos_zynq.elf
- object deny: fpga/zynq/evidence/kairos_zynq_boot/BOOT_A13_ELF_20260816/BOOT.BIN
- object deny: fpga/zynq/evidence/kairos_zynq_boot/BOOT_A13_ELF_20260816/kairos_zynq.elf
- object deny: fpga/zynq/evidence/kairos_zynq_boot/BOOT_A14_UNMAP_20260816/BOOT.BIN
- object deny: fpga/zynq/evidence/kairos_zynq_boot/BOOT_A14_UNMAP_20260816/kairos_zynq.elf
- object deny: fpga/zynq/evidence/kairos_zynq_boot/BOOT_A15_HID_KBD_MOUSE_20260817/BOOT.BIN
- object deny: fpga/zynq/evidence/kairos_zynq_boot/BOOT_A15_HID_KBD_MOUSE_20260817/kairos_zynq.elf
- no OpenSSH private-key PEM in non-parser text history
- audit PATH prepended D:\Xilinx\2026.1\Vitis\gnu\aarch32\nt\gcc-arm-none-eabi\bin; .cargo/config.toml still uses linker name arm-none-eabi-gcc
- gate pass: kairos_zynq-utility
- gate pass: craft-mainframe-build
- gate pass: craft-mainframe-test

## Build / test results

| Gate | Result |
|------|--------|
| `cargo-test-default` | FAIL exit 101 (`--test ai_bridge_tests`: `cancellation_logged`, `phase_transitions_on_success`) |
| `kairos_zynq-utility` | PASS |
| `craft-mainframe-build` | PASS |
| `craft-mainframe-test` | PASS (0 tests in package; binary builds) |

## Failures

- history contains FSBL/bit/BOOT/ELF/key/scratchpad paths (208 names); git rm --cached does not purge them; stop release
- rev-list --objects deny-list hits (124) on producing/main
- gate failed: cargo-test-default (exit 101); `ai_bridge_tests::cancellation_logged` (trace missing `cancelled`); `ai_bridge_tests::phase_transitions_on_success` (trace missing `Phi0 completed`). Not fixed here: that is AI trace vs process-transition coupling, not a license/secret issue, and not a `gphase_core` change.

**HEAD secrets/binaries:** deny-listed FSBL/bit/BOOT/`kairos_zynq.elf` untracked; `AUTHORIZED_PUBKEY` all-zero; `scratchpad/` never committed; OpenSSH PEM parser strings only.

**Publication:** not authorized. `GPOS_PUBLIC_READY` unset. `main` remains `3b64f9f`. No `origin` remote. No push.

## Publish policy

`git rm --cached` does not rewrite history. If history contains FSBL, `system.bit`, `BOOT.BIN`, `kairos_zynq.elf`, scratchpad, or private keys, stop. No squash, no `git init` replacement, no force-push.

Until `GPOS_PUBLIC_READY=1` and this SHA still matches `producing` HEAD, production readiness is **not** claimed.
