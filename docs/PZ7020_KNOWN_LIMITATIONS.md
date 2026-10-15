# PZ7020 Known Limitations

What Utility OS V1 on this board does **not** do. Status names match
`docs/PZ7020_REMAINING_CAPABILITY_20260817.md`. Proven surface:
`docs/UTILITY_OS_V1.md`. UART proofs under `fpga/zynq/evidence/kairos_zynq_boot/BOOT_A*/`.

Do not quote the July 2026 appendix as current board fact.

## Hardware unavailable

### IOMMU — `HARDWARE_UNAVAILABLE`

Zynq-7000 PS on PZ7020 has **no SMMU**. The Cortex-A9 MMU is a CPU translation
unit. It is not an IOMMU. A discrete IOMMU is not a companion chip on this
SoC. Custom-board path: `dma_contain` + optional PL AXI firewall; real SMMU
only on a later UltraScale+ class board
([`docs/CUSTOM_ZYNQ7000_BOARD.md`](CUSTOM_ZYNQ7000_BOARD.md) §5).

Software bound (not an IOMMU): `src/kernel/dma_contain.rs`. GEM `wire_tx_raw`
refuses descriptors the pool rejects. USB QH/qTD/SETUP/DATA use a **separate**
`USB_DMA` instance in `src/board/zynq7000/usb.rs` (not GEM's 8-pool table).
A malicious bus master can still DMA anywhere the interconnect allows.

### Hardware TRNG — `HARDWARE_UNAVAILABLE`

No PS TRNG. Source name: `TIMER_JITTER_SEEDED_DRBG`
(`src/board/zynq7000/rng.rs`). `EntropyPolicy::HardwareRequired` fails closed.
Do not call this a TRNG. SSH keys from this source are not high-assurance.
Custom Zynq-7000 board: ATECC608B `Random`
([`docs/CUSTOM_ZYNQ7000_BOARD.md`](CUSTOM_ZYNQ7000_BOARD.md)). Do not print
`ENTROPY_SOURCE: ATECC608B` on PZ7020.

### Device-bound checkpoint auth — not on this silicon

No eFUSE / BBRAM / PUF / ATECC in this Utility path. Research stash is not
`main`. Custom-board keystore: ATECC608B I²C; UART gate `HARDWARE_BOUND: YES`
only after that PCB.

## Proven, with named bounds

- **GEM 100/full and 1G** — A16 / A28 host 32/32 EtherType `0x88B5`. Idle 1G
  drops the first ~7–12 min-size frames; `wire_init` primes 16. Splash still
  prints `Ethernet attachment: none` (VirtIO); live path is GEM0.
- **SSH** — A27 baremetal `ssh-bm` PTY. Not hosted tokio/russh. Tree
  `AUTHORIZED_PUBKEY` is zeros; use `net ssh key`.
- **MMU + CPU1** — CPU0 isolation (Client D0, L2 W^X). CPU1 online,
  kernel-only L1. Apps CPU0-only. No SMP TLB shootdown. UART:
  `HARDWARE_BOUND apps=CPU0-only; SMP TLB shootdown=NO`.
- **USB hub HID** — A25. Nested expand past depth 3 is refused on purpose.
- **PhaseFS journal** — recover + `fsj check` proven. Do not run `fsj format`
  on the sacrificial SD.
- **G1P/TNSR/G64S** — gold PATH1_G64, `G1P_DONE`. G1P SPI 61 through the
  GIC is `PASS_ON_PZ7020` (`g1p irq-test` 8/8 `missed=0`). Polled
  `G1P_DONE` stays valid. HID drain stays polled.
- **USB DMA pools** — private `USB_DMA` (QH/qTD/SETUP/DATA). COM12
  `usb dma: pools=4 submits=0 refused=0 (not an IOMMU)`. Not an IOMMU.

## Not proven / not in the V1 path

- `zynq_stage1` replacing Xilinx FSBL on hardware.
- PCAP / devcfg bitstream load from the kernel (FSBL still loads `system.bit`).
- Guest RISC-V VM executing a program (boot says ready only).
- Phase Math `phaseperf` / `verify` beyond boot scalar self-check.

## Optional (not this Utility path)

A/B `BOOT.BIN` slots, a locked ship image (strip `fsj format` / crash `p6`–`p11`),
and BBRAM/eFUSE secure boot are **optional**. They are not required to run
Utility on PZ7020. BBRAM is FSBL/bootgen encryption, not a `kairos_zynq`
driver. See [`docs/CUSTOM_ZYNQ7000_BOARD.md`](CUSTOM_ZYNQ7000_BOARD.md).

## Operator

- COM12 115200 8-N-1, DTR/RTS off. Sacrificial SD `121220160204`. FAT32 copy
  only; no raw-sector format as a “gap close”.
- Toolchain paths in `.cargo/config.toml` / `boot.bif` are machine-specific.

## Appendix — July 2026 console milestone (historical)

The following were true of the 2026-07-30 console-proof image. They are
**not** V1 current fact. Kept so old reports still parse.

- GEM not in that `kairos_zynq` build; boot `Ethernet attachment: none` meant
  no GEM driver yet.
- Hosted SSH / hosted checkpoint only.
- Single core, MMU/caches off, phys==virt.
- Arch label `baremetal-riscv64` on an early ELF (later images print
  `baremetal-armv7-a-cortex-a9`).
