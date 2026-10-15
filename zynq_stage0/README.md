# Golden Phase stage-0 (`golden_phase_stage0.elf`)

Board init only: DDR, clocks, MIO, UART, SD, GIC, PL bitstream load — **not** an OS.

## Current seal (V13)

Release images under `fpga/zynq/release/PZ7020_GP_OS_V13/` and `…_META/` place
**`golden_phase_stage0.elf`** in the BootROM bootloader slot (replacing the
`zynq_fsbl.elf` *name* in `boot.bif`).

Binary content matches the sealed V2/V6 seller-PS7 FSBL (same DDR/MIO facts).
A from-scratch `ps7_init` port is tracked separately; do not claim a new DDR
bring-up until that lands.

## Contract

```text
BootROM → golden_phase_stage0.elf → system.bit → kairos_zynq_os.elf
```

UART evidence after handoff includes `STAGE0 BOOT PASS` then the GP-OS seals.
