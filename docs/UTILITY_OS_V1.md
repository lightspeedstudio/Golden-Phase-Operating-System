# Utility OS V1 — PZ7020 (main `8c8a298`)

Golden Phase **Utility OS** on PZ7020-SL-C. Not Mission OS (`craft` stays off).
Not a second kernel. Same `kairos` tree, board image:

`--no-default-features --features "zynq-a9,utility"` packed into gold PATH1_G64
`BOOT.BIN` (`scripts/pack_zynq_app_into_boot.py`). Do not rebuild PL. Do not
`fsj format`. Sacrificial SD `121220160204`. COM12 DTR/RTS off.

Splash still prints `v0.1.0 [gpos]`. This document names the **proven Utility
surface** on `main` as of 2026-08-18, not a Cargo version bump.

## What V1 is

One gold bitstream: G1P+TNSR at `0x40000000`, G64S at `0x40001000`
(`system.bit` `d19f08d2…8135829`). Cortex-A9 Utility ELF. Interactive `gpos>`
on PS UART0. CPU0 isolation + CPU1 online. GEM0 EtherType `0x88B5` at 100/full
and 1G (init primes 16 frames). USB HID on J10 and behind hub. Baremetal SSH
PTY. PhaseSentinel E2E. UART–GEM PhaseLink bridge. PhaseFS journal recover.

## Board proofs (do not re-open)

| Surface | Evidence | Class |
| --- | --- | --- |
| G1P / TNSR / G64S same-boot | gold PATH1_G64 UART `G1P_DONE` | fabric PASS |
| MMU isolation + journal | A21 combined matrix | 10/10 + `fsj check` |
| Ethernet 100/full 32/32 | `BOOT_A16_88B5_TXBURST_20260817/` | `WIRE_88B5_32` |
| Ethernet 1G 32/32 | `BOOT_A28_1G_20260817/` `114435` / `114656` | `WIRE_88B5_32_1G` |
| PhaseSentinel E2E | `BOOT_A24_PHASESENTINEL_E2E_20260817/` | contain then Allow |
| USB hub HID | `BOOT_A25_HUB_MOUSE_DEBUG_20260817/` | `USB_HUB_HID` |
| UART–GEM bridge | `BOOT_A26_UARTGEM_BRIDGE_20260817/` | `ack:ping-e2e` |
| SSH PTY | `BOOT_A27_SSH_PTY_20260817/` | `SSH_PTY` |
| GPA WAL + recovered `app run` | A11 + A23 | `EXISTING_PROOF_BOUND` |
| USB DMA pools | `BOOT_USB_DMA_GIC_20260818/` | `usb dma: pools=4` (not IOMMU) |
| G1P SPI 61 through GIC | `BOOT_USB_DMA_GIC_20260818/` `164811` | `g1p irq-test` 8/8 `missed=0` |

Matrix: `docs/PZ7020_REMAINING_CAPABILITY_20260817.md`.

## What V1 is not

- Not Mission / craft.
- Not Linux, not U-Boot, not a custom FSBL on the proven path.
- Not IOMMU (no SMMU). CPU MMU is not an IOMMU.
- Not a hardware TRNG. Entropy is timer-jitter DRBG.
- Not device-bound checkpoint-auth (no eFUSE/BBRAM/PUF).
- Custom Zynq-7000 board (ATECC608B, MAC EEPROM, RTC, BBRAM; IOMMU still not a
  chip): [`docs/CUSTOM_ZYNQ7000_BOARD.md`](CUSTOM_ZYNQ7000_BOARD.md).
- Not “all devices IRQ-driven”: G1P SPI 61 through the GIC is
  `PASS_ON_PZ7020`; HID drain and GEM RX stay polled (one-shot). Polled
  `G1P_DONE` stays valid.
- Not nested USB hub expand past the depth refuse.
- Not `fsj format` on the sacrificial card.
- Tree `AUTHORIZED_PUBKEY` is zeros; runtime `net ssh key` only.

A/B boot slots, a locked ship image, and BBRAM secure boot are **optional**
and not claimed. See `docs/PZ7020_KNOWN_LIMITATIONS.md`.

Idle 1G still drops the first ~7–12 min-size frames. `wire_init` sends 16 prime
frames so counted `txburst 32` is intact. USB QH/qTD/SETUP/DATA use a private
`USB_DMA` pool (not GEM's table). That is software DMA mitigation, not an IOMMU.

## Limits file

Current limits: `docs/PZ7020_KNOWN_LIMITATIONS.md`. July 2026 console-milestone
lines in that file are historical. Do not quote them as V1 board fact.
